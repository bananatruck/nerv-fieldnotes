import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright');
const baseURL = process.env.FIELDNOTES_URL || 'http://127.0.0.1:4783';
const browser = await chromium.launch({
  headless: true,
  ...((process.env.CHROMIUM_PATH || process.env.BROWSER_EXECUTABLE) ? { executablePath: process.env.CHROMIUM_PATH || process.env.BROWSER_EXECUTABLE } : {})
});
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  await page.addInitScript(() => {
    if (localStorage.getItem('layout-test-initialized')) return;
    const now = new Date().toISOString();
    localStorage.setItem('nerv-fieldnotes.v1', JSON.stringify({ version: 1, notes: [{
      id: 'layout-checklist', title: 'Before you call it a day',
      body: '- [ ] Get the loose thoughts down\n- [ ] Pick one thing for tomorrow\n- [x] Give your brain a break',
      tags: ['personal'], color: 'green', pinned: false, status: 'active',
      createdAt: now, updatedAt: now, checklist: true
    }] }));
    localStorage.setItem('layout-test-initialized', 'yes');
  });
  await page.goto(baseURL);
  await page.locator('.task').first().waitFor();
  const rows = await page.locator('.task').evaluateAll(tasks => tasks.map(task => ({
    display: getComputedStyle(task).display,
    top: task.getBoundingClientRect().top,
    bottom: task.getBoundingClientRect().bottom
  })));
  assert.equal(rows.length, 3, 'The checklist renders all three tasks');
  assert.equal(rows[0].display, 'flex', 'Each checklist task has its own flex row');
  for (let index = 1; index < rows.length; index++) {
    assert.ok(rows[index].top >= rows[index - 1].bottom, 'Checklist rows do not overlap');
  }
  await page.locator('.task input').first().check();
  await page.reload();
  assert.equal(await page.locator('.task input').first().isChecked(), true, 'A toggled task survives reload');
  for (const width of [1440, 1024, 768, 640, 360]) {
    await page.setViewportSize({ width, height: 1000 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, `No overflow at ${width}px`);
    await page.locator('#list-view').click();
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, `No list overflow at ${width}px`);
    await page.locator('#grid-view').click();
  }
  await page.setViewportSize({ width: 360, height: 900 });
  await page.locator('#new-note').click();
  assert.equal(await page.locator('#editor').evaluate(dialog => dialog.open), true);
  assert.equal(await page.locator('#editor').evaluate(dialog => dialog.getBoundingClientRect().width <= innerWidth), true);
  await page.locator('#editor .close-dialog').click();
  await mkdir(new URL('../artifacts/', import.meta.url), { recursive: true });
  await page.screenshot({ path: new URL('../artifacts/layout-mobile.png', import.meta.url).pathname, fullPage: true });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.screenshot({ path: new URL('../artifacts/layout-desktop.png', import.meta.url).pathname, fullPage: true });
  console.log('PASS checklist rows, durable toggle, five responsive widths, grid/list and mobile editor');
} finally {
  await browser.close();
}
