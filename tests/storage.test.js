import test from 'node:test';
import assert from 'node:assert/strict';
import { loadNotes, saveNotes, loadDraft, saveDraft, sampleNotes } from '../dist/lib/storage.js';
import { makeNote, validateNote } from '../dist/lib/model.js';

function memoryStorage() {
  const entries = new Map();
  return {
    getItem: key => entries.has(key) ? entries.get(key) : null,
    setItem: (key, value) => entries.set(key, value),
    removeItem: key => entries.delete(key),
  };
}

test('first launch returns null while an intentionally empty workspace stays empty', () => {
  const storage = memoryStorage();
  assert.equal(loadNotes(storage), null);
  saveNotes([], storage);
  assert.deepEqual(loadNotes(storage), []);
});

test('saved notes round-trip all editable state through versioned JSON', () => {
  const storage = memoryStorage();
  const notes = [makeNote({ title: 'Persist me', body: 'line 1\nline 2', tags: ['work'], pinned: true, checklist: true, status: 'archived', color: 'green' })];
  saveNotes(notes, storage);
  assert.deepEqual(loadNotes(storage), notes);
  assert.equal(JSON.parse(storage.getItem('nerv-fieldnotes.v1')).version, 1);
});

test('corrupt persisted notes fail visibly and remain available for recovery', () => {
  const storage = memoryStorage();
  const corrupt = '{ broken notes';
  storage.setItem('nerv-fieldnotes.v1', corrupt);
  assert.throws(() => loadNotes(storage));
  assert.equal(storage.getItem('nerv-fieldnotes.v1'), corrupt);
});

test('invalid persisted note schemas do not silently become an empty workspace', () => {
  const storage = memoryStorage();
  storage.setItem('nerv-fieldnotes.v1', JSON.stringify({ version: 1, notes: [{ title: 'broken' }] }));
  assert.throws(() => loadNotes(storage), /Invalid backup/);
});

test('storage quota and access errors propagate so the interface can report unsaved work', () => {
  const quotaError = new Error('QuotaExceededError');
  assert.throws(() => saveNotes([makeNote()], { setItem() { throw quotaError; } }), error => error === quotaError);
  const securityError = new Error('SecurityError');
  assert.throws(() => loadNotes({ getItem() { throw securityError; } }), error => error === securityError);
});

test('draft saves are independent from saved notes and can be explicitly cleared', () => {
  const storage = memoryStorage();
  const notes = [makeNote({ title: 'Saved' })];
  const draft = { title: 'Not saved yet', body: 'Draft text', id: null };
  saveNotes(notes, storage);
  saveDraft(draft, storage);
  assert.deepEqual(loadDraft(storage), draft);
  assert.deepEqual(loadNotes(storage), notes);
  saveDraft(null, storage);
  assert.equal(loadDraft(storage), null);
  assert.deepEqual(loadNotes(storage), notes);
});

test('corrupt drafts do not prevent opening the application', () => {
  const storage = memoryStorage();
  storage.setItem('nerv-fieldnotes.draft', '{ invalid');
  assert.equal(loadDraft(storage), null);
});

test('draft quota errors propagate rather than pretending that a draft was saved', () => {
  assert.throws(() => saveDraft({ title: 'Draft' }, { setItem() { throw new Error('Quota exceeded'); } }), /Quota exceeded/);
});

test('starter notes are valid and have distinct IDs on each fresh workspace', () => {
  const first = sampleNotes();
  const second = sampleNotes();
  assert.ok(first.length > 0);
  assert.ok(first.every(validateNote));
  assert.equal(new Set(first.map(n => n.id)).size, first.length);
  assert.ok(first.every(n => second.every(other => n.id !== other.id)));
  assert.ok(first.some(n => n.checklist));
  assert.ok(first.some(n => n.pinned));
});
