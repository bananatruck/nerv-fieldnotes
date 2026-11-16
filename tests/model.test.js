import test from 'node:test';
import assert from 'node:assert/strict';
import { COLORS, VIEWS, makeNote, validateNote, parseBackup, mergeNotes, filterNotes, parseTags, toggleTask, markdown } from '../dist/lib/model.js';

const note = fields => makeNote({ createdAt: '2026-11-15T09:00:00Z', updatedAt: '2026-11-15T10:00:00Z', ...fields });
const ids = notes => notes.map(n => n.id);

test('new notes have unique IDs and valid editable defaults', () => {
  const a = makeNote();
  const b = makeNote();
  assert.notEqual(a.id, b.id);
  assert.ok(validateNote(a));
  assert.equal(a.status, 'active');
  assert.equal(a.title, '');
  assert.equal(a.body, '');
  assert.equal(a.pinned, false);
  assert.equal(a.checklist, false);
  assert.deepEqual(a.tags, []);
});

test('supported colors and workspace views remain available', () => {
  for (const color of ['paper', 'amber', 'red', 'green', 'purple']) assert.ok(COLORS.includes(color));
  for (const view of ['all', 'pinned', 'archive', 'trash', 'inbox']) assert.ok(VIEWS.includes(view));
});

test('note validation rejects malformed IDs and unrecognized state', () => {
  for (const bad of [{ id: '' }, { id: '../escape' }, { id: 'x'.repeat(101) }, { color: 'blue' }, { status: 'gone' }, { pinned: 'yes' }, { checklist: 1 }]) {
    assert.ok(!validateNote(note(bad)), JSON.stringify(bad));
  }
  for (const bad of [null, undefined, [], 7, 'note']) assert.ok(!validateNote(bad));
});

test('note validation bounds text and tags before import', () => {
  assert.ok(validateNote(note({ title: 't'.repeat(500), body: 'b'.repeat(100000), tags: Array(20).fill('t'.repeat(40)) })));
  for (const bad of [{ title: 't'.repeat(501) }, { body: 'b'.repeat(100001) }, { tags: Array(21).fill('x') }, { tags: ['x'.repeat(41)] }, { tags: [1] }, { tags: 'work' }, { body: null }]) {
    assert.ok(!validateNote(note(bad)));
  }
});

test('note validation rejects unusable timestamps', () => {
  for (const bad of [{ createdAt: 'yesterday' }, { updatedAt: null }, { updatedAt: 123 }]) assert.ok(!validateNote(note(bad)));
});

test('backup parser accepts serialized version one exports and empty backups', () => {
  const notes = [note({ title: 'Keep me' })];
  assert.deepEqual(parseBackup(JSON.stringify({ version: 1, notes })), notes);
  assert.deepEqual(parseBackup({ version: 1, notes: [] }), []);
});

test('backup parser rejects malformed JSON, unsupported versions, and invalid note bodies', () => {
  for (const invalid of ['{', '{}', 'null', JSON.stringify({ version: 2, notes: [] }), JSON.stringify({ version: 1, notes: [{ title: 'incomplete' }] }), JSON.stringify({ version: 1, notes: 'wrong' })]) {
    assert.throws(() => parseBackup(invalid));
  }
});

test('backup parser rejects duplicate note IDs rather than dropping content', () => {
  const original = note({ id: 'same-id', body: 'first' });
  assert.throws(() => parseBackup({ version: 1, notes: [original, { ...original, body: 'second' }] }), /Invalid backup/);
});

test('backup parser rejects oversized note counts', () => {
  assert.throws(() => parseBackup({ version: 1, notes: Array(10001).fill(note()) }), /Invalid backup/);
});

test('import merge retains newer local edits and adds new notes', () => {
  const local = note({ id: 'existing', title: 'local', updatedAt: '2026-11-16T10:00:00Z' });
  const older = { ...local, title: 'old import', updatedAt: '2026-11-15T10:00:00Z' };
  const added = note({ id: 'new', title: 'new import' });
  assert.deepEqual(mergeNotes([local], [older, added]), [local, added]);
});

test('import merge accepts newer edits but retains local content on equal timestamps', () => {
  const local = note({ id: 'existing', title: 'local' });
  const equal = { ...local, title: 'same time' };
  assert.deepEqual(mergeNotes([local], [equal]), [local]);
  const newer = { ...local, title: 'newer', updatedAt: '2026-11-16T10:00:00Z' };
  assert.deepEqual(mergeNotes([local], [newer]), [newer]);
});

test('workspace views isolate active, pinned, archived, and deleted notes', () => {
  const notes = [note({ id: 'active' }), note({ id: 'pinned', pinned: true }), note({ id: 'archive', status: 'archived', pinned: true }), note({ id: 'trash', status: 'deleted' })];
  assert.deepEqual(ids(filterNotes(notes)), ['pinned', 'active']);
  assert.deepEqual(ids(filterNotes(notes, { view: 'pinned' })), ['pinned']);
  assert.deepEqual(ids(filterNotes(notes, { view: 'archive' })), ['archive']);
  assert.deepEqual(ids(filterNotes(notes, { view: 'trash' })), ['trash']);
});

test('search matches title, body, and tags case-insensitively using every term', () => {
  const notes = [note({ id: 'a', title: 'Build the engine', body: 'Check rendering', tags: ['Work'] }), note({ id: 'b', title: 'Build lunch', tags: ['personal'] })];
  assert.deepEqual(ids(filterNotes(notes, { query: '  BUILD   work  ' })), ['a']);
  assert.deepEqual(ids(filterNotes(notes, { query: 'rendering' })), ['a']);
  assert.deepEqual(ids(filterNotes(notes, { query: 'missing' })), []);
});

test('tag filters combine with search and respect workspace status', () => {
  const notes = [note({ id: 'a', title: 'Ship', tags: ['work'] }), note({ id: 'b', title: 'Ship', tags: ['workshop'] }), note({ id: 'c', title: 'Ship', tags: ['work'], status: 'archived' })];
  assert.deepEqual(ids(filterNotes(notes, { query: 'ship', tag: 'work' })), ['a']);
  assert.deepEqual(ids(filterNotes(notes, { view: 'archive', tag: 'work' })), ['c']);
});

test('sorting puts pinned notes first then uses requested ordering without mutating input', () => {
  const notes = [note({ id: 'z', title: 'Zulu', updatedAt: '2026-11-16T12:00:00Z', createdAt: '2026-11-15T12:00:00Z' }), note({ id: 'a', title: 'Alpha', updatedAt: '2026-11-15T12:00:00Z', createdAt: '2026-11-16T12:00:00Z' }), note({ id: 'p', title: 'Pinned', pinned: true })];
  assert.deepEqual(ids(filterNotes(notes, { sort: 'updated' })), ['p', 'z', 'a']);
  assert.deepEqual(ids(filterNotes(notes, { sort: 'created' })), ['p', 'a', 'z']);
  assert.deepEqual(ids(filterNotes(notes, { sort: 'title' })), ['p', 'a', 'z']);
  assert.deepEqual(ids(notes), ['z', 'a', 'p']);
});

test('tag input strips leading hashes, trims, and removes duplicates and blanks', () => {
  assert.deepEqual(parseTags(' work, #idea,work, , personal ,#idea'), ['work', 'idea', 'personal']);
  assert.deepEqual(parseTags(''), []);
});

test('tag input enforces the same length and count limits as note validation', () => {
  const tags = parseTags(Array.from({ length: 25 }, (_, i) => `${i}-${'x'.repeat(50)}`).join(','));
  assert.equal(tags.length, 20);
  assert.ok(tags.every(tag => tag.length <= 40));
  assert.ok(validateNote(note({ tags })));
});

test('task toggles preserve all other lines and can uncheck existing tasks', () => {
  const before = 'Heading\n- [ ] First\n- [X] Second\nTail';
  assert.equal(toggleTask(before, 1), 'Heading\n- [x] First\n- [X] Second\nTail');
  assert.equal(toggleTask(before, 2), 'Heading\n- [ ] First\n- [ ] Second\nTail');
  assert.equal(toggleTask(before, 99), before);
});

test('plain checklist lines can be completed while preserving indentation on Markdown tasks', () => {
  assert.equal(toggleTask('Get groceries\n  - [ ] Bread', 0), '- [x] Get groceries\n  - [ ] Bread');
  assert.equal(toggleTask('  - [ ] Bread', 0), '  - [x] Bread');
});

test('Markdown handoff carries exact note text, assignment, identifiers, and instructions', () => {
  const original = note({ id: 'handoff-note', title: 'Build it', body: 'A <b>literal</b> idea\nSecond line', tags: ['work', 'agents'] });
  const result = markdown(original, 'build-agent', 'Run the tests.');
  assert.ok(result.startsWith('# Build it\n\nA <b>literal</b> idea\nSecond line\n'));
  for (const text of ['Assigned to: build-agent', 'Tags: work, agents', 'Note ID: handoff-note', `Updated: ${original.updatedAt}`, 'Instructions:\nRun the tests.']) assert.ok(result.includes(text));
});

test('Markdown handoff provides defaults for untitled notes and absent tags', () => {
  const result = markdown(note());
  assert.ok(result.startsWith('# Untitled note\n'));
  assert.ok(result.includes('Assigned to: Agent'));
  assert.ok(result.includes('Tags: none'));
  assert.ok(!result.includes('Instructions:'));
});
