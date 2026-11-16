# NERV Fieldnotes

A local-first sticky notes workspace for Arch Linux, with an original interface inspired by NERV headquarters.
It runs with Python 3 and a modern browser, without installing application dependencies.

## Open it

After local installation, launch **NERV Fieldnotes** from the application menu or run `~/.local/bin/nerv-fieldnotes`.
The local address is http://127.0.0.1:4783.

```sh
bash scripts/install-local.sh
```

The installer creates and enables a systemd user service.
It runs at login and restarts after a failure.
It adds a desktop entry and the `fieldnotes-agent` command under `~/.local/bin`.
Keep this checkout in place because the service and launchers reference it.
The service binds to loopback only.

For a temporary session instead:

```sh
python3 server.py
```

## Everyday notes

Use **New note** to capture a title and text.
Add comma-separated tags, choose a paper color, pin the note, or turn on checklist mode.
The editor saves a recovery draft while you type.
Use **Save note** to put the draft on the board.
Close the editor to keep the draft, or choose **Discard draft** to remove it.
One recovery draft is retained at a time.

Cards provide pin, forward, duplicate, archive, and trash actions with descriptive hover labels.
Archived and trashed notes can be restored.
Moving a note provides Undo.
Permanent deletion requires confirmation.
Search matches titles, bodies, and tags together.
Use the sidebar tags to narrow the board.
Choose grid or list layout and sort by edited date, creation date, or title.

| Shortcut | Action |
| --- | --- |
| Ctrl+N | New note |
| Ctrl+K or Ctrl+/ | Search |
| Ctrl+Enter | Save editor |
| Escape | Close dialog or clear search |
| Ctrl+Z outside text fields | Undo last move |
| ? | Shortcut reference |

Browser and window-manager reserved shortcuts can take precedence.
All actions are also available as buttons.

## Forward to agents

Choose the arrow on a card, name an agent, and add optional instructions.
Review the exact handoff before copying, downloading Markdown, or sending it to the local inbox.
The inbox is a pull queue: it does not execute commands or start agents.
An acknowledgement means received, not completed.
The local inbox requires the local app; the hosted app supports copy and download.

```sh
~/.local/bin/fieldnotes-agent list
~/.local/bin/fieldnotes-agent --help
```

See [the handoff guide](docs/AGENT-HANDOFF.md) for the HTTP protocol and CLI examples.

## Data and backups

Notes live in localStorage in the browser profile and origin where you open the app.
The private hosted version and local app have independent notes.
Use **Settings & backups** to export all notes, including archived and trashed notes, as JSON.
Import validates the whole file, merges IDs, and keeps the newer edited note.
An invalid backup never replaces your notes.
Browser data deletion removes browser notes, so keep exported backups somewhere durable.
The service worker caches the interface for offline use after the first successful visit.

Forwarded handoffs live in SQLite at `$XDG_DATA_HOME/nerv-fieldnotes/inbox.sqlite3`, normally `~/.local/share/nerv-fieldnotes/inbox.sqlite3`.
Markdown copies are stored in its `inbox/` directory.
The CLI uses the HTTP API by default.
Back up the data directory while the service is stopped for a consistent manual copy.

## Verify and maintain

```sh
npm test
node --check dist/app.js
systemctl --user status nerv-fieldnotes.service
journalctl --user -u nerv-fieldnotes.service -n 30
```

The browser suite uses Playwright when it is installed in the environment.
Run it with `node tests/browser.mjs`.
Set `PLAYWRIGHT_MODULE_PATH` and `CHROMIUM_PATH` if the dependencies are outside normal resolution paths.

```sh
bash scripts/uninstall-local.sh
```

Uninstall removes the service and launchers while keeping your data and repository.
This is a personal single-user application.
It does not sync devices or provide collaborative editing.
