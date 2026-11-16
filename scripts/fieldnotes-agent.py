#!/usr/bin/env python3
"""Read and acknowledge the local Fieldnotes agent inbox without executing note content."""
import argparse
import json
import sys
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlsplit
from urllib.request import Request, urlopen


def request(base, path, body=None):
    raw = None if body is None else json.dumps(body).encode("utf-8")
    message = Request(base + path, data=raw, headers={"Content-Type": "application/json"})
    try:
        with urlopen(message, timeout=10) as response:
            return json.load(response)
    except HTTPError as error:
        try:
            detail = json.load(error).get("error", str(error))
        except (ValueError, AttributeError):
            detail = str(error)
        raise ValueError(f"Inbox returned HTTP {error.code}: {detail}") from error
    except (URLError, TimeoutError, ValueError) as error:
        raise ValueError(f"Cannot read local inbox at {base}: {error}. Start it with systemctl --user start nerv-fieldnotes") from error


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog="Examples: fieldnotes-agent list --agent codex\n  fieldnotes-agent ack HANDOFF_ID", formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--url", default="http://127.0.0.1:4783", help="Loopback runtime URL")
    commands = parser.add_subparsers(dest="command", required=True)
    listing = commands.add_parser("list", help="List pending handoffs as JSON", epilog="Example: fieldnotes-agent list --agent codex --all")
    listing.add_argument("--agent", help="Only notes addressed to this agent")
    listing.add_argument("--all", action="store_true", help="Include acknowledged handoffs")
    show = commands.add_parser("show", help="Read one full handoff", epilog="Example: fieldnotes-agent show HANDOFF_ID")
    show.add_argument("id")
    ack = commands.add_parser("ack", help="Mark a handoff acknowledged; safe to repeat", epilog="Example: fieldnotes-agent ack HANDOFF_ID")
    ack.add_argument("id")
    send = commands.add_parser("send", help="Enqueue one JSON handoff from stdin", epilog="Example: cat handoff.json | fieldnotes-agent send --stdin")
    send.add_argument("--stdin", required=True, action="store_true", help="Read one handoff JSON object from standard input")
    args = parser.parse_args()
    parsed = urlsplit(args.url)
    if parsed.scheme != "http" or parsed.hostname not in {"localhost", "127.0.0.1"} or parsed.username or parsed.password or parsed.path not in {"", "/"} or parsed.query or parsed.fragment:
        parser.error("--url must be an http://localhost:PORT or http://127.0.0.1:PORT origin")
    base = args.url.rstrip("/")
    try:
        if args.command == "ack":
            result = request(base, "/api/inbox/" + quote(args.id, safe="") + "/ack", {})
        elif args.command == "send":
            result = request(base, "/api/inbox", json.load(sys.stdin))
        else:
            result = request(base, "/api/inbox")
            if args.command == "list":
                result["items"] = [item for item in result["items"] if (args.all or item["status"] == "pending") and (not args.agent or item["agent"] == args.agent)]
            else:
                result = next((item for item in result["items"] if item["id"] == args.id), None)
                if result is None:
                    raise ValueError("Handoff not found; run fieldnotes-agent list --all")
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except (ValueError, BrokenPipeError) as error:
        print(f"fieldnotes-agent: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
