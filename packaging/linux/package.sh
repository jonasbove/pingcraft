#!/usr/bin/env bash
# Build and package the PingCraft desktop app for Linux (<arch> is x86_64, aarch64 or riscv64):
#
#   $DIST/pingcraft-<version>-linux-<arch>.AppImage        any distro with glibc >= the build host's
#   $DIST/pingcraft-<version>-linux-<arch>.AppImage.zsync  delta updates (needs zsyncmake)
#   $DIST/pingcraft-<version>-linux-<arch>.deb             Debian, Ubuntu, Mint, Pop!_OS, ...
#   $DIST/pingcraft-<version>-linux-<arch>.rpm             Fedora, openSUSE, RHEL, ...
#   $DIST/pingcraft-<version>-linux-<arch>.tar.gz          pingcraft-<version>-linux-<arch>/{bin,share}
#                                                           plus install.sh and README.md
#
# Usage: packaging/linux/package.sh [--skip-build] [--formats "appimage deb rpm tar" or appimage,deb,rpm,tar]
#
# Needs: cargo; nfpm for deb/rpm (https://nfpm.goreleaser.com); appimagetool for the AppImage
# (downloaded into $CARGO_TARGET_DIR if missing). Build on an old distro (CI: Ubuntu 22.04,
# glibc 2.35) so the binary runs on newer ones; GLIBC_FLOOR names the build host's glibc for the
# deb/rpm dependencies. Optional: desktop-file-validate, appstreamcli, zsyncmake.
#
# Cross builds (CI: riscv64, deb/rpm/tar.gz): set CROSS_ARCH (riscv64), CROSS_TARGET (the Rust
# triple), CROSS_COMPILE (binutils prefix, for strip) and optionally EMULATOR (e.g. qemu-riscv64)
# to smoke-test the binary.
#
# Adapted from CraftCloud's packaging/linux/package.sh, itself from craft-launcher's (MIT OR Apache-2.0).
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/linux"
APP_ID=io.github.jonasbove.pingcraft
PKG=pingcraft
# Verify a downloaded file's sha256 against its expected hex digest.
check_sha256() {
  local expected="$1"
  local target="$2"
  printf '%s  %s\n' "$expected" "$target" | sha256sum -c -
}

SKIP_BUILD=0
FORMATS="appimage deb rpm tar"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --formats) FORMATS="$2"; shift 2 ;;
    -h | --help) sed -n '2,22p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
FORMATS="$(printf %s "$FORMATS" | tr ',' ' ')"
for fmt in $FORMATS; do
  case "$fmt" in
    appimage | deb | rpm | tar) ;;
    *) echo "unknown format: $fmt (expected appimage, deb, rpm or tar)" >&2; exit 2 ;;
  esac
done

ARCH="${CROSS_ARCH:-$(uname -m)}"
case "$ARCH" in
  riscv64) DEB_ARCH=riscv64 ;;
  x86_64) DEB_ARCH=amd64 ;;
  aarch64 | arm64) ARCH=aarch64; DEB_ARCH=arm64 ;;
  *) echo "unsupported architecture $ARCH" >&2; exit 2 ;;
esac
BASENAME="$PKG-$VERSION-linux-$ARCH"

echo "==> PingCraft $VERSION for Linux $ARCH ($FORMATS)"

if [[ "$SKIP_BUILD" = 0 ]]; then
  (cd "$ROOT" && cargo build --release --locked -p "$PKG" ${CROSS_TARGET:+--target "$CROSS_TARGET"})
fi
BIN="$CARGO_TARGET_DIR/${CROSS_TARGET:+$CROSS_TARGET/}release"
WORK="$CARGO_TARGET_DIR/linux-package"
STAGE="$WORK/root"
rm -rf "$WORK"

# ---- stage an FHS tree (shared by every format) -------------------------------------------------
install -Dm755 "$BIN/$PKG" "$STAGE/usr/bin/$PKG"
"${CROSS_COMPILE:-}strip" "$STAGE/usr/bin/$PKG" 2>/dev/null || true
install -Dm644 "$HERE/$APP_ID.desktop" "$STAGE/usr/share/applications/$APP_ID.desktop"
mkdir -p "$STAGE/usr/share/metainfo"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@DATE@/$PINGCRAFT_BUILD_DATE/g" \
  "$HERE/$APP_ID.metainfo.xml.in" >"$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
stage_icons "$STAGE/usr/share"
check_sqlite "$STAGE/usr/bin/$PKG"
mkdir -p "$STAGE/usr/share/doc/$PKG"
copy_docs "$STAGE/usr/share/doc/$PKG"

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$STAGE/usr/share/applications/$APP_ID.desktop"
fi
if command -v appstreamcli >/dev/null; then
  appstreamcli validate --no-net --explain "$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
fi

has() { local format="$1"; case " $FORMATS " in *" $format "*) return 0 ;; *) return 1 ;; esac; }

# ---- .tar.gz ------------------------------------------------------------------------------------
# bin/ and share/ as a /usr or ~/.local tree (the Arch package and the Flatpak are built from it),
# plus install.sh, which puts it in ~/.local without root, and the desktop README.
if has tar; then
  mkdir -p "$WORK/tar"
  cp -R "$STAGE/usr" "$WORK/tar/$BASENAME"
  install -m755 "$HERE/install.sh" "$WORK/tar/$BASENAME/install.sh"
  install -m644 "$ROOT/packaging/README-desktop.md" "$WORK/tar/$BASENAME/README.md"
  tar -C "$WORK/tar" -czf "$DIST/$BASENAME.tar.gz" "$BASENAME"
  echo "wrote $DIST/$BASENAME.tar.gz"
fi

# ---- .deb / .rpm --------------------------------------------------------------------------------
if has deb || has rpm; then
  command -v nfpm >/dev/null || { echo "error: nfpm not found (https://nfpm.goreleaser.com/install/)" >&2; exit 1; }
  export VERSION
  export NFPM_ARCH="$DEB_ARCH"
  # The build host's glibc: Ubuntu 22.04 (2.35) normally, Ubuntu 24.04 (2.39) for riscv64 in CI.
  export GLIBC_FLOOR="${GLIBC_FLOOR:-2.35}"
  # nfpm expands env vars in fields like `version` and `arch`, but not in `contents[].src`.
  sed "s|\${STAGE}|$STAGE|g" "$HERE/nfpm.yaml" >"$WORK/nfpm.yaml"
  for fmt in deb rpm; do
    if has "$fmt"; then (cd "$ROOT" && nfpm package -f "$WORK/nfpm.yaml" -p "$fmt" -t "$DIST/$BASENAME.$fmt"); fi
  done
fi

# ---- AppImage -----------------------------------------------------------------------------------
if has appimage; then
  APPIMAGETOOL_VERSION=1.9.1
  case "$ARCH" in
    x86_64) APPIMAGETOOL_SHA256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0 ;;
    aarch64) APPIMAGETOOL_SHA256=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158 ;;
    *) APPIMAGETOOL_SHA256="" ;;
  esac
  # appimagetool otherwise fetches the unpinned type2-runtime `continuous` build and embeds it.
  APPIMAGE_RUNTIME_VERSION=20251108
  case "$ARCH" in
    x86_64) APPIMAGE_RUNTIME_SHA256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d ;;
    aarch64) APPIMAGE_RUNTIME_SHA256=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444 ;;
    *) APPIMAGE_RUNTIME_SHA256="" ;;
  esac
  if [[ -z "$APPIMAGE_RUNTIME_SHA256" ]]; then
    echo "no pinned AppImage runtime for $ARCH; build the AppImage on x86_64 or aarch64" >&2
    exit 2
  fi
  APPDIR="$WORK/PingCraft.AppDir"
  cp -R "$STAGE" "$APPDIR"
  # AppRun is a script, not a symlink: it installs the menu entry and icons into $XDG_DATA_HOME
  # before starting, so a Wayland compositor finds the window's icon by its app id.
  install -Dm755 "$HERE/AppRun" "$APPDIR/AppRun"
  cp "$HERE/$APP_ID.desktop" "$APPDIR/$APP_ID.desktop"
  cp "$STAGE/usr/share/icons/hicolor/256x256/apps/$APP_ID.png" "$APPDIR/$APP_ID.png"
  ln -s "$APP_ID.png" "$APPDIR/.DirIcon"

  TOOL="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
  if [[ -z "$TOOL" ]]; then
    if [[ -z "$APPIMAGETOOL_SHA256" ]]; then
      echo "no pinned appimagetool for $ARCH; set APPIMAGETOOL or put appimagetool on PATH" >&2
      exit 2
    fi
    TOOL="$CARGO_TARGET_DIR/appimagetool-$APPIMAGETOOL_VERSION-$ARCH.AppImage"
    if [[ ! -x "$TOOL" ]]; then
      download="$TOOL.download"
      curl --proto '=https' -fsSL -o "$download" \
        "https://github.com/AppImage/appimagetool/releases/download/$APPIMAGETOOL_VERSION/appimagetool-$ARCH.AppImage"
      check_sha256 "$APPIMAGETOOL_SHA256" "$download"
      chmod 755 "$download"
      mv "$download" "$TOOL"
    fi
    check_sha256 "$APPIMAGETOOL_SHA256" "$TOOL"
  fi
  RUNTIME="$CARGO_TARGET_DIR/appimage-runtime-$APPIMAGE_RUNTIME_VERSION-$ARCH"
  if [[ ! -f "$RUNTIME" ]]; then
    download="$RUNTIME.download"
    curl --proto '=https' -fsSL -o "$download" \
      "https://github.com/AppImage/type2-runtime/releases/download/$APPIMAGE_RUNTIME_VERSION/runtime-$ARCH"
    check_sha256 "$APPIMAGE_RUNTIME_SHA256" "$download"
    mv "$download" "$RUNTIME"
  fi
  check_sha256 "$APPIMAGE_RUNTIME_SHA256" "$RUNTIME"
  # Absolute, because appimagetool runs in $DIST below.
  OUT="$(cd "$DIST" && pwd)/$BASENAME.AppImage"
  APPDIR="$(cd "$APPDIR" && pwd)"
  TOOL="$(cd "$(dirname "$TOOL")" && pwd)/$(basename "$TOOL")"
  RUNTIME="$(cd "$(dirname "$RUNTIME")" && pwd)/$(basename "$RUNTIME")"
  # A .zsync left by an earlier run would hide a missing zsyncmake and describe another file.
  rm -f "$OUT.zsync"
  # Update information: AppImageUpdate and the like read it from the file and fetch only the
  # blocks that changed, through the .zsync published next to each AppImage. `latest` is the
  # newest published release that is not a pre-release. A fork's builds point at its own releases.
  REPO="${GITHUB_REPOSITORY:-jonasbove/pingcraft}"
  UPDATE_INFO="gh-releases-zsync|${REPO%%/*}|${REPO#*/}|latest|$PKG-*-linux-$ARCH.AppImage.zsync"
  # Extract-and-run: works without FUSE (containers, CI). With zsyncmake on the host appimagetool
  # also writes the .zsync, into its working directory, hence the cd.
  (cd "$DIST" && ARCH="$ARCH" APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream --runtime-file "$RUNTIME" -u "$UPDATE_INFO" "$APPDIR" "$OUT")
  echo "wrote $OUT"
  if [[ -s "$OUT.zsync" ]]; then
    echo "wrote $OUT.zsync"
  else
    warn "zsyncmake not found, so $OUT.zsync was not written; AppImage delta updates need it"
  fi
fi

# The binary answers --version without a display, so this checks it starts.
if [[ -z "${CROSS_TARGET:-}" ]]; then
  "$STAGE/usr/bin/$PKG" --version
elif [[ -n "${EMULATOR:-}" ]]; then
  "$EMULATOR" "$STAGE/usr/bin/$PKG" --version
fi
echo "==> done"
ls -lh "$DIST"
