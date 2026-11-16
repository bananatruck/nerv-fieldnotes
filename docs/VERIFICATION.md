# Verification

## Automated checks

The release was checked on this Arch Linux workstation with Node, Python 3.14, and Chromium.

| Check | Result |
| --- | --- |
| Note model and browser storage unit tests | 30 passed |
| Real HTTP, SQLite, CLI, and installer integration tests | 16 passed |
| Browser interaction scenarios | 25 passed |
| Checklist and responsive layout regression | Passed |
| Browser uncaught application errors | 0 |
| Installed systemd user service | Enabled and active |
| Local health endpoint | Returned service `nerv-fieldnotes` |

The browser suite starts a real server on an ephemeral loopback port and uses a temporary SQLite directory.
It runs in an isolated browser context.
The clipboard, downloaded files, inbox records, Markdown snapshot, acknowledgement, and persisted notes are read back from their actual outputs.
It tests offline reload and editing through the service worker.
It also confirms that user HTML stays inert.

The layout regression checks grid and list views at 1440, 1024, 768, 640, and 360 pixels.
It checks checklist row separation, persisted completion, and editor containment on mobile.
Desktop and mobile screenshots were inspected.

Two defects were reproduced with failing browser checks before being fixed.
Checklist items initially flowed inline instead of occupying separate rows.
Malformed scalar preference data initially broke the theme and layout controls.
Both regressions now pass.

## Run the checks

```sh
npm test
node --check dist/app.js
node --check dist/sw.js
```

When Playwright and its browser are available through normal resolution:

```sh
node tests/browser.mjs
node tests/layout.mjs
```

For an external Playwright installation, set `PLAYWRIGHT_MODULE_PATH` to its package directory.
For a browser outside Playwright's default cache, set `CHROMIUM_PATH` to its executable.
The layout suite uses `FIELDNOTES_URL` if the app is served somewhere other than `http://127.0.0.1:4783`.

## Practical limits

The workspace is personal and browser-local.
It has no device sync or collaborative editing.
A single recovery draft is retained at a time.
Use JSON export before clearing browser data or changing browser profiles.

The hosted app cannot reach the desktop inbox.
Its forwarding options are copy and download.
Local agents must read or poll the inbox themselves.
Acknowledgement records receipt, not task completion.
