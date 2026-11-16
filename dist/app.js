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

