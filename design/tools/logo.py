"""Build PingCraft's logo: design/assets/logo/pingcraft-logo.pcraft (PhotoCraft) and its exports.

    python3 -I design/tools/logo.py --photocraft /path/to/photocraft-cli [--source drawing-on-flat-blue.png]

The logo is the engraved dolphin on a rounded orchid tile, made like CraftCloud's mackerel logo: a rounded-rectangle
shape layer "Tile" (1600 px, radius 224, the brand orchid) and the dolphin as a pixel layer "Dolphin", clipped to it.

- `--source`: the original drawing, black-and-white ink on a flat blue (#4da3c7). The dolphin is cut out by how
  much blue each pixel has (ink is grey, so its coverage is 1 - blueness) and the blue is un-mixed from the edges,
  so no blue fringe is left. It writes `assets/logo/pingcraft-dolphin.png` (RGBA, 2048 px). Without `--source` the
  saved cut-out is used.
- Exports, all from the .pcraft through `photocraft-cli convert`: PNGs at 16 … 1600 px, `pingcraft.ico`
  (16 … 256), `pingcraft.icns` (PNG chunks, written here; no external tool) and `pingcraft-logo.webp`.

Needs numpy only with `--source`, and ImageMagick (`magick`) for the resizes.
"""

import argparse
import os
import struct
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
LOGO = os.path.join(os.path.dirname(HERE), "assets", "logo")
TILE = "#b546c9"  # brand orchid = THEME accent (lib.py); hue 291, between FilmCraft's violet and EffectCraft's pink
SIZE, RADIUS, SCALE = 1600, 224, 76  # tile px, corner radius px, dolphin scale % of the 2048 px cut-out
BLUE = (77, 163, 199)  # the source drawing's flat background
PNG_SIZES = (16, 32, 48, 64, 128, 256, 512, 1024, 1600)
# ICNS chunk type -> pixel size (PNG payloads)
ICNS = (("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic10", 1024),
        ("ic11", 32), ("ic12", 64), ("ic13", 256), ("ic14", 512))


def run(*cmd):
    subprocess.run(["nice", "-n", "10", *cmd], check=True, stdout=subprocess.DEVNULL)


def cut_out(source, out):
    import numpy as np

    with tempfile.TemporaryDirectory() as tmp:
        raw = os.path.join(tmp, "src.rgb")
        size = subprocess.run(["magick", "identify", "-format", "%w %h", source], check=True, capture_output=True,
                              text=True).stdout.split()
        w, h = int(size[0]), int(size[1])
        run("magick", source, "-depth", "8", f"rgb:{raw}")
        px = np.fromfile(raw, dtype=np.uint8).reshape(h, w, 3).astype(np.float32)
        r, b = px[..., 0], px[..., 2]
        a = 1.0 - np.clip((b - r) / (BLUE[2] - BLUE[0]), 0, 1)
        a[a < 0.02] = 0
        grey = np.clip((r - (1 - a) * BLUE[0]) / np.maximum(a, 1e-3), 0, 255)
        rgba = np.zeros((h, w, 4), np.uint8)
        rgba[..., 0] = rgba[..., 1] = rgba[..., 2] = grey.astype(np.uint8)
        rgba[..., 3] = (a * 255).round().astype(np.uint8)
        rgba.tofile(os.path.join(tmp, "out.rgba"))
        run("magick", "-size", f"{w}x{h}", "-depth", "8", f"rgba:{os.path.join(tmp, 'out.rgba')}", out)


def build_pcraft(cli, dolphin, out):
    if os.path.exists(out):
        os.remove(out)
    steps = [
        ("shape.create", {"kind": "roundedRect", "rect": [0, 0, SIZE, SIZE], "radii": RADIUS, "fill": TILE}),
        ("layer.renameLayer", {"name": "Tile"}),
        ("file.placeEmbedded", {"path": dolphin, "scale": SCALE, "center": [SIZE / 2, SIZE / 2],
                                "smartObject": False, "fit": False}),
        ("layer.renameLayer", {"name": "Dolphin"}),
        ("layer.createClippingMask", {}),
        ("layer.delete", {"layer": 2}),  # the new document's empty "Layer 1"; a document must keep one layer
    ]
    args = [cli, "run", "--new", '{"width":%d,"height":%d,"background":"transparent","name":"PingCraft logo"}'
            % (SIZE, SIZE)]
    for cmd, params in steps:
        import json
        args += ["--cmd", cmd, "--params", json.dumps(params)]
    run(*args, "--out", out)


def write_icns(pngs, out):
    chunks = b""
    for kind, px in ICNS:
        data = open(pngs[px], "rb").read()
        chunks += kind.encode() + struct.pack(">I", len(data) + 8) + data
    with open(out, "wb") as f:
        f.write(b"icns" + struct.pack(">I", len(chunks) + 8) + chunks)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--photocraft", required=True)
    ap.add_argument("--source")
    a = ap.parse_args()
    os.makedirs(LOGO, exist_ok=True)
    dolphin = os.path.join(LOGO, "pingcraft-dolphin.png")
    if a.source:
        cut_out(a.source, dolphin)
    if not os.path.exists(dolphin):
        sys.exit("no assets/logo/pingcraft-dolphin.png: pass --source with the original drawing")
    pcraft = os.path.join(LOGO, "pingcraft-logo.pcraft")
    build_pcraft(a.photocraft, dolphin, pcraft)
    full = os.path.join(LOGO, f"pingcraft-logo-{SIZE}.png")
    run(a.photocraft, "convert", pcraft, full)
    pngs = {SIZE: full}
    for px in PNG_SIZES:
        if px != SIZE:
            p = os.path.join(LOGO, f"pingcraft-logo-{px}.png")
            run("magick", full, "-filter", "Lanczos", "-resize", f"{px}x{px}", "-strip", p)
            pngs[px] = p
    run("magick", *[pngs[px] for px in (16, 32, 48, 64, 128, 256)], os.path.join(LOGO, "pingcraft.ico"))
    write_icns(pngs, os.path.join(LOGO, "pingcraft.icns"))
    run("magick", full, "-quality", "90", os.path.join(LOGO, "pingcraft-logo.webp"))
    print("logo:", ", ".join(sorted(os.listdir(LOGO))))


if __name__ == "__main__":
    main()
