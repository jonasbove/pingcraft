#!/bin/sh
# deb/rpm postinstall and postremove (nfpm.yaml): refresh the desktop's caches so the menu entry and
# icons show up (or go) at once. Every step is optional.
set -e
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database -q /usr/share/applications || true; fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true; fi
exit 0
