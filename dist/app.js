// section: application state
import { makeNote, validateNote, filterNotes, parseTags, parseBackup, mergeNotes, toggleTask, markdown, uid, COLORS } from './lib/model.js';
import { loadNotes, saveNotes, sampleNotes, loadDraft, saveDraft } from './lib/storage.js';
import { $, $$, el, download, filename, relativeDate } from './lib/dom.js';
const state = { notes: [], view: 'all', query: '', tag: '', sort: 'updated', editing: null, color: 'paper', forwarding: null, inbox: [], local: false, readonly: false, undo: null };
let toastTimer;
let draftTimer;
let initialEditorValue = '';
let settings = {};
try { settings = JSON.parse(localStorage.getItem('nerv-fieldnotes.settings') || '{}') || {}; } catch {}

// section: feedback and persistence
function toast(message, action) {
  clearTimeout(toastTimer);
  const node = $('#toast'); node.replaceChildren(el('span', {}, message));
  if (action) node.append(el('button', { class: 'button button-ghost', onclick: action.run }, action.label));
  node.classList.remove('hidden');
  toastTimer = setTimeout(() => node.classList.add('hidden'), action ? 10000 : 5500);
}
function persist(next) {
  if (state.readonly) { toast('Storage could not be read. Export the original data before making changes.'); return false; }
  try {
    saveNotes(next); state.notes = next; $('#save-state').textContent = 'ALL CHANGES SAVED';
    render(); return true;
  } catch { $('#save-state').textContent = 'SAVE FAILED'; toast('Could not save. Browser storage may be full. Export your notes before closing.'); return false; }
}
function updateNote(id, fields) {
  return persist(state.notes.map(n => n.id === id ? { ...n, ...fields, updatedAt: new Date().toISOString() } : n));
}

// section: settings persistence
function saveSettings() {
  try { localStorage.setItem('nerv-fieldnotes.settings', JSON.stringify(settings)); }
  catch { toast('Could not save preferences.'); }
}
function applySettings() {
  document.body.dataset.theme = settings.theme === 'light' ? 'light' : 'dark';
  document.body.classList.toggle('view-list', settings.layout === 'list');
  $('#theme').value = document.body.dataset.theme;
  for (const mode of ['grid', 'list']) {
    const active = (settings.layout || 'grid') === mode;
    $(`#${mode}-view`).classList.toggle('active', active);
    $(`#${mode}-view`).setAttribute('aria-pressed', String(active));
  }
}

// section: navigation
function navigate(view, tag = '') {
  state.view = view; state.tag = tag;
  if (view === 'inbox') refreshInbox();
  $('.sidebar').classList.remove('open'); document.body.classList.remove('nav-open');
  $('#search').disabled = view === 'inbox';
  $('#sort').disabled = view === 'inbox';
  render();
}
function renderNavigation() {
  const count = status => state.notes.filter(n => n.status === status).length;
  $('#count-all').textContent = count('active'); $('#count-archive').textContent = count('archived');
  $('#count-trash').textContent = count('deleted');
  $('#count-pinned').textContent = state.notes.filter(n => n.status === 'active' && n.pinned).length;
  $('#count-inbox').textContent = state.inbox.filter(n => n.status === 'pending').length;
  $('#active-count').textContent = String(count('active')).padStart(2, '0');
  const title = { all: 'All notes', pinned: 'Pinned', archive: 'Archive', trash: 'Trash', inbox: 'Agent inbox' }[state.view];
  $('#view-label').textContent = title.toUpperCase();
  $('#section-label').textContent = state.tag ? `TAG / ${state.tag}` : title === 'All notes' ? 'YOUR NOTES' : title.toUpperCase();
  $$('.nav-link[data-view]').forEach(b => { const selected = b.dataset.view === state.view && !state.tag; b.classList.toggle('active', selected); if (selected) b.setAttribute('aria-current', 'page'); else b.removeAttribute('aria-current'); });
  const tags = [...new Set(state.notes.filter(n => n.status === 'active').flatMap(n => n.tags))].sort();
  $('#tag-nav').replaceChildren(...tags.map(tag => el('button', { class: `nav-link${state.tag === tag ? ' active' : ''}`, onclick: () => navigate('all', state.tag === tag ? '' : tag) }, `# ${tag}`)));
}

// section: note task rendering
function notePreview(note) {
  if (!note.checklist) return el('p', { class: 'note-preview' }, note.body || 'An empty note. Make it yours.');
  const list = el('div', { class: 'note-preview task-list' });
  note.body.split('\n').slice(0, 6).forEach((line, index) => {
    if (!line.trim()) return;
    const checked = /^\s*- \[x\]/i.test(line);
    const checkbox = el('input', { type: 'checkbox', 'aria-label': line.replace(/^\s*- \[[ x]\]\s*/i, '') || 'Task', onchange: () => updateNote(note.id, { body: toggleTask(note.body, index) }) });
    checkbox.checked = checked; checkbox.disabled = note.status !== 'active';
    list.append(el('label', { class: checked ? 'task checked' : 'task' }, checkbox, el('span', {}, line.replace(/^\s*- \[[ x]\]\s*/i, ''))));
  });
  return list;
}

// section: note card rendering
function noteCard(note) {
  const action = (label, symbol, fn, extra = '') => el('button', { class: `icon-button ${extra}`, title: label, 'aria-label': `${label}: ${note.title || 'Untitled'}`, onclick: fn }, symbol);
  const tools = el('div', { class: 'note-tools' });
  if (note.status === 'active') tools.append(action(note.pinned ? 'Unpin' : 'Pin', '⌖', () => updateNote(note.id, { pinned: !note.pinned }), note.pinned ? 'is-pinned' : ''));
  const actions = el('div', { class: 'note-actions' });
  if (note.status === 'deleted') actions.append(action('Restore', '↶', () => move(note, 'active')), action('Delete forever', '×', () => deleteForever(note)));
  else {
    actions.append(action('Forward to agent', '↗', () => openForward(note)));
    actions.append(action('Duplicate', '⧉', () => duplicate(note)));
    actions.append(action(note.status === 'archived' ? 'Unarchive' : 'Archive', note.status === 'archived' ? '↶' : '▤', () => move(note, note.status === 'archived' ? 'active' : 'archived')));
    actions.append(action('Move to trash', '×', () => move(note, 'deleted')));
  }
  return el('article', { class: `note${note.pinned ? ' pinned' : ''}`, 'data-color': note.color, 'data-id': note.id },
    el('div', { class: 'note-top' }, el('span', { class: 'note-category' }, note.checklist ? 'CHECKLIST' : 'FIELD NOTE'), tools),
    el('button', { class: 'note-title', onclick: () => openEditor(note), 'aria-label': `Edit ${note.title || 'Untitled note'}` }, note.title || 'Untitled note'),
    notePreview(note), el('div', { class: 'note-tags' }, ...note.tags.map(t => el('button', { class: 'tag', onclick: () => navigate('all', t) }, `#${t}`))),
    el('div', { class: 'note-bottom' }, el('time', { class: 'note-date', datetime: note.updatedAt, title: new Date(note.updatedAt).toLocaleString() }, relativeDate(note.updatedAt)), actions));
}

// section: board rendering
function render() {
  renderNavigation();
  if (state.view === 'inbox') { renderInbox(); return; }
  const notes = filterNotes(state.notes, state);
  $('#result-count').textContent = `${String(notes.length).padStart(2, '0')} ${notes.length === 1 ? 'NOTE' : 'NOTES'}`;
  const board = $('#notes'); board.replaceChildren(...notes.map(noteCard));
  if (!notes.length) {
    const title = state.query || state.tag ? 'No matching thoughts.' : state.view === 'trash' ? 'Nothing in the trash.' : state.view === 'archive' ? 'A little breathing room.' : state.view === 'pinned' ? 'Keep something close.' : 'Start with one thought.';
    board.append(el('div', { class: 'empty-state' }, el('span', { class: 'empty-mark', 'aria-hidden': 'true' }, '+'), el('h2', {}, title), el('p', {}, state.query || state.tag ? 'Try another search or clear the filters.' : state.view === 'all' ? 'A reminder, a plan, a half-formed idea. Put it here.' : 'Notes you move here will appear in this view.'),
      el('button', { class: 'button button-primary', onclick: () => { if (state.query || state.tag) { state.query = ''; state.tag = ''; $('#search').value = ''; render(); } else openEditor(); } }, state.query || state.tag ? 'Clear filters' : 'New note')));
  }
}

// section: moving and undo
function move(note, status) {
  const previous = note.status;
  if (updateNote(note.id, { status })) {
    state.undo = { id: note.id, status: previous };
    toast(status === 'deleted' ? 'Note moved to trash.' : status === 'archived' ? 'Note archived.' : 'Note restored.', { label: 'Undo', run: undoMove });
  }
}
function undoMove() {
  if (!state.undo) return;
  const { id, status } = state.undo;
  if (updateNote(id, { status })) { state.undo = null; toast('Move undone.'); }
}
function duplicate(note) {
  if (persist([...state.notes, makeNote({ title: `${note.title || 'Untitled'} (copy)`, body: note.body, tags: [...note.tags], color: note.color, checklist: note.checklist })])) toast('Note duplicated.');
}
async function deleteForever(note) {
  const dialog = $('#confirm'); dialog.returnValue = ''; dialog.showModal();
  await new Promise(resolve => dialog.addEventListener('close', resolve, { once: true }));
  if (dialog.returnValue === 'delete' && persist(state.notes.filter(n => n.id !== note.id))) { state.undo = null; toast('Note permanently deleted.'); }
}

// section: editor data
function editorValue() {
  return { id: state.editing, title: $('#note-title').value, body: $('#note-body').value, tags: $('#note-tags').value, color: state.color, pinned: $('#note-pinned').checked, checklist: $('#note-checklist').checked };
}
function setEditor(data) {
  $('#note-title').value = data.title || ''; $('#note-body').value = data.body || '';
  $('#note-tags').value = Array.isArray(data.tags) ? data.tags.join(', ') : data.tags || '';
  $('#note-pinned').checked = Boolean(data.pinned); $('#note-checklist').checked = Boolean(data.checklist);
  state.color = COLORS.includes(data.color) ? data.color : 'paper'; renderColors();
}
function renderColors() {
  $('#note-colors').replaceChildren(...COLORS.map(color => el('button', { type: 'button', class: `color-option${state.color === color ? ' selected' : ''}`, 'data-color': color, 'aria-label': `${color} note`, 'aria-pressed': String(state.color === color), title: color, onclick: () => { state.color = color; renderColors(); queueDraft(); } }, state.color === color ? '✓' : '')));
}

