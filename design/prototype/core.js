// Shared by the main window and the Mini Window: preferences, lookup, entry mounting.
import { DICTS, GROUPS, ENTRIES, LEMMAS, T, HTML_LANG } from './data.js';

export const store = {
  get(key, fallback) {
    try {
      const raw = localStorage.getItem('opendict.' + key);
      return raw === null ? fallback : JSON.parse(raw);
    } catch { return fallback; }
  },
  set(key, value) {
    try { localStorage.setItem('opendict.' + key, JSON.stringify(value)); } catch { /* storage blocked: keep working in memory */ }
  },
};

const BY_ID = Object.fromEntries(DICTS.map(d => [d.id, d]));
const ALL_IDS = DICTS.map(d => d.id);
const storedOrder = store.get('order', ALL_IDS).filter(id => BY_ID[id]);

export const prefs = {
  locale: store.get('locale', 'zh'),
  group: store.get('group', GROUPS[0].id),
  order: [...storedOrder, ...ALL_IDS.filter(id => !storedOrder.includes(id))],
  disabled: store.get('disabled', []),
  membership: store.get('membership', Object.fromEntries(GROUPS.map(g => [g.id, g.dicts]))),
  authored: store.get('authored', []),
  wordbook: store.get('wordbook', []),
};
if (!T[prefs.locale]) prefs.locale = 'zh';
if (!GROUPS.some(g => g.id === prefs.group)) prefs.group = GROUPS[0].id;

export const save = key => store.set(key, prefs[key]);

export const MOD = /Mac|iPhone|iPad/.test(navigator.platform) ? '⌘' : 'Ctrl';

export function t(key, ...args) {
  const value = T[prefs.locale][key] ?? T.zh[key] ?? key;
  return typeof value === 'function' ? value(...args) : value;
}

export const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

// Normalization: case, diacritics, width and kana type do not distinguish headwords.
export function norm(s) {
  return s.normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase()
    .replace(/[ァ-ヶ]/g, c => String.fromCharCode(c.charCodeAt(0) - 0x60));
}

export const orderedDicts = () => prefs.order.map(id => BY_ID[id]);

export function activeDicts() {
  const members = prefs.membership[prefs.group] ?? [];
  return orderedDicts().filter(d => members.includes(d.id) && !prefs.disabled.includes(d.id));
}

export const hits = hw => activeDicts().filter(d => ENTRIES[d.id]?.[hw]);

export function headwords() {
  const set = new Set();
  for (const d of activeDicts()) for (const hw of Object.keys(ENTRIES[d.id] ?? {})) set.add(hw);
  return [...set].sort(byIndexOrder);
}

// One ordering for the headword index, used both to sort it and to find a word's place in it.
function byIndexOrder(a, b) { return norm(a).localeCompare(norm(b), 'en'); }

function distance(a, b) {
  if (Math.abs(a.length - b.length) > 2) return 3;
  let prev = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const cur = [i];
    for (let j = 1; j <= b.length; j++) {
      cur[j] = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
    }
    prev = cur;
  }
  return prev[b.length];
}

// What the user typed -> headwords to offer. Prefix first, wildcards if present,
// the lemma in front when the input is an inflected form, close matches as a last resort.
export function resolve(input) {
  const typed = input.trim();
  const nq = norm(typed);
  const all = headwords();
  // Nothing typed: the column is the whole headword index, to browse.
  if (!nq) return { items: all, lemma: null, near: false, miss: false, browse: true };
  let items;
  if (/[*?]/.test(nq)) {
    const pattern = nq.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.');
    const re = new RegExp('^' + pattern + '$');
    items = all.filter(hw => re.test(norm(hw)));
  } else {
    items = all.filter(hw => norm(hw).startsWith(nq));
  }
  let lemma = null;
  const base = LEMMAS[typed] ?? LEMMAS[nq];
  if (base && all.includes(base)) {
    lemma = { from: typed, to: base };
    items = [base, ...items.filter(hw => hw !== base)];
  }
  let near = false, miss = false;
  if (!items.length) {
    items = all.map(hw => [hw, distance(norm(hw), nq)]).filter(([, d]) => d <= 2).sort((a, b) => a[1] - b[1]).map(([hw]) => hw);
    near = items.length > 0;
    if (!near) {
      // No match at all: open the index where the word would have stood. If nothing
      // stands after it, there is nothing relevant to offer, so the column stays empty.
      miss = true;
      items = all.filter(hw => byIndexOrder(hw, typed) > 0);
    }
  } else if (!/[*?]/.test(nq)) {
    // Like a printed page, the column carries on from the match through the index.
    const last = Math.max(...items.map(hw => all.indexOf(hw)));
    items = [...items, ...all.slice(last + 1).filter(hw => !items.includes(hw))];
  }
  return { items, lemma, near, miss, browse: false };
}

// Entries are third-party HTML. Each one lives in its own shadow root so a
// dictionary's styles cannot leak into the app or into another dictionary.
export function mountEntry(host, dict, hw, onLink) {
  const root = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
  root.innerHTML = `<style>:host{display:block}.e{overflow-wrap:anywhere}.e img{filter:var(--entry-filter,none)}${dict.css}</style><div class="e" lang="${HTML_LANG[dict.lang]}">${ENTRIES[dict.id][hw]}</div>`;
  if (host.dataset.wired) return;
  host.dataset.wired = '1';
  root.addEventListener('click', event => {
    const link = event.target.closest('a[href^="entry://"]');
    if (!link) return;
    event.preventDefault();
    onLink(decodeURIComponent(link.getAttribute('href').slice('entry://'.length)));
  });
}

// Fill every [data-t], [data-t-aria], [data-t-ph] and [data-t-title] from the string table.
export function applyLocale(root = document) {
  document.documentElement.lang = HTML_LANG[prefs.locale];
  for (const el of root.querySelectorAll('[data-t]')) el.textContent = t(el.dataset.t, MOD);
  for (const el of root.querySelectorAll('[data-t-aria]')) el.setAttribute('aria-label', t(el.dataset.tAria));
  for (const el of root.querySelectorAll('[data-t-ph]')) el.placeholder = t(el.dataset.tPh);
  for (const el of root.querySelectorAll('[data-t-title]')) el.title = t(el.dataset.tTitle);
  for (const el of root.querySelectorAll('[data-mod]')) el.textContent = MOD;
}

// Theme: 'system' follows the OS; 'light' and 'dark' pin a scheme. The <meta> is the
// single source the stylesheet reads.
const schemeMeta = () => document.querySelector('meta[name="color-scheme"]');
export const getTheme = () => ({ light: 'light', dark: 'dark' }[schemeMeta().content] ?? 'system');
export function setTheme(value) {
  schemeMeta().content = value === 'system' ? 'light dark' : value;
  try {
    if (value === 'system') localStorage.removeItem('color-scheme');
    else localStorage.setItem('color-scheme', value);
  } catch { /* not persisted, still applied */ }
}
addEventListener('storage', event => {
  if (event.key === 'color-scheme') schemeMeta().content = event.newValue ?? 'light dark';
});

export function toggleWord(hw, from) {
  const at = prefs.wordbook.findIndex(w => w.hw === hw);
  if (at >= 0) prefs.wordbook.splice(at, 1);
  else prefs.wordbook.unshift({ hw, from, at: Date.now() });
  save('wordbook');
  return at < 0;
}

const svg = body => `<svg class="i" viewBox="0 0 16 16" aria-hidden="true">${body}</svg>`;
export const icon = {
  chev: svg('<path d="M4 6l4 4 4-4"/>'),
  up: svg('<path d="M4 10l4-4 4 4"/>'),
  speak: svg('<path d="M2.5 6v4H5l3.5 3V3L5 6z"/><path d="M11 5.5a3.5 3.5 0 0 1 0 5"/>'),
  mark: svg('<path d="M4 2.5h8v11L8 10.5l-4 3z"/>'),
  page: svg('<path d="M4 2.5h5.5L12 5v8.5H4z"/><path d="M9.5 2.5V5H12"/>'),
  check: svg('<path d="M3.5 8.5l3 3 6-7"/>'),
  close: svg('<path d="M4 4l8 8M12 4l-8 8"/>'),
};
