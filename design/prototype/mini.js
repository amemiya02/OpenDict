// Mini Window demo: summon a lookup over a stand-in reading window.
import { HEAD, HTML_LANG } from './data.js';
import { prefs, t, esc, MOD, hits, resolve, mountEntry, applyLocale, toggleWord, icon } from './core.js';

prefs.group = 'all'; // the demo answers from every dictionary; this is not saved

const $ = id => document.getElementById(id);
const mini = $('mini'), mq = $('mq'), sheet = $('mini-sheet'), rail = $('mini-rail'), page = $('mini-page');
const saveBtn = $('mini-save'), openLink = $('mini-open');
let current = null, opener = null;

const langOf = hw => HTML_LANG[HEAD[hw]?.lang ?? 'en'];

function renderSave() {
  const saved = prefs.wordbook.some(w => w.hw === current);
  saveBtn.disabled = !current;
  saveBtn.setAttribute('aria-pressed', String(saved));
  saveBtn.innerHTML = `${icon.mark}<span>${t(saved ? 'saved' : 'save')}</span>`;
}

function render(input) {
  const typed = input.trim();
  const r = resolve(typed);
  current = r.browse || r.miss ? null : r.items[0] ?? null;
  rail.innerHTML = '';
  openLink.href = 'index.html?q=' + encodeURIComponent(current ?? typed);
  if (!current) {
    sheet.innerHTML = `<header class="head"><h1 id="mini-title">【${esc(typed || 'OpenDict')}】</h1></header>${typed ? `<p class="lead">${t('mini_none', typed)}</p>` : ''}`;
    renderSave();
    return;
  }
  const hw = current, found = hits(hw), meta = HEAD[hw] ?? {};
  const note = r.lemma ? `<p class="note">${t('lemma')} <b>${esc(r.lemma.from)}</b> → <b>${esc(r.lemma.to)}</b></p>` : r.near ? `<p class="note">${t('near')}</p>` : '';
  sheet.innerHTML = `<header class="head"><h1 id="mini-title" lang="${langOf(hw)}">【${esc(hw)}】</h1>${meta.say ? `<span class="ipa" lang="${langOf(hw)}">${esc(meta.say)}</span>` : ''}</header>${note}`
    + found.map(d => `<section class="dict" id="m-${d.id}" tabindex="-1" aria-labelledby="mn-${d.id}"><header class="dict-h"><h2 id="mn-${d.id}">【${esc(d.name)}】</h2><span class="tag">${t('sample')}</span></header><div class="dict-b" id="mb-${d.id}"></div></section>`).join('');
  for (const d of found) mountEntry($('mb-' + d.id), d, hw, word => { mq.value = word; render(word); });
  rail.innerHTML = found.map((d, i) => `<a class="thumb" href="#m-${d.id}" style="--i:${i}" aria-label="${esc(d.name)}" title="${esc(d.name)}"><b>${esc(d.mark)}</b></a>`).join('');
  page.scrollTop = 0;
  syncSpy();
  renderSave();
}

// Open beside the word and stay on screen: below it, else above it, else to its side.
function place(rect) {
  const width = mini.offsetWidth, height = mini.offsetHeight, span = document.documentElement.clientWidth;
  let left = Math.min(Math.max(8, rect.left), span - width - 8);
  let top = rect.bottom + 10;
  if (top + height > innerHeight - 8) {
    if (rect.top - height - 10 >= 52) top = rect.top - height - 10;
    else {
      top = Math.max(52, innerHeight - height - 8);
      left = rect.right + 12 + width <= span - 8 ? rect.right + 12 : Math.max(8, rect.left - width - 12);
    }
  }
  mini.style.left = `${left + scrollX}px`;
  mini.style.top = `${top + scrollY}px`;
}

function summon(text, getRect, from) {
  for (const w of document.querySelectorAll('.w[aria-expanded="true"]')) w.setAttribute('aria-expanded', 'false');
  opener = from ?? null;
  opener?.setAttribute('aria-expanded', 'true');
  mq.value = text;
  render(text);
  if (!mini.open) {
    // Open inside the current viewport so the browser has no reason to scroll to it.
    mini.style.left = `${scrollX + 8}px`;
    mini.style.top = `${scrollY + 60}px`;
    mini.show();
  }
  place(getRect());
  syncSpy();
  mq.focus({ preventScroll: true });
}

function dismiss() {
  if (!mini.open) return;
  mini.close();
  opener?.setAttribute('aria-expanded', 'false');
  opener?.focus({ preventScroll: true });
}

function syncSpy() {
  const links = [...rail.querySelectorAll('.thumb')];
  if (!links.length) return;
  const line = page.getBoundingClientRect().top + 48;
  const index = links.findLastIndex(a => $(a.hash.slice(1)).getBoundingClientRect().top <= line);
  const active = links[Math.max(0, index)];
  for (const a of links) {
    a.classList.toggle('target-current', a === active);
    a.setAttribute('aria-current', a === active ? 'true' : 'false');
  }
}
page.addEventListener('scroll', syncSpy, { passive: true });

rail.addEventListener('click', event => {
  const link = event.target.closest('.thumb');
  if (!link) return;
  event.preventDefault();
  const section = $(link.hash.slice(1));
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  page.scrollTo({ top: section.offsetTop - page.offsetTop - 6, behavior: calm ? 'auto' : 'smooth' });
  section.focus({ preventScroll: true });
});

for (const word of document.querySelectorAll('.w')) {
  word.setAttribute('aria-expanded', 'false');
  word.setAttribute('aria-controls', 'mini');
  word.addEventListener('click', () => summon(word.textContent, () => word.getBoundingClientRect(), word));
}

mq.addEventListener('input', event => { if (!event.isComposing) render(mq.value); });
mq.addEventListener('compositionend', () => render(mq.value));
$('mfind').addEventListener('submit', event => { event.preventDefault(); render(mq.value); });
$('mini-close').addEventListener('click', dismiss);
saveBtn.addEventListener('click', () => {
  if (!current) return;
  toggleWord(current, hits(current)[0]?.name ?? '');
  renderSave();
});

document.addEventListener('keydown', event => {
  const mod = event.ctrlKey || event.metaKey, key = event.key.toLowerCase();
  if (event.key === 'Escape' && !event.isComposing && event.keyCode !== 229) { dismiss(); return; }
  if (mod && event.altKey && key === 'd') {
    // Stand-in for the global shortcut: look up whatever text is selected in the document.
    const selection = getSelection();
    const text = selection.toString().trim();
    if (!text || !selection.rangeCount) return;
    event.preventDefault();
    const range = selection.getRangeAt(0).cloneRange();
    summon(text, () => range.getBoundingClientRect());
  } else if (mod && key === 'd' && mini.open) {
    event.preventDefault();
    saveBtn.click();
  } else if (event.altKey && /^[1-9]$/.test(event.key) && mini.open) {
    event.preventDefault();
    rail.querySelectorAll('.thumb')[event.key - 1]?.click();
  }
});

// ?theme=light|dark and ?w=<index of a dotted word> open a particular state; neither is saved.
const params = new URLSearchParams(location.search);
if (['light', 'dark'].includes(params.get('theme'))) document.querySelector('meta[name="color-scheme"]').content = params.get('theme');

applyLocale();
renderSave();
document.querySelectorAll('.w')[Number(params.get('w') ?? -1)]?.click();
