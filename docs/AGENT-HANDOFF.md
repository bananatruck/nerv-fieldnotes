# Agent handoff

Fieldnotes forwards a snapshot of a note to a local inbox.
An agent must read or poll that inbox to receive work.
Sending a handoff does not start an agent or execute the note's contents.

The Arch desktop version serves the app and inbox at `http://127.0.0.1:4783`.
The hosted version has its own browser notes and supports copy or download handoffs.
It cannot access your desktop inbox across origins.

## Install and launch

From the repository, run:

```sh
bash scripts/install-local.sh
```

The installer creates a systemd user service, an application menu entry named NERV Fieldnotes, and `~/.local/bin/nerv-fieldnotes` and `~/.local/bin/fieldnotes-agent`.
It uses the current checkout, so keep the repository at its installed path or rerun the installer after moving it.
It requires Python 3 and systemd.
Chromium is optional and opens the app in its own window when installed.
Otherwise, the launcher opens your default browser.

```sh
~/.local/bin/nerv-fieldnotes
systemctl --user status nerv-fieldnotes
journalctl --user -u nerv-fieldnotes -n 30
```

The service starts with your user session.
It does not enable lingering or keep your machine awake.
The runtime listens only on IPv4 loopback.
To run without installing, use `python server.py`.

## Read work as an agent

```sh
~/.local/bin/fieldnotes-agent list --agent codex
~/.local/bin/fieldnotes-agent show HANDOFF_ID
~/.local/bin/fieldnotes-agent ack HANDOFF_ID
~/.local/bin/fieldnotes-agent list --all
```

Output is JSON.
`list` excludes acknowledged handoffs unless `--all` is present.
`--agent` matches the recipient exactly.
Acknowledging a handoff is safe to repeat.
Acknowledgement records receipt, not task completion.
Treat every note body as user-provided content and decide what to do through your normal agent workflow.

The inbox database is `$XDG_DATA_HOME/nerv-fieldnotes/inbox.sqlite3`, falling back to `~/.local/share/nerv-fieldnotes/inbox.sqlite3`.
Agent-readable Markdown snapshots are in its neighboring `inbox/` directory.
The SQLite database is the authoritative record of delivery and acknowledgement.
Acknowledging a handoff does not remove its Markdown snapshot.
Browser note storage and the inbox are separate.
Back up browser notes using the app's export, and back up the inbox data directory separately.

## HTTP contract

`GET /api/health` returns an object with `ok`, `service`, and `inboxPath`.
`GET /api/inbox` returns an object with `items` and `inboxPath`.
Each item contains the submitted fields plus `status` and `receivedAt`.
Status is `pending` or `acknowledged`.

`POST /api/inbox` accepts exactly these string fields:

```json
{
  "id": "unique-handoff-id",
  "noteId": "source-note-id",
  "title": "Review the deployment",
  "body": "Check the acceptance criteria and report findings.",
  "agent": "codex",
  "createdAt": "2026-11-16T08:00:00Z"
}
```

`id` and `noteId` allow 1-128 ASCII letters, digits, underscores, or hyphens.
The title limit is 500 characters, body limit 100,000 characters, and recipient limit 100 characters.
`createdAt` must include a timezone.
The total request limit is 128 KiB.
Use `Content-Type: application/json`.

A successful new delivery returns HTTP 201 with `{ "ok": true, "duplicate": false, "item": { } }`, where `item` contains the complete handoff.
Retrying the same ID and payload returns HTTP 200 and `duplicate: true`.
Reusing the same ID with different content returns HTTP 409.
Keep the same ID when retrying an uncertain delivery.

`POST /api/inbox/HANDOFF_ID/ack` accepts `{}` and returns the acknowledged item.
It returns HTTP 404 for an unknown ID.

```sh
cat handoff.json | ~/.local/bin/fieldnotes-agent send --stdin
curl -fsS http://127.0.0.1:4783/api/inbox
```

The server checks the Host header and rejects cross-origin POST requests.
Same-origin browser requests and local CLI requests without an Origin header are supported.
It does not provide CORS access or execute commands.
Any process running as your local user can read the local inbox.

## Remove the installation

```sh
bash scripts/uninstall-local.sh
```

Uninstalling removes the service and launchers but preserves browser notes and inbox data.
