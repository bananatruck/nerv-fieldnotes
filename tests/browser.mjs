import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtemp, mkdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
let playwright;
for (const candidate of [process.env.PLAYWRIGHT_MODULE_PATH, 'playwright'].filter(Boolean)) {
  try { playwright = require(candidate); break; } catch {}
}
if (!playwright) throw new Error('Install playwright or set PLAYWRIGHT_MODULE_PATH to its package directory.');
const temporary = await mkdtemp(join(tmpdir(), 'fieldnotes-browser-'));
const artifacts = join(root, 'artifacts');
await mkdir(artifacts, { recursive: true });
const server = spawn('python3', ['server.py', '--port', '0', '--data-dir', join(temporary, 'data')], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
let serverErrors = '';
server.stderr.on('data', bytes => { serverErrors += bytes; });
const origin = await new Promise((resolveOrigin, reject) => {
  const timer = setTimeout(() => reject(new Error(`Test runtime did not start: ${serverErrors}`)), 10000);
  let output = '';
  server.stdout.on('data', bytes => {
    output += bytes;
    if (output.includes('\n')) { clearTimeout(timer); try { resolveOrigin(JSON.parse(output.split('\n')[0]).url); } catch (error) { reject(error); } }
  });
  server.once('error', reject);
  server.once('exit', code => { if (!output) reject(new Error(`Runtime exited ${code}: ${serverErrors}`)); });
});
let browser;
const checks = [];
const errors = [];
async function check(name, run) {
  await run();
  checks.push(name);
  console.log(`PASS ${name}`);
}
try {
  browser = await playwright.chromium.launch({ headless: true, ...(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {}) });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, acceptDownloads: true, permissions: ['clipboard-read', 'clipboard-write'] });
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(origin);
  await page.locator('article.note').first().waitFor();
  const card = title => page.locator('article.note').filter({ has: page.getByRole('button', { name: `Edit ${title}`, exact: true }) });
  const nav = view => page.locator(`.sidebar button[data-view="${view}"]`).click();
  async function create(title, body, options = {}) {
    await page.locator('#new-note').click();
    await page.locator('#note-title').fill(title);
    await page.locator('#note-body').fill(body);
    if (options.tags) await page.locator('#note-tags').fill(options.tags);
    if (options.checklist) await page.locator('#note-checklist').check();
    if (options.color) await page.getByRole('button', { name: `${options.color} note`, exact: true }).click();
    await page.locator('#editor-form').getByRole('button', { name: /Save note/ }).click();
    await page.locator('#editor').waitFor({ state: 'hidden' });
    await card(title).waitFor();
  }
  await check('initial app renders and service worker activates', async () => {
    assert.equal(await page.title(), 'NERV Fieldnotes');
    assert.equal(await page.locator('article.note').count(), 4);
    await page.evaluate(async () => { await navigator.serviceWorker.ready; if (!navigator.serviceWorker.controller) await new Promise(resolveController => navigator.serviceWorker.addEventListener('controllerchange', resolveController, { once: true })); });
  });
  await check('create note with tags and color', async () => {
    await create('QA launch plan', 'Think clearly.\nShip carefully.', { tags: 'qa, work', color: 'amber' });
    assert.equal(await card('QA launch plan').getAttribute('data-color'), 'amber');
    assert.equal(await card('QA launch plan').locator('.tag').count(), 2);
  });
  await check('edit with Ctrl Enter and retain content after reload', async () => {
    await card('QA launch plan').locator('.note-title').click();
    await page.locator('#note-body').fill('Think clearly.\nShip carefully.\nEdited and persisted.');
    await page.keyboard.press('Control+Enter');
    await page.locator('#editor').waitFor({ state: 'hidden' });
    await page.reload();
    await card('QA launch plan').waitFor();
    assert.match(await card('QA launch plan').innerText(), /Edited and persisted/);
  });
  await check('Ctrl K search finds body text and Escape clears search', async () => {
    await page.keyboard.press('Control+k');
    assert.equal(await page.locator('#search').evaluate(n => n === document.activeElement), true);
    await page.keyboard.type('persisted');
    assert.equal(await page.locator('article.note').count(), 1);
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('#search').inputValue(), '');
    assert.equal(await page.locator('article.note').count(), 5);
  });
  await check('tag navigation filters and Escape clears the tag', async () => {
    await page.locator('#tag-nav').getByRole('button', { name: '# qa', exact: true }).click();
    assert.equal(await page.locator('article.note').count(), 1);
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('article.note').count(), 5);
  });
  await check('pin and unpin update the pinned workspace', async () => {
    await card('QA launch plan').getByRole('button', { name: 'Pin: QA launch plan', exact: true }).click();
    await nav('pinned');
    await card('QA launch plan').waitFor();
    await card('QA launch plan').getByRole('button', { name: 'Unpin: QA launch plan', exact: true }).click();
    assert.equal(await card('QA launch plan').count(), 0);
    await nav('all');
  });
  await check('archive and restore preserve the note', async () => {
    await card('QA launch plan').getByRole('button', { name: 'Archive: QA launch plan', exact: true }).click();
    assert.equal(await card('QA launch plan').count(), 0);
    await nav('archive');
    await card('QA launch plan').getByRole('button', { name: 'Unarchive: QA launch plan', exact: true }).click();
    await nav('all');
    await card('QA launch plan').waitFor();
  });
  await check('trash supports toast undo and Ctrl Z outside text fields', async () => {
    await card('QA launch plan').getByRole('button', { name: 'Move to trash: QA launch plan', exact: true }).click();
    await page.locator('#toast').getByRole('button', { name: 'Undo', exact: true }).click();
    await card('QA launch plan').waitFor();
    await card('QA launch plan').getByRole('button', { name: 'Move to trash: QA launch plan', exact: true }).click();
    await page.locator('h1').click();
    await page.keyboard.press('Control+z');
    await card('QA launch plan').waitFor();
  });
  await check('trash restores notes and permanent deletion requires confirmation', async () => {
    await create('QA disposable', 'Delete only this test note.');
    await card('QA disposable').getByRole('button', { name: 'Move to trash: QA disposable', exact: true }).click();
    await nav('trash');
    await card('QA disposable').getByRole('button', { name: 'Restore: QA disposable', exact: true }).click();
    await nav('all');
    await card('QA disposable').getByRole('button', { name: 'Move to trash: QA disposable', exact: true }).click();
    await nav('trash');
    await card('QA disposable').getByRole('button', { name: 'Delete forever: QA disposable', exact: true }).click();
    await page.locator('#confirm').getByRole('button', { name: 'Cancel', exact: true }).click();
    await card('QA disposable').waitFor();
    await card('QA disposable').getByRole('button', { name: 'Delete forever: QA disposable', exact: true }).click();
    await page.locator('#confirm').getByRole('button', { name: 'Delete forever', exact: true }).click();
    await page.waitForFunction(() => !document.querySelector('#confirm').open);
    await card('QA disposable').waitFor({ state: 'hidden' });
    await nav('all');
  });
  await check('checklist toggles persist across reload', async () => {
    await create('QA checklist', '- [ ] First task\n- [x] Second task', { checklist: true, color: 'green' });
    await card('QA checklist').getByRole('checkbox', { name: 'First task', exact: true }).check();
    await page.reload();
    await card('QA checklist').waitFor();
    assert.equal(await card('QA checklist').getByRole('checkbox', { name: 'First task', exact: true }).isChecked(), true);
    assert.equal(await card('QA checklist').getByRole('checkbox', { name: 'Second task', exact: true }).isChecked(), true);
  });
  await check('draft survives close and reload, and Ctrl N opens recovered draft', async () => {
    await page.locator('h1').click();
    await page.keyboard.press('Control+n');
    await page.locator('#note-title').fill('QA recovered draft');
    await page.locator('#note-body').fill('This survives without saving a note.');
    await page.keyboard.press('Escape');
    await page.reload();
    await page.locator('h1').click();
    await page.keyboard.press('Control+n');
    await page.locator('#editor').waitFor();
    assert.equal(await page.locator('#note-title').inputValue(), 'QA recovered draft');
    assert.match(await page.locator('#draft-status').innerText(), /Recovered/);
    await page.keyboard.press('Control+Enter');
    await card('QA recovered draft').waitFor();
  });
  await check('duplicate note creates an independent editable copy', async () => {
    await card('QA launch plan').getByRole('button', { name: 'Duplicate: QA launch plan', exact: true }).click();
    await card('QA launch plan (copy)').waitFor();
    assert.notEqual(await card('QA launch plan').getAttribute('data-id'), await card('QA launch plan (copy)').getAttribute('data-id'));
  });
  await check('forward copy and Markdown download contain assignment and instructions', async () => {
    await card('QA launch plan').getByRole('button', { name: 'Forward to agent: QA launch plan', exact: true }).click();
    await page.locator('#agent-name').fill('qa-agent');
    await page.locator('#agent-instructions').fill('Verify the actual result.');
    const expected = await page.locator('#handoff-preview').inputValue();
    assert.match(expected, /Assigned to: qa-agent/);
    assert.match(expected, /Verify the actual result/);
    await page.locator('#copy-handoff').click();
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), expected);
    const pendingDownload = page.waitForEvent('download');
    await page.locator('#download-handoff').click();
    const downloaded = await pendingDownload;
    assert.equal(downloaded.suggestedFilename(), 'qa-launch-plan.md');
    assert.equal(await readFile(await downloaded.path(), 'utf8'), expected);
  });
  await check('send persists a durable handoff and acknowledge updates the actual inbox', async () => {
    await page.locator('#send-handoff').waitFor({ state: 'visible' });
    await page.waitForFunction(() => !document.querySelector('#send-handoff').disabled);
    await page.locator('#send-handoff').click();
    await page.locator('#forward').waitFor({ state: 'hidden' });
    await nav('inbox');
    const inboxCard = page.locator('article.inbox-item').filter({ hasText: 'QA launch plan' });
    await inboxCard.waitFor();
    assert.match(await inboxCard.innerText(), /qa-agent/);
    const response = await context.request.get(`${origin}/api/inbox`);
    const data = await response.json();
    assert.equal(data.items.length, 1);
    assert.equal(data.items[0].status, 'pending');
    const markdownCopy = await readFile(join(temporary, 'data', 'inbox', `${data.items[0].id}.md`), 'utf8');
    assert.match(markdownCopy, /Verify the actual result/);
    await inboxCard.getByRole('button', { name: 'Acknowledge receipt' }).click();
    await page.waitForFunction(() => document.querySelector('article.inbox-item .note-category')?.textContent === 'ACKNOWLEDGED');
    const acknowledged = await (await context.request.get(`${origin}/api/inbox`)).json();
    assert.equal(acknowledged.items[0].status, 'acknowledged');
    await nav('all');
  });
  await check('JSON export captures every workspace and valid import merges by note ID', async () => {
    await page.locator('#settings-button').click();
    const pendingDownload = page.waitForEvent('download');
    await page.locator('#export').click();
    const downloaded = await pendingDownload;
    const data = JSON.parse(await readFile(await downloaded.path(), 'utf8'));
    assert.equal(data.version, 1);
    assert.ok(data.notes.some(n => n.title === 'QA launch plan'));
    const imported = { ...data.notes[0], id: 'qa-imported', title: 'QA imported backup', body: 'From a portable backup.', updatedAt: new Date().toISOString() };
    await page.locator('#import-file').setInputFiles({ name: 'backup.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ version: 1, notes: [imported] })) });
    await page.waitForFunction(() => document.querySelector('#toast').textContent.includes('Imported 1 notes'));
    await page.locator('#settings .close-dialog').click();
    await card('QA imported backup').waitFor();
  });
  await check('malformed import leaves existing notes unchanged', async () => {
    const count = await page.locator('article.note').count();
    await page.locator('#settings-button').click();
    await page.locator('#import-file').setInputFiles({ name: 'invalid.json', mimeType: 'application/json', buffer: Buffer.from('{"version":1,"notes":[{"title":"broken"}]}') });
    await page.waitForFunction(() => document.querySelector('#toast').textContent.includes('Invalid backup'));
    await page.locator('#settings .close-dialog').click();
    assert.equal(await page.locator('article.note').count(), count);
  });
  await check('light theme and list layout survive reload', async () => {
    await page.locator('#settings-button').click();
    await page.locator('#theme').selectOption('light');
    await page.locator('#settings .close-dialog').click();
    await page.locator('#list-view').click();
    await page.reload();
    await card('QA launch plan').waitFor();
    assert.equal(await page.locator('body').getAttribute('data-theme'), 'light');
    assert.equal(await page.locator('body').evaluate(n => n.classList.contains('view-list')), true);
    await page.locator('#grid-view').click();
    await page.locator('#settings-button').click();
    await page.locator('#theme').selectOption('dark');
    await page.locator('#settings .close-dialog').click();
  });
  await check('question mark opens shortcuts and Escape closes the dialog', async () => {
    await page.locator('h1').click();
    await page.keyboard.press('?');
    await page.locator('#help').waitFor();
    await page.keyboard.press('Escape');
    await page.locator('#help').waitFor({ state: 'hidden' });
  });
  await check('note text renders as inert text rather than executable HTML', async () => {
    await create('QA <img src=x onerror=alert(1)>', '<script>window.FIELDNOTES_XSS=true</script>\n<img src=x onerror="window.FIELDNOTES_XSS=true">');
    assert.equal(await page.evaluate(() => window.FIELDNOTES_XSS), undefined);
    assert.equal(await page.locator('#notes img, #notes script').count(), 0);
  });
  await check('desktop preview fits without horizontal overflow', async () => {
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.screenshot({ path: join(artifacts, 'desktop.png'), fullPage: true });
  });
  await check('mobile navigation and editor remain usable at 360 pixels', async () => {
    await page.setViewportSize({ width: 360, height: 800 });
    await page.locator('#mobile-menu').click();
    await page.locator('.sidebar.open').waitFor();
    await nav('pinned');
    await page.locator('#mobile-menu').click();
    await nav('all');
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.locator('#new-note').click();
    assert.equal(await page.evaluate(() => { const box = document.querySelector('#editor').getBoundingClientRect(); return box.left >= 0 && box.right <= innerWidth; }), true);
    await page.keyboard.press('Escape');
    await page.screenshot({ path: join(artifacts, 'mobile.png'), fullPage: true });
    await page.setViewportSize({ width: 1440, height: 1000 });
  });
  await check('offline reload serves the cached app and keeps notes editable', async () => {
    await context.setOffline(true);
    await page.reload();
    await card('QA launch plan').waitFor();
    await create('QA offline note', 'Captured without a network.');
    await context.setOffline(false);
    await page.reload();
    await card('QA offline note').waitFor();
  });
  await check('corrupt scalar and array preferences recover without breaking layout or theme controls', async () => {
    const priorErrors = errors.length;
    for (const invalid of ['3', 'true', '"dark"', '[]']) {
      await page.evaluate(value => localStorage.setItem('nerv-fieldnotes.settings', value), invalid);
      await page.reload();
      await page.locator('#list-view').click();
      assert.equal(await page.locator('body').evaluate(n => n.classList.contains('view-list')), true, `List view failed for preferences ${invalid}`);
      await page.locator('#settings-button').click();
      await page.locator('#theme').selectOption('light');
      assert.equal(await page.locator('body').getAttribute('data-theme'), 'light');
      await page.locator('#settings .close-dialog').click();
      await page.locator('#grid-view').click();
    }
    assert.equal(errors.length, priorErrors);
    await page.evaluate(() => localStorage.removeItem('nerv-fieldnotes.settings'));
    await page.reload();
  });
  await check('clean starter previews show the shipped design at desktop and mobile sizes', async () => {
    const previewContext = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
    const preview = await previewContext.newPage();
    preview.on('pageerror', error => errors.push(error.message));
    await preview.goto(origin);
    await preview.locator('article.note').first().waitFor();
    await preview.screenshot({ path: join(artifacts, 'desktop-preview.png'), fullPage: true });
    await preview.setViewportSize({ width: 360, height: 800 });
    await preview.screenshot({ path: join(artifacts, 'mobile-preview.png'), fullPage: true });
    await previewContext.close();
  });
  await check('browser reports no uncaught application errors', async () => { assert.deepEqual(errors, []); });
  console.log(JSON.stringify({ ok: true, checks: checks.length, names: checks, uncaughtErrors: errors, screenshots: ['artifacts/desktop.png', 'artifacts/mobile.png'], isolatedInbox: true }, null, 2));
  await context.close();
} catch (error) {
  console.error(`FAIL after ${checks.length} successful browser checks: ${error.stack}`);
  throw error;
} finally {
  if (browser) await browser.close();
  server.kill('SIGTERM');
  await new Promise(resolveExit => { if (server.exitCode !== null) resolveExit(); else server.once('exit', resolveExit); });
  await rm(temporary, { recursive: true, force: true });
}
