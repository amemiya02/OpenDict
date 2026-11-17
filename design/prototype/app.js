// Main window of the OpenDict prototype.
import { DICTS, GROUPS, HEAD, LOCALES, HTML_LANG } from './data.js';
import {
  prefs, save, t, esc, MOD, orderedDicts, activeDicts, hits, resolve, mountEntry,
  applyLocale, getTheme, setTheme, toggleWord, icon,
} from './core.js';

const T_LOCALES = Object.keys(LOCALES);
const $ = id => document.getElementById(id);
const app = $('app'), q = $('q'), list = $('list'), sheet = $('sheet'), page = $('page'), rail = $('rail');
const palette = $('palette'), pq = $('pq'), palList = $('pal-list');

const ui = {
  items: [], sel: 0, lemma: null, near: false, miss: false,   // suggestions for what is typed
  current: null,                                  // headword on the page
  left: 'suggest', view: 'lookup',
  history: [], cursor: -1,
  folded: new Set(),
  commands: [], cmdSel: 0,
};

const say = message => { $('live').textContent = message; };
const composing = event => event.isComposing || event.keyCode === 229;
const langOf = hw => HTML_LANG[HEAD[hw]?.lang ?? 'en'];

function swap(update, after) {
  if (!document.startViewTransition) { update(); after?.(); return; }
  document.startViewTransition(update).finished.finally(() => after?.());
}

/* ---------- left column ---------- */

function leftItems() {
  if (ui.left === 'history') return [...new Set([...ui.history].reverse())];
  if (ui.left === 'wordbook') return prefs.wordbook.map(w => w.hw);
  return ui.items;
}

function renderLeft() {
  const items = leftItems();
  ui.sel = Math.min(ui.sel, items.length - 1);
  // "You are here" belongs to the word on the page; in the Library nothing in the column is current.
  const on = ui.view === 'lookup' ? ui.sel : -1;
  list.innerHTML = items.map((hw, i) => `<li role="option" id="opt-${i}" class="opt" aria-selected="${i === on}" data-i="${i}" lang="${langOf(hw)}">${i === on ? '<i class="here"></i>' : ''}<span>${esc(hw)}</span><small>${hits(hw).length}</small></li>`).join('');
  if (on >= 0) q.setAttribute('aria-activedescendant', `opt-${on}`); else q.removeAttribute('aria-activedescendant');
  $('run').innerHTML = items.length
    ? `<span lang="${langOf(items[0])}">${esc(items[0])}</span><span lang="${langOf(items.at(-1))}">${esc(items.at(-1))}</span>`
    : `<span>${t(ui.left)}</span>`;
  const note = $('note');
  const showNote = ui.left === 'suggest' && (ui.lemma || ui.near || (ui.miss && items.length > 0));
  note.hidden = !showNote;
  if (showNote) note.innerHTML = ui.lemma ? `${t('lemma')} <b>${esc(ui.lemma.from)}</b> → <b>${esc(ui.lemma.to)}</b>` : t(ui.near ? 'near' : 'miss_note');
  const empty = $('list-empty');
  empty.hidden = items.length > 0;
  empty.textContent = t('list_empty_' + ui.left);
  for (const tab of document.querySelectorAll('[data-left]')) tab.setAttribute('aria-pressed', String(tab.dataset.left === ui.left && ui.view === 'lookup'));
  $('open-library').setAttribute('aria-pressed', String(ui.view === 'library'));
  list.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' });
}

function setLeft(mode) {
  ui.left = mode;
  ui.sel = 0;
  if (ui.view === 'library') { ui.view = 'lookup'; renderPage(); }
  renderLeft();
}

/* ---------- lookup page ---------- */

function show(hw, { commit = false, animate = false } = {}) {
  ui.view = 'lookup';
  ui.current = hw;
  if (commit && hw && ui.history[ui.cursor] !== hw) {
    ui.history.splice(ui.cursor + 1);
    ui.history.push(hw);
    ui.cursor = ui.history.length - 1;
  }
  if (animate) swap(renderPage); else renderPage();
  $('back').disabled = ui.cursor <= 0;
  $('fwd').disabled = ui.cursor >= ui.history.length - 1;
}

function renderPage() {
  app.dataset.view = ui.view;
  if (ui.view === 'library') renderLibrary(); else renderLookup();
  const group = GROUPS.find(g => g.id === prefs.group);
  $('st-group').textContent = `${t('group')}：${group.name}`;
  $('st-count').textContent = t('dicts_n', activeDicts().length);
}

const wordList = words => `<ul class="words">${words.map(hw => `<li><button class="word" data-word="${esc(hw)}" lang="${langOf(hw)}">${esc(hw)}</button></li>`).join('')}</ul>`;

// The page when no Entry is showing. It always offers somewhere to go next.
function blank() {
  const typed = q.value.trim();
  if (!typed) {
    const recent = [...new Set([...ui.history].reverse())].slice(0, 24);
    const saved = prefs.wordbook.map(w => w.hw).slice(0, 24);
    return `<header class="head"><h1>【OpenDict】</h1><span class="meta">${t('dicts_n', activeDicts().length)}</span></header>
      <p class="lead">${t('tips', MOD)}</p>
      ${recent.length ? `<h2 class="sub">${t('recent')}</h2>${wordList(recent)}` : ''}
      ${saved.length ? `<h2 class="sub">${t('wordbook')}</h2>${wordList(saved)}` : ''}
      <h2 class="sub">${t('index')}</h2>${wordList(ui.items)}`;
  }
  // No match: say so, then offer the other Group, the wildcard, and the index where the word would stand.
  const widest = GROUPS.at(-1);
  let elsewhere = null;
  if (prefs.group !== widest.id) {
    const keep = prefs.group;
    prefs.group = widest.id;
    const r = resolve(typed);
    prefs.group = keep;
    if (!r.miss) elsewhere = r.items[0];
  }
  const pattern = typed.length > 2 ? `${typed.slice(0, 2)}*` : `${typed}*`;
  return `<header class="head"><h1>【${esc(typed)}】</h1><span class="meta">${t('not_found')}</span></header>
    <p class="lead">${t('nomatch', typed)}</p>
    <div class="ways">${elsewhere ? `<button class="btn" data-go-group="${widest.id}">${t('in_all', widest.name, elsewhere)}</button>` : ''}
      <span>${t('wildcard_tip')}</span><button class="btn" data-query="${esc(pattern)}">${t('try_q')} <b>${esc(pattern)}</b></button></div>
    ${ui.items.length ? `<h2 class="sub">${t('next_words')}</h2>${wordList(ui.items.slice(0, 40))}` : ''}`;
}

function renderLookup() {
  const hw = ui.current;
  rail.innerHTML = '';
  if (!hw) { sheet.innerHTML = blank(); return; }
  const all = activeDicts(), found = hits(hw), meta = HEAD[hw] ?? {};
  const saved = prefs.wordbook.some(w => w.hw === hw);
  const speakBtn = 'speechSynthesis' in window
    ? `<button class="btn" id="speak" title="${t('speak_note')}">${icon.speak}<span>${t('speak')}</span></button>` : '';
  const sections = found.map(d => {
    const folded = ui.folded.has(d.id), authored = prefs.authored.includes(d.id);
    return `<section class="dict${authored ? ' authored' : ''}" id="d-${d.id}" tabindex="-1" aria-labelledby="n-${d.id}"${folded ? ' data-folded' : ''}>
      <header class="dict-h"><h2 id="n-${d.id}">【${esc(d.name)}】</h2><span class="tag">${t('sample')}</span>
        <button class="ico tone" data-tone="${d.id}" aria-pressed="${authored}" aria-label="${t('as_authored')}" title="${t('as_authored')}">${icon.page}</button>
        <button class="ico fold" data-fold="${d.id}" aria-expanded="${!folded}" aria-controls="b-${d.id}" aria-label="${t(folded ? 'expand' : 'collapse')}">${icon.chev}</button></header>
      <div class="dict-b" id="b-${d.id}"></div></section>`;
  }).join('');
  sheet.innerHTML = `<header class="head"><h1 lang="${langOf(hw)}">【${esc(hw)}】</h1>${meta.say ? `<span class="ipa" lang="${langOf(hw)}">${esc(meta.say)}</span>` : ''}
    <span class="meta">${t('answered', found.length, all.length)}</span>
    <div class="acts">${speakBtn}<button class="btn" id="save" aria-pressed="${saved}">${icon.mark}<span>${t(saved ? 'saved' : 'save')}</span><kbd>${MOD} D</kbd></button></div></header>
    ${found.length ? sections : `<p class="lead">${t('no_group', hw)}</p>`}`;
  for (const d of found) mountEntry($('b-' + d.id), d, hw, lookupWord);
  rail.innerHTML = found.map((d, i) => `<a class="thumb" href="#d-${d.id}" style="--i:${i}" title="Alt+${i + 1}"><span>${esc(d.name)}</span><b>${esc(d.mark)}</b></a>`).join('')
    + (all.length > found.length ? `<p class="rail-none">${t('none_n', all.length - found.length)}</p>` : '');
  page.scrollTop = 0;
  syncSpy();
}

function toggleSave() {
  if (!ui.current || ui.view !== 'lookup') return;
  const added = toggleWord(ui.current, hits(ui.current)[0]?.name ?? '');
  say(t(added ? 'added' : 'removed', ui.current));
  renderPage();
  if (ui.left === 'wordbook') renderLeft();
}

function speak() {
  if (!ui.current || ui.view !== 'lookup' || !('speechSynthesis' in window)) return;
  const utterance = new SpeechSynthesisUtterance(ui.current);
  utterance.lang = { en: 'en-US', ja: 'ja-JP', zh: 'zh-CN' }[HEAD[ui.current]?.lang ?? 'en'];
  speechSynthesis.cancel();
  speechSynthesis.speak(utterance);
}

function setFold(id, folded) {
  if (folded) ui.folded.add(id); else ui.folded.delete(id);
  const section = $('d-' + id);
  if (!section) return;
  section.toggleAttribute('data-folded', folded);
  const button = section.querySelector('.fold');
  button.setAttribute('aria-expanded', String(!folded));
  button.setAttribute('aria-label', t(folded ? 'expand' : 'collapse'));
}

sheet.addEventListener('click', event => {
  const target = event.target.closest('button');
  if (!target) return;
  if (target.id === 'save') toggleSave();
  else if (target.id === 'speak') speak();
  else if (target.dataset.word) lookupWord(target.dataset.word);
  else if (target.dataset.query) { q.value = target.dataset.query; onInput(); q.focus(); }
  else if (target.dataset.goGroup) setGroup(target.dataset.goGroup);
  else if (target.dataset.proto !== undefined) say(t('proto_only'));
  else if (target.dataset.fold) setFold(target.dataset.fold, !ui.folded.has(target.dataset.fold));
  else if (target.dataset.tone) {
    const id = target.dataset.tone, at = prefs.authored.indexOf(id);
    if (at >= 0) prefs.authored.splice(at, 1); else prefs.authored.push(id);
    save('authored');
    target.setAttribute('aria-pressed', String(at < 0));
    $('d-' + id).classList.toggle('authored', at < 0);
  } else if (target.dataset.move) moveDict(target.dataset.move, Number(target.dataset.by));
});

/* ---------- thumb index: follows the scroll position ---------- */

const nativeSpy = CSS.supports('scroll-target-group: auto');

function syncSpy() {
  const links = [...rail.querySelectorAll('.thumb')];
  if (!links.length) return;
  const native = nativeSpy ? rail.querySelector('.thumb:target-current') : null;
  let current = native;
  if (!current) {
    const line = page.getBoundingClientRect().top + 64;
    const index = links.findLastIndex(a => $(a.hash.slice(1)).getBoundingClientRect().top <= line);
    current = links[Math.max(0, index)];
  }
  for (const a of links) {
    a.classList.toggle('target-current', !native && a === current);
    a.setAttribute('aria-current', a === current ? 'true' : 'false');
  }
}
page.addEventListener(nativeSpy ? 'scrollend' : 'scroll', syncSpy, { passive: true });

rail.addEventListener('click', event => {
  const link = event.target.closest('.thumb');
  if (!link) return;
  event.preventDefault();
  const section = $(link.hash.slice(1));
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  section.scrollIntoView({ behavior: calm ? 'auto' : 'smooth', block: 'start' });
  section.focus({ preventScroll: true });
});

/* ---------- search field ---------- */

function refresh(select) {
  const r = resolve(q.value);
  ui.items = r.items; ui.lemma = r.lemma; ui.near = r.near; ui.miss = r.miss;
  ui.left = 'suggest';
  // Browsing the index or looking at a miss, nothing is selected until the reader picks.
  const at = select ? r.items.indexOf(select) : -1;
  ui.sel = at >= 0 ? at : (r.browse || r.miss ? -1 : 0);
  renderLeft();
  return r.items[ui.sel] ?? null;
}

function onInput() { app.classList.add('typing'); show(refresh()); }
const settle = () => app.classList.remove('typing'); // a word was chosen: the narrow-window overlay can go

function lookupWord(hw) {
  q.value = hw;
  refresh(hw);
  show(hw, { commit: true, animate: true });
}

function move(by) {
  const items = leftItems();
  if (!items.length) return;
  ui.sel = ui.sel < 0 ? (by > 0 ? 0 : items.length - 1) : (ui.sel + by + items.length) % items.length;
  renderLeft();
  show(items[ui.sel]);
}

function go(by) {
  const next = ui.cursor + by;
  if (next < 0 || next >= ui.history.length) return;
  ui.cursor = next;
  const hw = ui.history[next];
  q.value = hw;
  refresh(hw);
  show(hw, { animate: true });
}

q.addEventListener('input', event => { if (!event.isComposing) onInput(); });
q.addEventListener('compositionend', onInput);
q.addEventListener('keydown', event => {
  if (composing(event)) return;
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') { event.preventDefault(); move(event.key === 'ArrowDown' ? 1 : -1); }
  else if (event.key === 'Escape') {
    // In a narrow window the first Esc only puts the overlay away; otherwise Esc clears the field.
    const overlay = app.classList.contains('typing') && matchMedia('(max-width: 620px)').matches;
    if (overlay || !q.value) settle(); else { q.value = ''; onInput(); settle(); }
  }
});
q.addEventListener('blur', settle);
// Enter goes through the form's own submission, which browsers hold back while an IME is composing.
$('find').addEventListener('submit', event => {
  event.preventDefault();
  const hw = leftItems()[ui.sel] ?? ui.current;
  if (hw) show(hw, { commit: true });
  settle();
});

list.addEventListener('mousedown', event => event.preventDefault()); // keep the caret in the search field
list.addEventListener('click', event => {
  const option = event.target.closest('.opt');
  if (!option) return;
  ui.sel = Number(option.dataset.i);
  const hw = leftItems()[ui.sel];
  if (ui.left !== 'suggest') q.value = hw;
  renderLeft();
  show(hw, { commit: true, animate: true });
  settle();
  setSide(false);
});

// Narrow windows: the left column is an overlay with its own toggle in the cover band.
function setSide(open) {
  app.classList.toggle('side-open', open);
  $('toggle-side').setAttribute('aria-pressed', String(open));
}
$('toggle-side').addEventListener('click', () => setSide(!app.classList.contains('side-open')));

$('back').addEventListener('click', () => go(-1));
$('fwd').addEventListener('click', () => go(1));
for (const tab of document.querySelectorAll('[data-left]')) tab.addEventListener('click', () => setLeft(tab.dataset.left));

/* ---------- library ---------- */

function toggleLibrary() {
  ui.view = ui.view === 'library' ? 'lookup' : 'library';
  if (ui.view === 'lookup') { const hw = refresh(ui.current); ui.current = hw; }
  swap(renderPage, () => { renderLeft(); if (ui.view === 'library') $('lib-h').focus(); else q.focus(); });
}

function renderLibrary() {
  rail.innerHTML = '';
  const dicts = orderedDicts();
  const rows = dicts.map((d, i) => `<tr>
    <td class="order"><button class="ico" data-move="${d.id}" data-by="-1" aria-label="${t('move_up')}：${esc(d.name)}"${i === 0 ? ' disabled' : ''}>${icon.up}</button><button class="ico" data-move="${d.id}" data-by="1" aria-label="${t('move_down')}：${esc(d.name)}"${i === dicts.length - 1 ? ' disabled' : ''}>${icon.chev}</button></td>
    <td><input type="checkbox" data-enable="${d.id}" aria-label="${esc(t('enable', d.name))}"${prefs.disabled.includes(d.id) ? '' : ' checked'}></td>
    <th scope="row"><b class="chip">${esc(d.mark)}</b>${esc(d.name)}</th>
    <td class="num">${d.entries.toLocaleString(HTML_LANG[prefs.locale])}</td><td class="num">${d.size}</td>
    ${GROUPS.map(g => `<td class="mid"><input type="checkbox" data-group="${g.id}" data-dict="${d.id}" aria-label="${esc(t('in_group', d.name, g.name))}"${prefs.membership[g.id].includes(d.id) ? ' checked' : ''}></td>`).join('')}</tr>`).join('');
  sheet.innerHTML = `<header class="head"><h1 id="lib-h" tabindex="-1">【${t('library')}】</h1><span class="meta">${t('lib_meta', DICTS.length)}</span></header>
    <div class="tools"><button class="btn" data-proto>${t('add_folder')}</button><button class="btn" data-proto>${t('rescan')}</button><button class="btn" data-proto>${t('new_group')}</button></div>
    <h2 class="sub">${t('lib_folders')}</h2>
    <ul class="folders"><li><code>D:\\Dictionaries</code><span>${t('lib_found', 5)}</span></li><li><code>D:\\Dictionaries\\日本語</code><span>${t('lib_found', 3)}</span></li></ul>
    <h2 class="sub">${t('lib_dicts')}</h2>
    <table class="tbl"><thead><tr><th scope="col">${t('col_order')}</th><th scope="col">${t('col_on')}</th><th scope="col">${t('col_name')}</th><th scope="col" class="num">${t('col_entries')}</th><th scope="col" class="num">${t('col_size')}</th>${GROUPS.map(g => `<th scope="col" class="mid">${esc(g.name)}</th>`).join('')}</tr></thead><tbody>${rows}</tbody></table>
    <p class="fine">${t('lib_note')}</p>`;
}

function moveDict(id, by) {
  const from = prefs.order.indexOf(id), to = from + by;
  if (to < 0 || to >= prefs.order.length) return;
  prefs.order.splice(to, 0, prefs.order.splice(from, 1)[0]);
  save('order');
  renderLibrary();
  sheet.querySelector(`[data-move="${id}"][data-by="${by}"]:not([disabled])`)?.focus();
}

sheet.addEventListener('change', event => {
  const box = event.target;
  if (box.dataset.enable) {
    prefs.disabled = box.checked ? prefs.disabled.filter(id => id !== box.dataset.enable) : [...prefs.disabled, box.dataset.enable];
    save('disabled');
  } else if (box.dataset.group) {
    const members = prefs.membership[box.dataset.group];
    prefs.membership[box.dataset.group] = box.checked ? [...members, box.dataset.dict] : members.filter(id => id !== box.dataset.dict);
    save('membership');
  }
  renderPage();
});

$('open-library').addEventListener('click', toggleLibrary);

/* ---------- group ---------- */

const groupSelect = $('group');
groupSelect.innerHTML = GROUPS.map(g => `<option value="${g.id}">${esc(g.name)}</option>`).join('');
groupSelect.value = prefs.group;

function setGroup(id) {
  prefs.group = id;
  save('group');
  groupSelect.value = id;
  if (ui.view === 'library') { renderPage(); return; }
  const keep = ui.current;
  const first = refresh(keep);
  show(hits(keep).length ? keep : first, { animate: true });
  renderLeft();
}
groupSelect.addEventListener('change', () => setGroup(groupSelect.value));

/* ---------- command palette ---------- */

function buildCommands() {
  const saved = prefs.wordbook.some(w => w.hw === ui.current);
  const system = t(matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
  const theme = getTheme();
  return [
    { label: t('c_focus'), hint: '/', run: () => { q.focus(); q.select(); } },
    ...(ui.current && ui.view === 'lookup' ? [{ label: t(saved ? 'c_unsave' : 'c_save'), hint: `${MOD} D`, run: toggleSave }] : []),
    ...(ui.current && ui.view === 'lookup' && 'speechSynthesis' in window ? [{ label: t('c_speak'), hint: 'Alt P', run: speak }] : []),
    ...(ui.cursor > 0 ? [{ label: t('c_back'), hint: 'Alt ←', run: () => go(-1) }] : []),
    ...(ui.cursor < ui.history.length - 1 ? [{ label: t('c_forward'), hint: 'Alt →', run: () => go(1) }] : []),
    { label: t(ui.view === 'library' ? 'c_lookup' : 'c_library'), run: toggleLibrary },
    { label: t(prefs.authored.length ? 'c_dimmed_all' : 'c_authored_all'), run: () => {
      prefs.authored = prefs.authored.length ? [] : DICTS.map(d => d.id);
      save('authored');
      renderPage();
    } },
    { label: t('c_history'), run: () => setLeft('history') },
    { label: t('c_wordbook'), run: () => setLeft('wordbook') },
    { label: t('c_mini'), run: () => { location.href = 'mini.html'; } },
    { label: t('c_fold_all'), run: () => hits(ui.current ?? '').forEach(d => setFold(d.id, true)) },
    { label: t('c_unfold_all'), run: () => hits(ui.current ?? '').forEach(d => setFold(d.id, false)) },
    ...GROUPS.map(g => ({ label: t('c_group', g.name), on: g.id === prefs.group, run: () => setGroup(g.id) })),
    { label: t('c_theme_system', system), on: theme === 'system', run: () => setTheme('system') },
    { label: t('c_theme_light'), on: theme === 'light', run: () => setTheme('light') },
    { label: t('c_theme_dark'), on: theme === 'dark', run: () => setTheme('dark') },
    ...Object.entries(LOCALES).map(([id, name]) => ({ label: t('c_lang', name), on: id === prefs.locale, run: () => setLocale(id) })),
  ];
}

function renderPalette() {
  const needle = pq.value.trim().toLowerCase();
  ui.commands = buildCommands().filter(c => c.label.toLowerCase().includes(needle));
  ui.cmdSel = Math.min(ui.cmdSel, Math.max(0, ui.commands.length - 1));
  palList.innerHTML = ui.commands.length
    ? ui.commands.map((c, i) => `<li role="option" id="cmd-${i}" class="cmd" data-i="${i}" aria-selected="${i === ui.cmdSel}"><span class="tick">${c.on ? icon.check : ''}</span><span>${esc(c.label)}</span>${c.hint ? `<kbd>${esc(c.hint)}</kbd>` : ''}</li>`).join('')
    : `<li class="cmd none">${t('pal_empty')}</li>`;
  if (ui.commands.length) pq.setAttribute('aria-activedescendant', `cmd-${ui.cmdSel}`); else pq.removeAttribute('aria-activedescendant');
  palList.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' });
}

function openPalette() {
  if (palette.open) return;
  pq.value = '';
  ui.cmdSel = 0;
  renderPalette();
  palette.showModal();
}

function runCommand(index) {
  const command = ui.commands[index];
  if (!command) return;
  palette.close();
  command.run();
}

pq.addEventListener('input', () => { ui.cmdSel = 0; renderPalette(); });
pq.addEventListener('keydown', event => {
  if (composing(event)) return;
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    if (!ui.commands.length) return;
    ui.cmdSel = (ui.cmdSel + (event.key === 'ArrowDown' ? 1 : -1) + ui.commands.length) % ui.commands.length;
    renderPalette();
  } else if (event.key === 'Enter') { event.preventDefault(); runCommand(ui.cmdSel); }
});
palList.addEventListener('mousedown', event => event.preventDefault());
palList.addEventListener('click', event => {
  const item = event.target.closest('[data-i]');
  if (item) runCommand(Number(item.dataset.i));
});
$('open-palette').addEventListener('click', openPalette);

// Click outside closes the palette where <dialog closedby> is not supported yet.
if (!('closedBy' in HTMLDialogElement.prototype)) {
  const outside = event => {
    const r = palette.getBoundingClientRect();
    return event.clientX < r.left || event.clientX > r.right || event.clientY < r.top || event.clientY > r.bottom;
  };
  let pressedOutside = false;
  palette.addEventListener('pointerdown', event => { pressedOutside = outside(event); });
  palette.addEventListener('pointerup', event => { if (pressedOutside && outside(event)) palette.close(); });
}

function setLocale(id) {
  prefs.locale = id;
  save('locale');
  applyLocale();
  renderLeft();
  renderPage();
}

/* ---------- keyboard ---------- */

document.addEventListener('keydown', event => {
  const mod = event.ctrlKey || event.metaKey, key = event.key.toLowerCase();
  if (mod && key === 'k') { event.preventDefault(); openPalette(); return; }
  if (palette.open) return;
  if ((mod && key === 'l') || (key === '/' && !event.target.closest('input, textarea'))) { event.preventDefault(); q.focus(); q.select(); }
  else if (mod && key === 'd') { event.preventDefault(); toggleSave(); }
  else if (event.altKey && key === 'p') { event.preventDefault(); speak(); }
  else if (event.altKey && event.key === 'ArrowLeft') { event.preventDefault(); go(-1); }
  else if (event.altKey && event.key === 'ArrowRight') { event.preventDefault(); go(1); }
  else if (event.altKey && /^[1-9]$/.test(event.key)) { event.preventDefault(); rail.querySelectorAll('.thumb')[event.key - 1]?.click(); }
});

/* ---------- start ---------- */

// The address can open a particular state, for demos and review captures:
// ?q=word &lang=zh|en|ja &group=read|ja|all &theme=light|dark &view=library &palette=1
// None of these are saved; they apply to this page load only.
const params = new URLSearchParams(location.search);
if (T_LOCALES.includes(params.get('lang'))) prefs.locale = params.get('lang');
if (GROUPS.some(g => g.id === params.get('group'))) { prefs.group = params.get('group'); groupSelect.value = prefs.group; }
if (['light', 'dark'].includes(params.get('theme'))) document.querySelector('meta[name="color-scheme"]').content = params.get('theme');

applyLocale();
q.value = params.get('q') ?? 'serendipities';
show(refresh(), { commit: true });
q.focus();
if (params.get('view') === 'library') { ui.view = 'library'; renderPage(); renderLeft(); }
if (params.get('palette')) openPalette();
