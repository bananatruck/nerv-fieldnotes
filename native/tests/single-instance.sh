#!/usr/bin/env bash
# Exercise real remote application dispatch on a private session bus and data root.
set -euo pipefail
if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
  printf '%s\n' 'Usage: bash native/tests/single-instance.sh [PATH_TO_BINARY]' '' 'Requires a desktop display, dbus-run-session, gdbus, and timeout.' 'Starts an isolated session bus and temporary workspace, then sends' '--board and --new-note requests to the running native application.' '' 'Example:' '  bash native/tests/single-instance.sh target/release/nerv-fieldnotes'
  exit 0
fi
if [[ $# -gt 1 ]]; then printf '%s\n' 'Pass at most one binary path.' >&2; exit 2; fi
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
binary="${1:-$repo_dir/target/release/nerv-fieldnotes}"
[[ -x "$binary" ]] || { printf 'Native binary not found: %s\n' "$binary" >&2; exit 2; }
binary="$(realpath -- "$binary")"
for command in dbus-run-session gdbus timeout; do
  command -v "$command" >/dev/null || { printf 'Required test command missing: %s\n' "$command" >&2; exit 2; }
done
if [[ "${FIELDNOTES_SINGLE_INSTANCE_BUS:-}" != 1 ]]; then
  exec dbus-run-session -- env FIELDNOTES_SINGLE_INSTANCE_BUS=1 bash "$0" "$binary"
fi
test_dir="$(mktemp -d /tmp/fieldnotes-single-instance.XXXXXX)"
app_pid=''
cleanup() {
  local result=$?
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill "$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  if (( result != 0 )); then
    printf '%s\n' 'Native single-instance test failed. Application output:' >&2
    cat "$test_dir/app.log" >&2 || true
  fi
  # Only files below the freshly allocated test directory are removed.
  find "$test_dir" -depth -delete
  exit "$result"
}
trap cleanup EXIT
export XDG_DATA_HOME="$test_dir/data"
export XDG_CONFIG_HOME="$test_dir/config"
export XDG_CACHE_HOME="$test_dir/cache"
export GTK_A11Y=none
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME"
# Do not pass --data-dir: that intentionally permits an independent GUI instance.
"$binary" >"$test_dir/app.log" 2>&1 &
app_pid=$!
ready=0
for ((attempt=0; attempt<100; attempt++)); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    printf '%s\n' 'Primary application exited during startup.' >&2
    exit 1
  fi
  owner="$(gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner dev.kesh.Fieldnotes)"
  if [[ "$owner" == '(true,)' ]]; then ready=1; break; fi
  sleep 0.1
done
(( ready == 1 )) || { printf '%s\n' 'Application did not acquire its session bus name.' >&2; exit 1; }
owner_pid() {
  gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.GetConnectionUnixProcessID dev.kesh.Fieldnotes
}
expected="$(owner_pid)"
[[ "$expected" == "(uint32 $app_pid,)" ]] || { printf 'Unexpected bus owner: %s, primary PID %s\n' "$expected" "$app_pid" >&2; exit 1; }
for request in --board --new-note --board; do
  timeout 10 "$binary" "$request" >>"$test_dir/app.log" 2>&1
  kill -0 "$app_pid"
  actual="$(owner_pid)"
  [[ "$actual" == "$expected" ]] || { printf 'Application owner changed after %s: %s\n' "$request" "$actual" >&2; exit 1; }
  printf 'PASS remote %s returned successfully to primary PID %s\n' "$request" "$app_pid"
done
# Match the exact executable path and this private data root, without touching
# unrelated app instances on the user's normal session bus.
matching=0
for proc in /proc/[0-9]*; do
  [[ -r "$proc/environ" && -L "$proc/exe" ]] || continue
  executable="$(readlink "$proc/exe" 2>/dev/null || true)"
  [[ "$executable" == "$binary" ]] || continue
  if tr '\0' '\n' <"$proc/environ" 2>/dev/null | grep -qx "XDG_DATA_HOME=$XDG_DATA_HOME"; then
    ((matching+=1))
  fi
done
[[ "$matching" == 1 ]] || { printf 'Expected one surviving app process for test workspace, found %s\n' "$matching" >&2; exit 1; }
printf '%s\n' 'PASS exactly one native app process owns the isolated workspace after all remote requests'
