# Fieldnotes build plan

Build a local-first sticky notes workspace for Kesh's Arch Linux desktop.
Use original NERV HQ inspired typography, color, and instrumentation.

## Required behavior

- Create, edit, autosave drafts, duplicate, pin, color, tag, and search notes.
- Keep active, archived, and deleted notes in separate views.
- Restore deleted notes and provide undo after moving notes.
- Support checklists without requiring Markdown knowledge.
- Export a complete JSON backup and validate imports before merging.
- Forward a note as a Markdown handoff through copy, download, or a durable local inbox.
- Keep hosted and local browser storage independent and explain that in the interface.
- Install a persistent loopback service and desktop launcher on Arch Linux.
- Support keyboard shortcuts, keyboard dialogs, responsive layout, and reduced motion.
- Cache the static interface for offline use.

## Verification

Run model tests, actual HTTP integration tests, and browser interaction checks.
Inspect screenshots at desktop and mobile widths.
Check the deployed version and the installed local service.
Push 68 substantive commits, recording each successful remote update outside the repository.

## Storage

Browser notes stay in localStorage for this origin and browser profile.
JSON export is the portable backup format.
The local server stores explicitly forwarded handoffs in SQLite under XDG_DATA_HOME.
It never executes note text.
Hosted publication uses private Sites hosting.
