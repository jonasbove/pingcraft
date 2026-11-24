"""Build design/pingcraft.figcraft and design/exports/*.png with FigCraft's headless engine.

    python3 -I -B design/tools/build.py --cli /path/to/figcraft-cli --fonts /path/to/fonts

`--fonts` is a directory holding Inter-Variable.ttf, Archivo-Variable.ttf and GeistMono-Variable.ttf
(from storytold/craft-fonts; see design/README.md). Everything in the document is made with FigCraft
commands (see fc.py, lib.py); nothing writes the .figcraft JSON directly.
"""

import argparse
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from lib import DESIGN, THEME, Doc  # noqa: E402
from components import Library  # noqa: E402
import screens  # noqa: E402


def tokens_page(d):
    """Colour swatches (dark and light) next to the type specimens."""
    d.set(d.specimens, x=0, y=0)
    for mi, mode in enumerate(("Dark", "Light")):
        f = d.frame(None, f"Colour tokens ({mode.lower()})", 820 + mi * 560, 0, 520, 40 + 44 * len(THEME), fill="bg")
        if mode == "Light":
            d.light(f)
        for i, k in enumerate(THEME):
            d.rect(f, 24, 24 + i * 44, 32, 32, k, name=k, stroke="line", strokeWeight=1,
                   strokeAlign="inside")
            d.text(f, k, "Mono/13", "ink", x=72, y=30 + i * 44, name=f"{k} label")
            d.text(f, THEME[k][mi], "Mono/12", "muted", x=300, y=31 + i * 44, name=f"{k} value")


def overview(d, page_name, path):
    pages = d.R("page.list")
    pages = pages["pages"] if isinstance(pages, dict) else pages
    pid = [p["id"] for p in pages if p["name"] == page_name][0]
    d.R("page.select", {"id": pid})
    kids = d.R("node.get", {"id": pid, "fields": ["name", "x", "y", "width", "height"]}).get("children", [])
    # A multi-layer export has a transparent background, so the overview gets a backdrop in the page colour.
    x0 = min(k["x"] for k in kids) - 40
    y0 = min(k["y"] for k in kids) - 40
    x1 = max(k["x"] + k["width"] for k in kids) + 40
    y1 = max(k["y"] + k["height"] for k in kids) + 40
    bd = d.R("node.create", {"type": "rectangle", "x": x0, "y": y0, "width": x1 - x0, "height": y1 - y0,
                             "index": 0, "name": "Overview backdrop", "select": False,
                             "props": {"fill": "#0b0b0d", "locked": True}})["id"]
    d.export([bd] + [k["id"] for k in kids], path, 1)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--cli", default="figcraft-cli", help="path to figcraft-cli")
    ap.add_argument("--fonts", required=True, help="directory with the Inter, Archivo and Geist Mono variable fonts")
    ap.add_argument("--only", help="comma-separated frame names to export (default: all)")
    ap.add_argument("--no-2x", action="store_true", help="skip the 2x exports")
    a = ap.parse_args()
    only = set(a.only.split(",")) if a.only else None

    t0 = time.time()
    d = Doc(a.cli, a.fonts)
    d.page("Tokens & type", first=True)
    d.tokens()
    tokens_page(d)

    d.page("Components")
    Library(d).build(screens.W)

    frames = screens.build(d)  # always the whole document; --only limits the exports

    out = os.path.join(DESIGN, "pingcraft.figcraft")
    d.save(out)
    print("saved", os.path.relpath(out), f"{time.time() - t0:.1f}s")
    ex = os.path.join(DESIGN, "exports")
    os.makedirs(os.path.join(ex, "2x"), exist_ok=True)
    n = 0
    for name, fid in frames:
        if only and name not in only:
            continue
        d.export(fid, os.path.join(ex, f"{name}.png"), 1)
        if not a.no_2x:
            d.export(fid, os.path.join(ex, "2x", f"{name}@2x.png"), 2)
        n += 1
    if not only:
        overview(d, "Tokens & type", os.path.join(ex, "_tokens.png"))
        overview(d, "Components", os.path.join(ex, "_components.png"))
    print("exported", n, "frames", f"{time.time() - t0:.1f}s")
    for note in d.notes:
        print("note:", note)
    d.fc.close()


if __name__ == "__main__":
    main()
