# Architecture

## Browser workspace

The buildless interface lives in `dist/` and uses browser ES modules.
`lib/model.js` holds pure note, backup, filtering, task, and handoff operations.
`lib/storage.js` owns browser persistence and the recovery draft.
`lib/dom.js` builds DOM nodes with text content rather than inserting user HTML.
`app.js` connects the board, native dialogs, shortcuts, and local inbox.
`sw.js` caches only known static assets and excludes the API.

Saved notes use a versioned JSON envelope.
Every import validates all notes and duplicate IDs before merging.
Storage write failures leave the current in-memory board intact and show an error.
Unreadable persisted data is preserved and the interface blocks writes until it can be recovered.

## Local runtime

`server.py` serves static files and a loopback-only HTTP inbox.
The browser never sends a note until the user chooses Send to inbox.
The server validates host, origin, content type, payload size, and each handoff field.
SQLite is authoritative and handoff IDs make retries idempotent.
A conflicting reuse of an ID is rejected.
Markdown copies make a handoff easy to read with normal agent filesystem tools.
No shell command or agent process is launched from note text.

## Deployment

The systemd user service runs from this checkout and stores handoffs outside Git.
The private Sites deployment serves the same static files.
Browser-local storage is independent per origin and profile.
The hosted app intentionally has no access to the local filesystem or inbox.
