# Acceptance criteria

- A new note survives a page reload with title, body, tags, color, pin state, and checklist mode.
- Search matches all terms across title, text, and tags.
- Archive and trash retain note content and allow restoration.
- Permanent deletion requires explicit confirmation.
- A draft survives closing the editor and reloading the page.
- A checklist completion survives a reload.
- A copied or downloaded handoff includes the note ID, text, tags, destination, and instructions.
- Sending a handoff persists it in the real local inbox.
- An agent acknowledgement changes the durable inbox record.
- An exported backup round-trips through the import validator.
- Invalid imports and storage errors preserve existing notes.
- Keyboard shortcuts operate the actual dialogs and search field.
- The board and editor fit a 360-pixel viewport.
- The app loads offline after caching its static interface.
- The desktop service is enabled and answers the health endpoint.
- The private hosted deployment reports success.
- The Git repository contains at least 68 meaningful pushed changes.
