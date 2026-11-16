#!/usr/bin/env bash
# Remove the installed native app, leaving notes and agent handoffs intact.
set -euo pipefail
if [[ $# -gt 0 ]]; then
  case "$1" in
    --help|-h) printf '%s\n' 'Usage: bash scripts/uninstall-native.sh' '' 'Remove native launchers and the binary. Keep notes and inbox data.' '' 'Example:' '  bash scripts/uninstall-native.sh'; exit 0 ;;
    *) printf '%s\n' 'Unknown option. Run bash scripts/uninstall-native.sh --help.' >&2; exit 2 ;;
  esac
fi
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
rm -f -- "$HOME/.local/bin/nerv-fieldnotes" "$HOME/.local/bin/fieldnotes-agent" "$data_dir/applications/dev.kesh.Fieldnotes.desktop" "$data_dir/icons/hicolor/scalable/apps/nerv-fieldnotes.svg"
if command -v update-desktop-database >/dev/null; then update-desktop-database "$data_dir/applications"; fi
printf '%s\n' 'Removed the native app and launchers. Notes, backups, and inbox data were preserved.'
