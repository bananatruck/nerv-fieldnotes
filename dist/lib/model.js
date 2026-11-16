export const COLORS = ['paper', 'amber', 'red', 'green', 'purple'];
export const VIEWS = ['all', 'pinned', 'archive', 'trash', 'inbox'];
export const uid = () => crypto.randomUUID();
export function makeNote(fields = {}) {
  const now = new Date().toISOString();
  return { id: uid(), title: '', body: '', tags: [], color: 'paper', pinned: false,
    status: 'active', createdAt: now, updatedAt: now, checklist: false, ...fields };
}
export function validateNote(n) {
  return n && typeof n === 'object' && typeof n.id === 'string' && /^[\w-]{1,100}$/.test(n.id)
    && typeof n.title === 'string' && n.title.length <= 500 && typeof n.body === 'string' && n.body.length <= 100000
    && Array.isArray(n.tags) && n.tags.length <= 20 && n.tags.every(t => typeof t === 'string' && t.length <= 40)
    && COLORS.includes(n.color) && ['active', 'archived', 'deleted'].includes(n.status)
    && typeof n.pinned === 'boolean' && typeof n.checklist === 'boolean'
    && typeof n.createdAt === 'string' && Number.isFinite(Date.parse(n.createdAt))
    && typeof n.updatedAt === 'string' && Number.isFinite(Date.parse(n.updatedAt));
}
export function parseBackup(raw) {
  const data = typeof raw === 'string' ? JSON.parse(raw) : raw;
  if (!data || data.version !== 1 || !Array.isArray(data.notes) || data.notes.length > 10000
    || !data.notes.every(validateNote) || new Set(data.notes.map(n => n.id)).size !== data.notes.length) {
    throw new Error('Invalid backup. Select a Fieldnotes version 1 JSON export.');
  }
  return data.notes;
}
export function mergeNotes(existing, incoming) {
  const byId = new Map(existing.map(n => [n.id, n]));
  for (const note of incoming) {
    const old = byId.get(note.id);
    if (!old || Date.parse(note.updatedAt) > Date.parse(old.updatedAt)) byId.set(note.id, note);
  }
  return [...byId.values()];
}
export function filterNotes(notes, { view = 'all', query = '', tag = '', sort = 'updated' } = {}) {
  const terms = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
  const filtered = notes.filter(n => {
    const inView = view === 'trash' ? n.status === 'deleted' : view === 'archive' ? n.status === 'archived'
      : n.status === 'active' && (view !== 'pinned' || n.pinned);
    const haystack = [n.title, n.body, ...n.tags].join(' ').toLowerCase();
    return inView && (!tag || n.tags.includes(tag)) && terms.every(t => haystack.includes(t));
  });
  return filtered.sort((a, b) => Number(b.pinned) - Number(a.pinned) || (sort === 'title'
    ? (a.title || 'Untitled').localeCompare(b.title || 'Untitled')
    : Date.parse(b[sort === 'created' ? 'createdAt' : 'updatedAt']) - Date.parse(a[sort === 'created' ? 'createdAt' : 'updatedAt'])));
}
export function parseTags(raw) {
  return [...new Set(raw.split(',').map(t => t.trim().replace(/^#/, '').slice(0, 40)).filter(Boolean))].slice(0, 20);
}
export function toggleTask(body, index) {
  return body.split('\n').map((line, i) => i === index ? (/^\s*- \[x\]/i.test(line)
    ? line.replace(/\[x\]/i, '[ ]') : /^\s*- \[ \]/.test(line) ? line.replace('[ ]', '[x]') : `- [x] ${line}`) : line).join('\n');
}
export function markdown(note, agent = 'Agent', instructions = '') {
  return `# ${note.title || 'Untitled note'}\n\n${note.body}\n\n---\nAssigned to: ${agent}\nTags: ${note.tags.join(', ') || 'none'}\nNote ID: ${note.id}\nUpdated: ${note.updatedAt}\n${instructions ? `\nInstructions:\n${instructions}\n` : ''}`;
}
