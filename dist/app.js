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

