#!/bin/sh
# Adds PingCraft to your app menu (no root needed): the binary to ~/.local/bin, the menu entry,
# the AppStream file and icons to ~/.local/share. Run from the unpacked release tarball
# (pingcraft-<ver>-linux-<arch>/): ./install.sh   Undo: ./install.sh --remove
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
bin="${HOME}/.local/bin"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
id=io.github.jonasbove.pingcraft
sizes="16 32 48 64 128 256 512"

refresh() {
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$data/applications" 2>/dev/null || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = --remove ]; then
    rm -f "$bin/pingcraft" "$data/applications/$id.desktop" "$data/metainfo/$id.metainfo.xml"
    for s in $sizes; do rm -f "$data/icons/hicolor/${s}x${s}/apps/$id.png"; done
    refresh
    echo "Removed PingCraft from the app menu. Your messages and sign-in on this computer are untouched."
    exit 0
fi

[ -x "$here/bin/pingcraft" ] || { echo "error: run this from the unpacked pingcraft-<ver>-linux-<arch> folder" >&2; exit 1; }
install -Dm755 "$here/bin/pingcraft" "$bin/pingcraft"
# The menu entry starts the installed copy by its full path, so ~/.local/bin needn't be on PATH.
tmp="$data/applications/.$id.desktop.tmp"
mkdir -p "$data/applications"
sed -e "s|^Exec=pingcraft|Exec=\"$bin/pingcraft\"|" -e "s|^TryExec=.*|TryExec=$bin/pingcraft|" \
    "$here/share/applications/$id.desktop" > "$tmp"
mv -f "$tmp" "$data/applications/$id.desktop"
install -Dm644 "$here/share/metainfo/$id.metainfo.xml" "$data/metainfo/$id.metainfo.xml"
for s in $sizes; do
    icon="$here/share/icons/hicolor/${s}x${s}/apps/$id.png"
    if [ -f "$icon" ]; then install -Dm644 "$icon" "$data/icons/hicolor/${s}x${s}/apps/$id.png"; fi
done
refresh
echo "Installed: PingCraft is in your app menu (binary: $bin/pingcraft)."
