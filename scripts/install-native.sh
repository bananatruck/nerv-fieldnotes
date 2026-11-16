#!/usr/bin/env bash
# Build and install the native app for the current user. No root needed.
set -euo pipefail
skip_build=0
case "${1:-}" in
  --skip-build) skip_build=1 ;;
  --help|-h)
    printf '%s\n' 'Usage: bash scripts/install-native.sh [--skip-build]' '' 'Build and install the native Rust desktop application for this user.' 'Use --skip-build to install an existing target/release/nerv-fieldnotes.' '' 'Examples:' '  bash scripts/install-native.sh' '  bash scripts/install-native.sh --skip-build'
    exit 0 ;;
  '') ;;
  *) printf '%s\n' 'Unknown option. Run bash scripts/install-native.sh --help.' >&2; exit 2 ;;
esac
if (( $# > 1 )); then printf '%s\n' 'Only one installer option is supported.' >&2; exit 2; fi
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
config_dir="${XDG_CONFIG_HOME:-$HOME/.config}"
data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
bin_dir="$HOME/.local/bin"
if (( skip_build == 0 )); then
  command -v cargo >/dev/null || { printf '%s\n' 'Cargo is required to build. On Arch: sudo pacman -S rust' >&2; exit 1; }
  command -v pkg-config >/dev/null || { printf '%s\n' 'pkg-config is required. On Arch: sudo pacman -S pkgconf' >&2; exit 1; }
  pkg-config --atleast-version=4.12 gtk4 || { printf '%s\n' 'GTK4 development files are required. On Arch: sudo pacman -S gtk4' >&2; exit 1; }
  cargo build --manifest-path "$repo_dir/native/Cargo.toml" --target-dir "$repo_dir/target" --release --locked -j 1
fi
binary="$repo_dir/target/release/nerv-fieldnotes"
[[ -x "$binary" ]] || { printf 'Missing release binary: %s\nRun bash scripts/install-native.sh to build it.\n' "$binary" >&2; exit 1; }
"$binary" --version
if command -v ldd >/dev/null; then
  dependencies="$(ldd "$binary")"
  if [[ "$dependencies" == *'not found'* ]]; then printf '%s\n' "$dependencies" 'A native library is missing. On Arch install gtk4 and sqlite.' >&2; exit 1; fi
  if [[ "${dependencies,,}" =~ webkit|chromium|electron ]]; then printf '%s\n' 'Unexpected browser runtime dependency in native binary.' >&2; exit 1; fi
fi
mkdir -p -- "$bin_dir" "$data_dir/applications" "$data_dir/icons/hicolor/scalable/apps"
# Prepare all generated launchers before changing the installed application.
staging_dir="$(mktemp -d "$bin_dir/.fieldnotes-install.XXXXXX")"
trap 'rm -rf -- "$staging_dir"' EXIT
install -m 755 -- "$binary" "$staging_dir/nerv-fieldnotes"
quote_shell() { printf '%q' "$1"; }
printf '#!/usr/bin/env bash\nexec %s inbox "$@"\n' "$(quote_shell "$bin_dir/nerv-fieldnotes")" > "$staging_dir/fieldnotes-agent"
chmod 755 "$staging_dir/fieldnotes-agent"
# Desktop Exec values have their own quoting rules.
escape_exec() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//\$/\\\$}"
  value="${value//\`/\\\`}"
  value="${value//%/%%}"
  printf '"%s"' "$value"
}
cat > "$staging_dir/dev.kesh.Fieldnotes.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=NERV Fieldnotes
Comment=Native sticky notes and local agent handoffs
Exec=$(escape_exec "$bin_dir/nerv-fieldnotes")
Icon=nerv-fieldnotes
Terminal=false
Categories=Office;
StartupNotify=true
StartupWMClass=dev.kesh.Fieldnotes
Actions=NewNote;AllNotes;
Keywords=Sticky;Notes;NERV;EVA;Evangelion;

[Desktop Action NewNote]
Name=New note
Exec=$(escape_exec "$bin_dir/nerv-fieldnotes") --new-note

[Desktop Action AllNotes]
Name=All notes
Exec=$(escape_exec "$bin_dir/nerv-fieldnotes") --board
DESKTOP
if command -v desktop-file-validate >/dev/null; then desktop-file-validate "$staging_dir/dev.kesh.Fieldnotes.desktop"; fi
bash -n "$staging_dir/fieldnotes-agent"
install -m 644 -- "$repo_dir/native/assets/icon.svg" "$data_dir/icons/hicolor/scalable/apps/nerv-fieldnotes.svg"
mv -f -- "$staging_dir/nerv-fieldnotes" "$bin_dir/nerv-fieldnotes"
mv -f -- "$staging_dir/fieldnotes-agent" "$bin_dir/fieldnotes-agent"
install -m 644 -- "$staging_dir/dev.kesh.Fieldnotes.desktop" "$data_dir/applications/dev.kesh.Fieldnotes.desktop"
legacy_desktop="$data_dir/applications/nerv-fieldnotes.desktop"
if [[ -f "$legacy_desktop" ]] && grep -qx 'Name=NERV Fieldnotes' "$legacy_desktop"; then
  rm -f -- "$legacy_desktop"
fi
# The native app and Rust inbox CLI access SQLite directly.
# Retire only the old app-owned browser server, preserving all data.
if command -v systemctl >/dev/null && [[ -f "$config_dir/systemd/user/nerv-fieldnotes.service" ]]; then
  systemctl --user disable --now nerv-fieldnotes.service || printf '%s\n' 'Could not stop the legacy service. Run: systemctl --user disable --now nerv-fieldnotes.service' >&2
fi
if command -v update-desktop-database >/dev/null; then update-desktop-database "$data_dir/applications"; fi
if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q -t -f "$data_dir/icons/hicolor" || true; fi
printf '%s\n' 'Installed native NERV Fieldnotes.' "Launch: $bin_dir/nerv-fieldnotes" 'Application menu: NERV Fieldnotes' "Agent CLI: $bin_dir/fieldnotes-agent list" 'Existing notes and agent inbox data were preserved.'
