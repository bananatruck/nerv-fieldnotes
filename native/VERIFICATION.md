# Native verification

The Rust application is tested against its SQLite data and actual GTK widgets.
The test workspace uses a temporary data directory, keeping personal notes untouched.

## Verified release results

- 26 Rust unit tests passed.
- 4 executable CLI integration scenarios passed against the release binary.
- 23 native GTK checks passed against the optimized release binary, with critical GTK messages treated as fatal.
- Single-instance dispatch passed for repeated board and new-note requests on an isolated session bus.
- Clippy passed for all targets with warnings denied.
- The installed executable checksum matches the verified release binary.
- The desktop entry passed validation and is visible through the desktop application registry.
- The legacy browser service is disabled and inactive.

## Checks

- Unit tests cover note validation, search, task toggles, import recency, SQLite persistence, recovery drafts, and compatible inbox delivery.
- CLI process tests execute the actual binary without DISPLAY or WAYLAND_DISPLAY and read back its JSON, files, and database state.
- The native smoke suite opens real GTK windows, edits actual widgets, waits for debounced autosave, and reads the saved SQLite records.
- The smoke suite checks compact launch, repeated activation, the All notes menu, search, archive/undo, agent delivery and acknowledgement, invalid imports, and reopening the most recent note.
- Regression checks cover board changes while a sticky is open, clearing old board filters, and releasing closed-note event owners.
- GTK critical messages are fatal during the final smoke run.
- The app's own GTK renderer exports actual sticky and board screenshots for visual inspection.

## Run native UI checks

Build the binary first, then use a new temporary workspace:

```sh
task_test_dir=$(mktemp -d)
G_DEBUG=fatal-criticals GSK_RENDERER=cairo \
  FIELDNOTES_SCREENSHOT_DIR=artifacts \
  native/target/debug/nerv-fieldnotes --data-dir "$task_test_dir" --smoke-test
```

The smoke command requires `--data-dir` so it cannot accidentally modify the default workspace.
It opens short-lived native windows and exits with a failure code if a check fails.
For a release build installed by the installer, use `target/release/nerv-fieldnotes` instead.

## Desktop integration

The installed desktop file is `dev.kesh.Fieldnotes.desktop`, matching the GTK application ID for Wayland grouping.
It includes New note and All notes actions.
The board is registered with the desktop only when requested from the sticky menu.
The default launch opens the recent sticky, and repeated normal launches route to the same application instance.
Explicit `--data-dir` sessions are independent instances for separate workspaces and testing.

The native binary links GTK4 and SQLite directly.
It does not depend on a browser engine or a Python service.
