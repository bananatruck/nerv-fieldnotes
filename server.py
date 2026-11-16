#!/usr/bin/env python3
"""Loopback static app and durable, pull-based agent inbox. No third-party packages."""
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import sqlite3
import sys
import threading
from urllib.parse import unquote, urlsplit

MAX_BODY = 128 * 1024
IDENTIFIER = re.compile(r"^[A-Za-z0-9_-]{1,128}$")
FIELDS = ("id", "noteId", "title", "body", "agent", "createdAt")


def now():
    return datetime.now(timezone.utc).isoformat()


def default_data_dir():
    return Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "nerv-fieldnotes"


def validate_item(value):
    if not isinstance(value, dict) or set(value) != set(FIELDS):
        raise ValueError("Expected exactly id, noteId, title, body, agent, createdAt")
    if not all(isinstance(value[key], str) for key in FIELDS):
        raise ValueError("All handoff fields must be strings")
    if not all(IDENTIFIER.fullmatch(value[key]) for key in ("id", "noteId")):
        raise ValueError("id and noteId must be 1-128 ASCII letters, numbers, underscores, or hyphens")
    if len(value["title"]) > 500 or len(value["body"]) > 100000 or len(value["agent"]) > 100:
        raise ValueError("title, body, or agent exceeds its size limit")
    if not value["agent"].strip():
        raise ValueError("agent must not be empty")
    try:
        timestamp = datetime.fromisoformat(value["createdAt"].replace("Z", "+00:00"))
        if timestamp.tzinfo is None:
            raise ValueError("Timezone required")
    except ValueError as error:
        raise ValueError("createdAt must be an ISO 8601 timestamp with a timezone") from error
    return {key: value[key] for key in FIELDS}


class Inbox:
    def __init__(self, directory):
        self.directory = Path(directory).resolve()
        self.directory.mkdir(parents=True, exist_ok=True, mode=0o700)
        self.exports = self.directory / "inbox"
        self.exports.mkdir(mode=0o700, exist_ok=True)
        self.database = self.directory / "inbox.sqlite3"
        self.lock = threading.Lock()
        with self.connect() as db:
            db.execute("CREATE TABLE IF NOT EXISTS inbox (id TEXT PRIMARY KEY, payload TEXT NOT NULL, status TEXT NOT NULL, received_at TEXT NOT NULL)")
        self.database.chmod(0o600)

    @contextmanager
    def connect(self):
        database = sqlite3.connect(self.database, timeout=10)
        try:
            with database:
                yield database
        finally:
            database.close()

    @staticmethod
    def row_item(row):
        return {**json.loads(row[1]), "status": row[2], "receivedAt": row[3]}

    def items(self):
        with self.connect() as db:
            return [self.row_item(row) for row in db.execute("SELECT * FROM inbox ORDER BY received_at DESC, id DESC")]

    def add(self, item):
        serialized = json.dumps(item, ensure_ascii=False, sort_keys=True)
        with self.lock, self.connect() as db:
            row = db.execute("SELECT * FROM inbox WHERE id = ?", (item["id"],)).fetchone()
            if row:
                if row[1] != serialized:
                    raise FileExistsError("Handoff id already exists with different content")
                return self.row_item(row), True
            received = now()
            exported = self.exports / (item["id"] + ".md")
            temporary = exported.with_suffix(".tmp")
            # The database is authoritative; Markdown is a convenient agent-readable copy.
            document = f"# {item['title'] or 'Untitled note'}\n\nAgent: {item['agent']}\nHandoff ID: {item['id']}\nNote ID: {item['noteId']}\nCreated: {item['createdAt']}\n\n{item['body']}\n"
            temporary.write_text(document, encoding="utf-8")
            temporary.chmod(0o600)
            temporary.replace(exported)
            db.execute("INSERT INTO inbox VALUES (?, ?, 'pending', ?)", (item["id"], serialized, received))
            return {**item, "status": "pending", "receivedAt": received}, False

    def acknowledge(self, identifier):
        with self.lock, self.connect() as db:
            row = db.execute("SELECT * FROM inbox WHERE id = ?", (identifier,)).fetchone()
            if row is None:
                raise KeyError(identifier)
            db.execute("UPDATE inbox SET status = 'acknowledged' WHERE id = ?", (identifier,))
            item = self.row_item(row)
            item["status"] = "acknowledged"
            return item


class Handler(SimpleHTTPRequestHandler):
    server_version = "Fieldnotes"

    def __init__(self, *args, directory, inbox, **kwargs):
        self.inbox = inbox
        super().__init__(*args, directory=directory, **kwargs)

    def allowed_hosts(self):
        port = self.server.server_port
        return {f"127.0.0.1:{port}", f"localhost:{port}"}

    def valid_host(self):
        return self.headers.get("Host", "") in self.allowed_hosts()

    def json_response(self, status, value):
        data = json.dumps(value, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def end_headers(self):
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "same-origin")
        super().end_headers()

    def do_GET(self):
        if not self.valid_host():
            return self.json_response(403, {"error": "Invalid Host header"})
        path = unquote(urlsplit(self.path).path)
        if path == "/api/health":
            return self.json_response(200, {"ok": True, "service": "nerv-fieldnotes", "inboxPath": str(self.inbox.exports)})
        if path == "/api/inbox":
            return self.json_response(200, {"items": self.inbox.items(), "inboxPath": str(self.inbox.exports)})
        if path.startswith("/api/"):
            return self.json_response(404, {"error": "Unknown API endpoint"})
        requested = (Path(self.directory) / path.lstrip("/")).resolve()
        root = Path(self.directory).resolve()
        if any(part.startswith(".") for part in path.split("/") if part) or not requested.is_relative_to(root):
            return self.json_response(403, {"error": "Path not allowed"})
        if requested.is_dir() and not (requested / "index.html").is_file():
            return self.json_response(404, {"error": "Not found"})
        super().do_GET()

    def do_HEAD(self):
        if not self.valid_host():
            self.send_error(403)
            return
        path = unquote(urlsplit(self.path).path)
        requested = (Path(self.directory) / path.lstrip("/")).resolve()
        if any(part.startswith(".") for part in path.split("/") if part) or not requested.is_relative_to(Path(self.directory).resolve()):
            self.send_error(403)
            return
        super().do_HEAD()

    def do_POST(self):
        if not self.valid_host():
            return self.json_response(403, {"error": "Invalid Host header"})
        origin = self.headers.get("Origin")
        expected_origin = "http://" + self.headers.get("Host", "")
        if origin is not None and origin != expected_origin:
            return self.json_response(403, {"error": "Only same-origin requests are allowed"})
        if self.headers.get_content_type() != "application/json":
            return self.json_response(415, {"error": "Use Content-Type: application/json"})
        try:
            length = int(self.headers.get("Content-Length", "-1"))
        except ValueError:
            length = -1
        if length < 0 or length > MAX_BODY or self.headers.get("Transfer-Encoding"):
            return self.json_response(413, {"error": "A Content-Length of at most 131072 bytes is required"})
        self.connection.settimeout(10)
        try:
            value = json.loads(self.rfile.read(length))
            path = urlsplit(self.path).path
            if path == "/api/inbox":
                item, duplicate = self.inbox.add(validate_item(value))
                return self.json_response(200 if duplicate else 201, {"ok": True, "duplicate": duplicate, "item": item})
            match = re.fullmatch(r"/api/inbox/([A-Za-z0-9_-]{1,128})/ack", path)
            if match:
                if value != {}:
                    raise ValueError("Acknowledgement body must be {}")
                return self.json_response(200, {"ok": True, "item": self.inbox.acknowledge(match[1])})
            return self.json_response(404, {"error": "Unknown API endpoint"})
        except (ValueError, UnicodeDecodeError) as error:
            return self.json_response(400, {"error": str(error)})
        except FileExistsError as error:
            return self.json_response(409, {"error": str(error)})
        except KeyError:
            return self.json_response(404, {"error": "Handoff not found"})
        except (OSError, sqlite3.Error):
            self.log_error("Inbox persistence failed")
            return self.json_response(500, {"error": "Inbox could not be saved; retry the same handoff id"})


def make_server(port=4783, directory=None, data_dir=None):
    inbox = Inbox(data_dir or default_data_dir())
    directory = Path(directory or Path(__file__).parent / "dist").resolve()
    server = ThreadingHTTPServer(("127.0.0.1", port), partial(Handler, directory=str(directory), inbox=inbox))
    server.daemon_threads = True
    return server


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog="Example: python server.py --port 4783")
    parser.add_argument("--port", type=int, default=4783, help="Loopback port, default 4783")
    parser.add_argument("--data-dir", type=Path, default=default_data_dir(), help="Persistent inbox directory")
    parser.add_argument("--directory", type=Path, default=Path(__file__).parent / "dist", help="Static app directory")
    args = parser.parse_args()
    if not 0 <= args.port <= 65535:
        parser.error("--port must be between 0 and 65535")
    try:
        with make_server(args.port, args.directory, args.data_dir) as server:
            print(json.dumps({"url": f"http://127.0.0.1:{server.server_port}", "dataDir": str(args.data_dir)}), flush=True)
            server.serve_forever()
    except KeyboardInterrupt:
        pass
    except OSError as error:
        print(f"fieldnotes: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
