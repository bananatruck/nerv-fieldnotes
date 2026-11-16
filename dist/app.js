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

// section: editor lifecycle
function openEditor(note = null) {
  if (state.readonly) { toast('Storage recovery is required. Open Settings to download the original data.'); return; }
  state.editing = note?.id || null;
  const draft = loadDraft();
  const reusable = draft && draft.id === state.editing && typeof draft.title === 'string' && typeof draft.body === 'string';
  setEditor(reusable ? draft : note || {});
  $('#editor-heading').textContent = note ? 'Edit your note' : 'Capture a thought';
  $('#draft-status').textContent = reusable ? 'Recovered your unsaved draft.' : 'Changes stay in a draft until you save.';
  initialEditorValue = JSON.stringify(editorValue());
  $('#editor').showModal(); $('#note-title').focus();
}
function queueDraft() {
  clearTimeout(draftTimer);
  draftTimer = setTimeout(flushDraft, 250);
}
function flushDraft() {
  clearTimeout(draftTimer);
  if (!$('#editor').open) return;
  if (JSON.stringify(editorValue()) === initialEditorValue) return;
  try { saveDraft(editorValue()); $('#draft-status').textContent = 'Draft saved on this device.'; }
  catch { $('#draft-status').textContent = 'Draft could not save. Keep this window open and export if needed.'; }
}
function saveEditor(event) {
  event?.preventDefault(); clearTimeout(draftTimer);
  const data = editorValue();
  if (!data.title.trim() && !data.body.trim()) { toast('Add a title or a thought first.'); $('#note-title').focus(); return; }
  const fields = { title: data.title.trim(), body: data.body, tags: parseTags(data.tags), color: data.color, pinned: data.pinned, checklist: data.checklist };
  const saved = state.editing ? updateNote(state.editing, fields) : persist([...state.notes, makeNote(fields)]);
  if (saved) { try { saveDraft(null); } catch {} initialEditorValue = JSON.stringify(editorValue()); $('#editor').close(); toast('Note saved.'); }
}

// section: handoff preview
function handoffText() { return markdown(state.forwarding, $('#agent-name').value.trim() || 'Agent', $('#agent-instructions').value.trim()); }
function refreshHandoff() { if (state.forwarding) $('#handoff-preview').value = handoffText(); }
async function openForward(note) {
  state.forwarding = note; $('#agent-instructions').value = ''; refreshHandoff();
  $('#forward').showModal(); await checkLocal();
  $('#send-handoff').disabled = !state.local;
  $('#inbox-status').textContent = state.local ? 'Local inbox connected. Send a durable handoff to your agent queue.' : 'Local inbox is available in the installed app at http://127.0.0.1:4783. Copy and download work here.';
}

// section: handoff copy and download
async function copyHandoff() {
  try { await navigator.clipboard.writeText(handoffText()); toast('Handoff copied. Paste it into your agent session.'); }
  catch { $('#handoff-preview').focus(); $('#handoff-preview').select(); toast('Clipboard unavailable. The handoff is selected. Press Ctrl+C.'); }
}
function downloadHandoff() { download(`${filename(state.forwarding.title)}.md`, handoffText(), 'text/markdown'); toast('Markdown handoff downloaded.'); }

// section: local inbox discovery
async function api(path, options = {}) {
  const response = await fetch(path, { ...options, signal: AbortSignal.timeout(4000), cache: 'no-store' });
  if (!response.ok) throw new Error(`Inbox request failed (${response.status}).`);
  return response.json();
}
async function checkLocal() {
  if (!['127.0.0.1', 'localhost'].includes(location.hostname)) { state.local = false; return; }
  try { const data = await api('/api/health'); state.local = data.ok === true && data.service === 'nerv-fieldnotes'; }
  catch { state.local = false; }
}

// section: durable handoff submission
let pendingHandoff = null;
async function sendHandoff() {
  if (!state.local || !state.forwarding) return;
  const button = $('#send-handoff'); button.disabled = true;
  const signature = JSON.stringify([state.forwarding.id, handoffText()]);
  if (!pendingHandoff || pendingHandoff.signature !== signature) pendingHandoff = { signature, id: uid(), createdAt: new Date().toISOString() };
  try {
    await api('/api/inbox', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id: pendingHandoff.id, noteId: state.forwarding.id, title: state.forwarding.title || 'Untitled note', body: handoffText(), agent: $('#agent-name').value.trim() || 'Agent', createdAt: pendingHandoff.createdAt }) });
    pendingHandoff = null; $('#forward').close(); toast('Handoff queued in the local agent inbox.'); await refreshInbox();
  } catch (error) { toast(`${error.message} Try again, or copy/download the handoff.`); }
  finally { button.disabled = !state.local; }
}

// section: inbox loading
async function refreshInbox() {
  await checkLocal();
  if (state.local) {
    try { const data = await api('/api/inbox'); state.inbox = data.items; state.inboxPath = data.inboxPath; }
    catch { state.local = false; toast('Could not read local inbox. Try Refresh.'); }
  }
  if (state.view === 'inbox') render(); else renderNavigation();
}

// section: inbox presentation
function renderInbox() {
  $('#result-count').textContent = `${state.inbox.length} HANDOFFS`;
  const board = $('#notes'); board.replaceChildren();
  const intro = el('div', { class: 'inbox-intro' }, el('h2', {}, 'A clear handoff. A quieter head.'), el('p', {}, state.local ? 'Queued notes are ready for your agents to read. Acknowledged means received, not completed.' : 'The local inbox lives on your Arch Linux desktop. Open http://127.0.0.1:4783 after installing the service. Copy and download are available from any note.'), el('button', { class: 'button button-ghost', onclick: refreshInbox }, 'Refresh inbox'));
  board.append(intro);
  if (!state.local) return;
  if (!state.inbox.length) board.append(el('div', { class: 'empty-state' }, el('h2', {}, 'No handoffs yet.'), el('p', {}, 'Choose the arrow on a note to forward it to an agent.')));
  for (const item of state.inbox) {
    const card = el('article', { class: 'note inbox-item', 'data-color': item.status === 'pending' ? 'amber' : 'green' }, el('div', { class: 'note-top' }, el('span', { class: 'note-category' }, item.status.toUpperCase())), el('h2', { class: 'inbox-title' }, item.title), el('p', {}, `To: ${item.agent}`), el('pre', { class: 'note-preview' }, item.body), el('time', { datetime: item.createdAt }, relativeDate(item.createdAt)));
    if (item.status === 'pending') card.append(el('button', { class: 'button button-ghost', onclick: async () => { try { await api(`/api/inbox/${encodeURIComponent(item.id)}/ack`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{}' }); await refreshInbox(); toast('Handoff acknowledged.'); } catch (error) { toast(error.message); } } }, 'Acknowledge receipt'));
    board.append(card);
  }
}

// section: backup export and import
function exportBackup() { download(`fieldnotes-${new Date().toISOString().slice(0, 10)}.json`, JSON.stringify({ version: 1, exportedAt: new Date().toISOString(), notes: state.notes }, null, 2), 'application/json'); toast('Backup exported. It includes archived and deleted notes.'); }
async function importBackup(event) {
  const file = event.target.files[0]; if (!file) return;
  try {
    if (file.size > 20 * 1024 * 1024) throw new Error('Backup is too large. Maximum size is 20 MB.');
    const incoming = parseBackup(await file.text());
    if (persist(mergeNotes(state.notes, incoming))) toast(`Imported ${incoming.length} notes. Newer local edits were kept.`);
  } catch (error) { toast(error.message); }
  finally { event.target.value = ''; }
}

// section: settings and help dialogs
function openSettings() {
  $('#settings-inbox').textContent = state.local ? 'Local inbox connected. Only notes you explicitly forward are queued for agents.' : 'Local inbox not connected. Copy and Markdown downloads remain available.';
  $('#settings').showModal();
}
function closeDialog(dialog) { if (dialog.id === 'editor') flushDraft(); dialog.close(); }
$$('.close-dialog').forEach(button => button.addEventListener('click', () => closeDialog(button.closest('dialog'))));
$$('dialog').forEach(dialog => {
  dialog.addEventListener('cancel', () => { if (dialog.id === 'editor') flushDraft(); });
});

// section: keyboard controls
function keyboard(event) {
  const typing = /INPUT|TEXTAREA|SELECT/.test(event.target.tagName) || event.target.isContentEditable;
  const mod = event.ctrlKey || event.metaKey;
  const dialog = $('dialog[open]');
  if (mod && event.key === 'Enter' && $('#editor').open) { event.preventDefault(); saveEditor(); return; }
  if (dialog) return;
  if (mod && event.key.toLowerCase() === 'n') { event.preventDefault(); openEditor(); }
  else if (mod && ['k', '/'].includes(event.key.toLowerCase())) { event.preventDefault(); if (state.view === 'inbox') navigate('all'); $('#search').focus(); $('#search').select(); }
  else if (mod && event.key.toLowerCase() === 'z' && !typing && state.undo) { event.preventDefault(); undoMove(); }
  else if (event.key === '?' && !typing) { event.preventDefault(); $('#help').showModal(); }
  else if (event.key === 'Escape') {
    state.query = ''; state.tag = ''; $('#search').value = ''; $('.sidebar').classList.remove('open'); document.body.classList.remove('nav-open'); render();
  }
}

// section: toolbar and navigation bindings
$$('.nav-link[data-view]').forEach(button => button.addEventListener('click', () => navigate(button.dataset.view)));
$('#new-note').addEventListener('click', () => openEditor());
$('#search').addEventListener('input', event => { state.query = event.target.value; render(); });
$('#sort').addEventListener('change', event => { state.sort = event.target.value; render(); });
for (const mode of ['grid', 'list']) $(`#${mode}-view`).addEventListener('click', () => { settings.layout = mode; saveSettings(); applySettings(); });
$('#mobile-menu').addEventListener('click', () => { const open = $('.sidebar').classList.toggle('open'); document.body.classList.toggle('nav-open', open); $('#mobile-menu').setAttribute('aria-expanded', String(open)); });
$('#theme').addEventListener('change', event => { settings.theme = event.target.value; saveSettings(); applySettings(); });

// section: editor and handoff bindings
$('#editor-form').addEventListener('submit', saveEditor);
$('#editor-form').addEventListener('input', queueDraft);
$('#discard-draft').addEventListener('click', () => { clearTimeout(draftTimer); try { saveDraft(null); $('#editor').close(); toast('Draft discarded.'); } catch { toast('Could not discard draft.'); } });
$('#agent-name').addEventListener('input', refreshHandoff);
$('#agent-instructions').addEventListener('input', refreshHandoff);
$('#copy-handoff').addEventListener('click', copyHandoff);
$('#download-handoff').addEventListener('click', downloadHandoff);
$('#send-handoff').addEventListener('click', sendHandoff);

