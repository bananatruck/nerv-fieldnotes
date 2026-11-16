import { parseBackup, makeNote } from './model.js';
const KEY = 'nerv-fieldnotes.v1';
export function loadNotes(storage = localStorage) {
  const raw = storage.getItem(KEY);
  return raw === null ? null : parseBackup(raw);
}
export function saveNotes(notes, storage = localStorage) {
  storage.setItem(KEY, JSON.stringify({ version: 1, notes }));
}
export function loadDraft(storage = localStorage) {
  try { return JSON.parse(storage.getItem('nerv-fieldnotes.draft') || 'null'); } catch { return null; }
}
export function saveDraft(draft, storage = localStorage) {
  if (draft) storage.setItem('nerv-fieldnotes.draft', JSON.stringify(draft));
  else storage.removeItem('nerv-fieldnotes.draft');
}
export function sampleNotes() {
  return [
    makeNote({ title: 'A place for the things in your head.', body: 'One thought, one note. Capture it here, then get back to what matters.\n\nEdit this note or make a new one. Everything saves in this browser.', tags: ['start-here'], color: 'paper', pinned: true }),
    makeNote({ title: 'Your agents have an inbox.', body: 'Open a note and choose Forward. Copy a clean handoff, download Markdown, or send it to the local inbox.\n\nYou stay in control of what gets sent.', tags: ['agents'], color: 'amber' }),
    makeNote({ title: 'Before you call it a day', body: '- [ ] Get the loose thoughts down\n- [ ] Pick one thing for tomorrow\n- [x] Give your brain a break', checklist: true, tags: ['personal'], color: 'green' }),
    makeNote({ title: 'A few keys you know.', body: 'Ctrl + N    New note\nCtrl + K    Find anything\nCtrl + Enter    Save and close\nEscape    Close a dialog\n?    All shortcuts\n\nClick any title to edit.', tags: ['shortcuts'], color: 'paper' })
  ];
}
