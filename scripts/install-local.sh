#!/usr/bin/env bash
# Install only into the current user's XDG directories. No root needed.
set -euo pipefail
start_service=1
case "${1:-}" in
  --no-start) start_service=0 ;;
  --help|-h) printf '%s\n' 'Usage: bash scripts/install-local.sh [--no-start]' 'Install the user service and launchers without root.' 'Use --no-start to install files without contacting systemd.'; exit 0 ;;
  '') ;;
  *) echo 'Usage: bash scripts/install-local.sh [--no-start]' >&2; exit 2 ;;
esac
if (( $# > 1 )); then echo 'Only one installer option is supported.' >&2; exit 2; fi
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
command -v python3 >/dev/null || { echo 'Python 3 is required. On Arch: sudo pacman -S python' >&2; exit 1; }
command -v systemctl >/dev/null || { echo 'systemd user services are required.' >&2; exit 1; }
python3 - "$project_dir" <<'PY'
import json
import os
from pathlib import Path
import shlex
import sys
project = Path(sys.argv[1])
config = Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))
data = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))
bins = Path.home() / '.local/bin'
for directory in (config / 'systemd/user', data / 'applications', bins):
    directory.mkdir(parents=True, exist_ok=True)
quote = lambda value: json.dumps(str(value)).replace('%', '%%')
unit = '\n'.join([
    '[Unit]', 'Description=NERV Fieldnotes local notes and agent inbox', 'After=network.target', '',
    '[Service]', 'Type=simple',
    'ExecStart=' + ' '.join(quote(value) for value in (sys.executable, project / 'server.py', '--port', '4783', '--data-dir', data / 'nerv-fieldnotes')),
    'WorkingDirectory=' + str(project).replace('%', '%%'), 'Restart=on-failure', 'RestartSec=3', 'UMask=0077', 'NoNewPrivileges=true', 'PrivateTmp=true', '',
    '[Install]', 'WantedBy=default.target', '',
])
(config / 'systemd/user/nerv-fieldnotes.service').write_text(unit)
launcher = bins / 'nerv-fieldnotes'
launcher.write_text('''#!/usr/bin/env bash
set -euo pipefail
systemctl --user start nerv-fieldnotes.service
for browser in chromium google-chrome-stable google-chrome brave; do
  if command -v "$browser" >/dev/null 2>&1; then
    exec "$browser" --app=http://127.0.0.1:4783
  fi
done
exec xdg-open http://127.0.0.1:4783
''')
launcher.chmod(0o755)
agent = bins / 'fieldnotes-agent'
agent.write_text('#!/usr/bin/env bash\nexec ' + shlex.quote(sys.executable) + ' ' + shlex.quote(str(project / 'scripts/fieldnotes-agent.py')) + ' "$@"\n')
agent.chmod(0o755)
# Desktop Exec uses its own quoting rules, not shell quoting.
exec_value = '"' + str(launcher).replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%') + '"'
icon = project / 'dist/icon.svg'
desktop = '\n'.join(['[Desktop Entry]', 'Type=Application', 'Name=NERV Fieldnotes', 'Comment=Sticky notes and local agent handoffs', 'Exec=' + exec_value, 'Icon=' + str(icon), 'Terminal=false', 'Categories=Office;Utility;', 'StartupNotify=true', ''])
(data / 'applications/nerv-fieldnotes.desktop').write_text(desktop)
print('Installed user service, NERV Fieldnotes launcher, and fieldnotes-agent CLI.')
PY
if (( start_service == 0 )); then
  printf '%s\n' 'Files installed. Service was not started or enabled.'
  exit 0
fi
systemctl --user daemon-reload
systemctl --user enable --now nerv-fieldnotes.service
python3 - <<'PY'
import json
import time
from urllib.request import urlopen
for attempt in range(30):
    try:
        with urlopen('http://127.0.0.1:4783/api/health', timeout=1) as response:
            health = json.load(response)
        assert health.get('service') == 'nerv-fieldnotes'
        print('Ready: http://127.0.0.1:4783')
        print('Agent inbox: ' + health['inboxPath'])
        break
    except (OSError, AssertionError, ValueError):
        if attempt == 29:
            raise SystemExit('Service did not become ready. Run: journalctl --user -u nerv-fieldnotes -n 30')
        time.sleep(0.1)
PY
