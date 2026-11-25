# shellcheck shell=bash
# Shared setup for the packaging scripts. Source it: `. "$(dirname "$0")/../env.sh"`.
# Adapted from CraftCloud's packaging/env.sh, itself from craft-launcher's (MIT OR Apache-2.0).
#
# Exports:
#   ROOT                     workspace root
#   VERSION                  [workspace.package] version from Cargo.toml (override: PINGCRAFT_VERSION)
#   DIST                     output directory for release files (default: $ROOT/dist/release)
#   PINGCRAFT_BUILD_SHA      git commit of the build
#   PINGCRAFT_BUILD_DATE     UTC build date, YYYY-MM-DD (the AppStream release date)
#   PINGCRAFT_MAINTAINER     the deb/rpm Maintainer field (override for your own builds)
#   CARGO_TARGET_DIR         cargo's target dir (default: $ROOT/target)
#   CRAFT_FONTS_REQUIRED     1: the build fails without the colour emoji font, which `cargo xtask fonts` fetches
#                            here (PINGCRAFT_SKIP_FONTS=1 skips the fetch, for a fonts folder filled another way)
#
# And the helpers warn, copy_docs, sha256 and check_sqlite.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export ROOT

# The version lives in exactly one place: `[workspace.package] version` in the root Cargo.toml.
workspace_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") ; next }
    in_pkg && $1 == "version" { gsub(/[" ]/, "", $3); print $3; exit }
  ' "$ROOT/Cargo.toml"
  return
}

VERSION="${PINGCRAFT_VERSION:-$(workspace_version)}"
if [[ -z "$VERSION" ]]; then
  echo "error: could not read [workspace.package] version from $ROOT/Cargo.toml" >&2
  exit 1
fi
export VERSION

DIST="${DIST:-$ROOT/dist/release}"
mkdir -p "$DIST"
export DIST

if [[ -z "${PINGCRAFT_BUILD_SHA:-}" ]]; then
  PINGCRAFT_BUILD_SHA="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
fi
export PINGCRAFT_BUILD_SHA
export PINGCRAFT_BUILD_DATE="${PINGCRAFT_BUILD_DATE:-$(date -u +%Y-%m-%d)}"
export PINGCRAFT_MAINTAINER="${PINGCRAFT_MAINTAINER:-jonasbove <31095576+jonasbove@users.noreply.github.com>}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

# Packages carry colour emoji: fetch the pinned font (checked against its hash; crates/ui-egui/font_pin.rs) and
# make the build fail without it, instead of falling back to monochrome emoji.
export CRAFT_FONTS_REQUIRED=1
if [[ -z "${PINGCRAFT_SKIP_FONTS:-}" ]]; then
  (cd "$ROOT" && cargo xtask fonts) || exit 1
fi

# Emit a GitHub Actions warning (plain stderr outside Actions).
warn() {
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then echo "::warning::$*"; else echo "warning: $*" >&2; fi
  return
}

# Copy the licence, notice and readme files into a package directory.
copy_docs() {
  local dest="$1" f
  for f in README.md LICENSE-MIT LICENSE-APACHE NOTICE ATTRIBUTION.md; do
    if [[ -f "$ROOT/$f" ]]; then cp "$ROOT/$f" "$dest/"; fi
  done
  return
}

# Portable SHA-256 of a file (prints just the hash).
sha256() {
  local file="$1"
  if command -v sha256sum >/dev/null; then sha256sum "$file" | cut -d' ' -f1; else shasum -a 256 "$file" | cut -d' ' -f1; fi
  return
}

# No package may ship a SQLite older than 3.54.0 (RUSTSEC-2026-0459, deny.toml). PingCraft links the system's
# SQLite and never enables rusqlite's `bundled` feature, but every binary or library put in a package is checked
# anyway. A compiled-in SQLite always carries its database header magic ("SQLite format 3") and its version as a
# string of its own ("3.53.2"); a dynamically linked one carries neither.
check_sqlite() {
  local file version
  for file in "$@"; do
    [[ -f "$file" ]] || continue
    LC_ALL=C grep -aq 'SQLite format 3' "$file" || continue
    version="$(LC_ALL=C grep -aoE '[[:print:]]{5,}' "$file" | grep -xE '3\.[0-9]{2}\.[0-9]+' | sort -V | tail -n1 || true)"
    if [[ -z "$version" ]]; then
      echo "error: $file has SQLite compiled in, and its version could not be read" >&2
      return 1
    fi
    if [[ "$(printf '%s\n3.54.0\n' "$version" | sort -V | head -n1)" != "3.54.0" ]]; then
      echo "error: $file ships SQLite $version, older than 3.54.0 (RUSTSEC-2026-0459)" >&2
      return 1
    fi
    echo "$file: ships SQLite $version"
  done
  return 0
}

# The app icon in the freedesktop hicolor sizes, from the design's logo PNGs, into <share>/icons/hicolor.
stage_icons() {
  local share="$1" size
  for size in 16 32 48 64 128 256 512; do
    # mkdir, then install: FreeBSD's install(1) has no -D.
    mkdir -p "$share/icons/hicolor/${size}x${size}/apps"
    install -m644 "$ROOT/design/assets/logo/pingcraft-logo-$size.png" "$share/icons/hicolor/${size}x${size}/apps/io.github.jonasbove.pingcraft.png"
  done
  return
}
