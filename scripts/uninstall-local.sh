#!/usr/bin/env bash
# Preserve notes and handoffs. Remove only this application's installed launchers.
set -euo pipefail
systemctl --user disable --now nerv-fieldnotes.service || true
rm -f -- "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/nerv-fieldnotes.service" "${XDG_DATA_HOME:-$HOME/.local/share}/applications/nerv-fieldnotes.desktop" "$HOME/.local/bin/nerv-fieldnotes" "$HOME/.local/bin/fieldnotes-agent"
systemctl --user daemon-reload
printf '%s\n' 'Uninstalled Fieldnotes launchers and service. Browser notes and agent inbox data were preserved.'
