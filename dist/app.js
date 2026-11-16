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

