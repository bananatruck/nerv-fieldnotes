# Use native Fieldnotes

NERV Fieldnotes is a Rust application with GTK4 widgets and local SQLite storage.
The desktop app opens your most recently edited active sticky note.
Use its top-right dots menu to open the complete note board.

## Install on Arch Linux

Install the build tools and native libraries:

```sh
sudo pacman -S --needed rust pkgconf gtk4 sqlite
```

From the repository, build and install the application:

```sh
bash scripts/install-native.sh
```

The installer copies the release binary into `~/.local/bin` and adds **NERV Fieldnotes** to your application menu.
The installed application runs independently of the checkout.
It preserves the existing agent inbox database and disables the old Fieldnotes browser service if that service is installed.

Launch **NERV Fieldnotes** from your application menu or run:

```sh
~/.local/bin/nerv-fieldnotes
```

To open the full board directly, run:

```sh
~/.local/bin/nerv-fieldnotes --board
```

To create a sticky note directly, run:

```sh
~/.local/bin/nerv-fieldnotes --new-note
```

The application menu entry also provides **New note** and **All notes** actions.

## Move browser notes into the app

Export a version 1 JSON backup from the previous browser version before clearing browser data.
Import that file from the native app or run:

```sh
~/.local/bin/nerv-fieldnotes import fieldnotes-backup.json
```

Import validates the whole file before merging.
For an existing note ID, the newer edited version wins.
The native app stores notes in SQLite instead of browser storage.

## Back up saved notes

Export every saved note, including archived and trashed notes:

```sh
~/.local/bin/nerv-fieldnotes export fieldnotes-backup.json
```

An existing export file is preserved unless you add `--force`.
For a pipeline, use:

```sh
~/.local/bin/nerv-fieldnotes export --stdout > backup.json
cat backup.json | ~/.local/bin/nerv-fieldnotes import --stdin
```

The default database is `$XDG_DATA_HOME/nerv-fieldnotes/inbox.sqlite3`.
If `XDG_DATA_HOME` is unset, the default is `~/.local/share/nerv-fieldnotes/inbox.sqlite3`.
Use `--data-dir DIR` to keep a separate workspace.

## Read agent handoffs

Forward a note from the native app to put a snapshot into the durable agent inbox.
An agent reads and acknowledges handoffs with the native CLI:

```sh
~/.local/bin/fieldnotes-agent list --agent codex
~/.local/bin/fieldnotes-agent show HANDOFF_ID
~/.local/bin/fieldnotes-agent ack HANDOFF_ID
~/.local/bin/fieldnotes-agent list --all
```

Output is JSON.
Acknowledging a handoff records receipt and is safe to repeat.
It does not record task completion or execute the note text.

To enqueue a six-field handoff payload from a file, run:

```sh
cat handoff.json | ~/.local/bin/fieldnotes-agent send --stdin
```

The payload contains `id`, `noteId`, `title`, `body`, `agent`, and `createdAt`.
Keep the same ID and content when retrying a delivery.
A reused ID with different content is rejected.

## Build and verify

From the repository, run:

```sh
cargo test --manifest-path native/Cargo.toml --locked -j 1
cargo clippy --manifest-path native/Cargo.toml --locked --all-targets -j 1 -- -D warnings
cargo build --manifest-path native/Cargo.toml --release --locked -j 1
```

The app uses the installed GTK4 and SQLite libraries.

## Uninstall the application

Run:

```sh
bash scripts/uninstall-native.sh
```

Uninstall removes the binary, desktop entry, and agent launcher.
Your notes, backups, drafts, and agent inbox remain on disk.
