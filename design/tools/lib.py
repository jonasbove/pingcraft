"""Shared helpers for building the PingCraft design in FigCraft: tokens, text styles, layer helpers.

Adapted from CraftCloud's `design/tools/lib.py` (same owner, MIT OR Apache-2.0); the CraftCloud app data,
logo and OxiCloud facts are gone and the accent is PingCraft's own. Everything goes through FigCraft's
own engine commands (`figcraft-cli mcp`, see fc.py); nothing writes the .figcraft JSON directly.
"""

import os

from fc import FigCraft, FigCraftError

HERE = os.path.dirname(os.path.abspath(__file__))
DESIGN = os.path.dirname(HERE)
FONT_FILES = ("Inter-Variable.ttf", "Archivo-Variable.ttf", "GeistMono-Variable.ttf")

# --- tokens -------------------------------------------------------------------------------------
# The Craft launcher theme (craft-launcher crates/ui-egui/src/theme.rs, as CraftCloud uses it) with
# PingCraft's accent, a "ping" yellow. (dark, light). Alpha: ink at 60 % = 0x99, 15 % = 0x26, 40 % = 0x66
# (dark); light 62 % = 0x9e, 16 % = 0x29, 40 % = 0x66.
#
# The yellow is bright, so it is used two ways only (see README, "Accent"):
#   * as a fill under on_accent text (unread counts, the wordmark mark), where it is 12.9:1 in both modes;
#   * as text and thin marks through accent_ink (#ffd23f dark, #7a5c00 light), never `accent` itself
#     on the light background, where it is 1.3:1.
THEME = {
    "bg": ("#121316", "#f2f1ee"),
    "sunken": ("#060607", "#e9e8e4"),
    "ink": ("#f2f1ee", "#101014"),
    "ink_strong": ("#ffffff", "#000000"),
    "muted": ("#f2f1ee99", "#1010149e"),
    "faint": ("#f2f1ee61", "#1010146b"),
    "line": ("#f2f1ee26", "#10101429"),
    "line_strong": ("#f2f1ee66", "#10101466"),
    "accent": ("#ffd23f", "#ffd23f"),  # PingCraft ping yellow
    "accent_ink": ("#ffd23f", "#7a5c00"),  # 12.9:1 on dark bg; 5.5:1 on light bg
    "on_accent": ("#121316", "#121316"),  # 12.9:1 on accent
    "accent_tint": ("#ffd23f17", "#ffd23f24"),  # sheet hero wash, own-message wash: 9 % / 14 %
    "invert_bg": ("#ffffff", "#101014"),
    "invert_fg": ("#0b0b0c", "#f2f1ee"),
    "good": ("#3fd6b8", "#0a7563"),
    "warn": ("#ffc14d", "#8a5800"),
    "bad": ("#ff6259", "#b3261e"),
    "on_bad": ("#0b0b0c", "#ffffff"),
    "shadow": ("#0000008c", "#00000024"),
    "scrim": ("#000000aa", "#00000069"),
    "focus": ("#ffd23f", "#7a5c00"),  # focus ring = accent_ink
    "page": ("#0b0b0d", "#dcdbd6"),  # canvas behind the frames (not used inside the UI)
}
# Literal accent_ink per mode, for text ranges (FigCraft can't bind a variable to part of a text).
ACCENT_INK = {False: "#ffd23f", True: "#7a5c00"}

# --- text styles --------------------------------------------------------------------------------

DISPLAY = {"fontFamily": "Archivo", "fontVariations": {"wght": 620, "wdth": 118}}
INTER = lambda size, wght=400, lh=None: dict(  # noqa: E731
    {"fontFamily": "Inter", "fontSize": size, "fontVariations": {"wght": wght, "opsz": 14}},
    **({"lineHeight": lh} if lh else {}))
TEXT_STYLES = {
    "Display/Hero 40": dict(DISPLAY, fontSize=40, letterSpacing="-2%", lineHeight=44),
    "Display/Sheet title 26": dict(DISPLAY, fontSize=26, letterSpacing="-2%"),
    "Display/Heading 22": dict(DISPLAY, fontSize=22, letterSpacing="-2%"),
    "Display/Wordmark 18": dict(DISPLAY, fontSize=18, letterSpacing="-3%"),
    "Body/16 Medium": INTER(16, 500, 22),
    "Body/14": INTER(14, 400, 20),
    "Body/14 Medium": INTER(14, 500, 20),
    "Body/13": INTER(13, 400, 18),
    "Body/13 Medium": INTER(13, 500, 18),
    "Small/12.5": INTER(12.5, 400, 18),
    "Small/12": INTER(12, 400, 16),
    "HUD/11": {"fontFamily": "Geist Mono", "fontSize": 11, "fontVariations": {"wght": 500}, "letterSpacing": "15%",
               "textCase": "upper"},
    "Button/11": {"fontFamily": "Geist Mono", "fontSize": 11, "fontVariations": {"wght": 700}, "letterSpacing": "12%",
                  "textCase": "upper"},
    "Badge/10": {"fontFamily": "Geist Mono", "fontSize": 10, "fontVariations": {"wght": 500}, "letterSpacing": "10%",
                 "textCase": "upper"},
    "Mono/12": {"fontFamily": "Geist Mono", "fontSize": 12, "fontVariations": {"wght": 400}},
    "Mono/13": {"fontFamily": "Geist Mono", "fontSize": 13, "fontVariations": {"wght": 400}},
    "Mono/22 Key": {"fontFamily": "Geist Mono", "fontSize": 22, "fontVariations": {"wght": 500}, "letterSpacing": "6%",
                    "lineHeight": 34},
    "Symbol/34": {"fontFamily": "Inter", "fontSize": 34, "fontVariations": {"wght": 400, "opsz": 14}},
}


def paint(c):
    """'ink' -> Theme variable, '#hex' -> literal, None -> none."""
    if c is None:
        return "none"
    if c.startswith("#"):
        return c
    return {"$var": "Theme/" + c}


class Doc:
    def __init__(self, cli, fonts_dir):
        missing = [f for f in FONT_FILES if not os.path.isfile(os.path.join(fonts_dir, f))]
        if missing:
            raise SystemExit(f"--fonts {fonts_dir}: missing {', '.join(missing)} (see design/README.md, Fonts)")
        self.fc = FigCraft(cli, {"FIGCRAFT_NO_SYSTEM_FONTS": "1"})
        self.R = self.fc.run
        for f in FONT_FILES:
            self.R("text.loadFont", {"path": os.path.join(fonts_dir, f)})
        self.C = {}  # component name -> id
        self.notes = []  # FigCraft quirks met during the build, printed at the end

    # --- setup ---------------------------------------------------------------------------------
    def tokens(self):
        R = self.R
        R("variable.createCollection", {"name": "Theme", "modes": ["Dark", "Light"]})
        for k, (dk, lt) in THEME.items():
            R("variable.create", {"collection": "Theme", "name": k, "type": "color", "values": {"Dark": dk, "Light": lt}})
        # Text styles are made from specimen layers (`style.create {from}` keeps fontVariations and textCase).
        steps = [max(36, round(v["fontSize"] * 1.25) + 14) for v in TEXT_STYLES.values()]
        spec = self.frame(None, "Type specimens", 0, 0, 760, sum(steps) + 40, fill="bg")
        y = 0
        for (name, v), step in zip(TEXT_STYLES.items(), steps):
            sample = "☂ ⚓ ☎ ✈ ♥ ✂ ✏" if name.startswith("Symbol") else f"{name}  ·  one inbox"
            tid = self.node("text", spec, name, 24, 24 + y, text=sample, fill="ink", **v)
            R("style.create", {"type": "text", "name": name, "from": tid})
            R("style.apply", {"ids": [tid], "style": name})
            y += step
        self.specimens = spec
        R("style.create", {"type": "effect", "name": "Shadow/Dialog", "value": [
            {"type": "dropShadow", "color": "#0000008c", "offset": [0, 8], "radius": 24}]})
        try:
            R("style.bind", {"style": "Shadow/Dialog", "field": "effects/0/color", "variable": "shadow"})
        except FigCraftError as e:
            self.notes.append(f"style.bind effect colour failed: {e}")

    def light(self, ids):
        self.R("variable.setMode", {"ids": ids if isinstance(ids, list) else [ids], "collection": "Theme",
                                    "mode": "Light"})

    def page(self, name, first=False, bg="#0b0b0d"):
        if first:
            pages = self.R("page.list")
            pages = pages.get("pages", pages) if isinstance(pages, dict) else pages
            pid = pages[0]["id"]
            self.R("page.rename", {"id": pid, "name": name})
        else:
            pid = self.R("page.add", {"name": name})["id"]
        self.R("page.select", {"id": pid})
        self.R("page.setBackground", {"id": pid, "color": bg})
        return pid

    # --- layers --------------------------------------------------------------------------------
    def node(self, typ, parent=None, name=None, x=0, y=0, w=None, h=None, **props):
        p = {"type": typ, "x": x, "y": y, "select": False}
        if parent is not None:
            p["parent"] = parent
        if name:
            p["name"] = name
        if w is not None:
            p["width"] = w
        if h is not None:
            p["height"] = h
        for k in ("text", "path", "closed", "points"):
            if k in props:
                p[k] = props.pop(k)
        for k in ("fill", "stroke"):
            if k in props:
                props[k] = paint(props[k])
        if props:
            p["props"] = props
        return self.R("node.create", p)["id"]

    def frame(self, parent, name, x, y, w, h, fill=None, stroke=None, clip=True, **props):
        props.setdefault("fill", fill)
        if stroke:
            props["stroke"] = stroke
            props.setdefault("strokeWeight", 1)
            props.setdefault("strokeAlign", "inside")
        props.setdefault("clip", clip)
        return self.node("frame", parent, name, x, y, w, h, **props)

    def comp(self, name, x, y, w, h, fill=None, stroke=None, **props):
        props.setdefault("fill", fill)
        if stroke:
            props["stroke"] = stroke
            props.setdefault("strokeWeight", 1)
            props.setdefault("strokeAlign", "inside")
        cid = self.node("component", None, name, x, y, w, h, **props)
        self.C[name] = cid
        return cid

    def rect(self, parent, x, y, w, h, fill, name=None, **props):
        return self.node("rectangle", parent, name or "rect", x, y, w, h, fill=fill, **props)

    def text(self, parent, chars, style, color="ink", x=0, y=0, name=None, w=None, align=None, lines=None, **props):
        props["textStyle"] = style
        props["fill"] = color
        if w is not None:
            props.setdefault("textAutoResize", "height")
        if align:
            props["textAlign"] = align
        tid = self.node("text", parent, name or chars[:40], x, y, w, None, text=chars, **props)
        if lines:
            self.R("text.maxLines", {"ids": [tid], "lines": lines})
        return tid

    def set(self, ids, **props):
        if not isinstance(ids, list):
            ids = [ids]
        for k in ("fill", "stroke"):
            if k in props:
                props[k] = paint(props[k])
        return self.R("node.set", {"ids": ids, "props": props})

    def get(self, nid, *fields):
        return self.R("node.get", {"id": nid, "fields": list(fields)})

    def absolute(self, nid, x, y, w=None, h=None):
        """Take a layer out of its auto layout flow and pin it at x, y (the position has to follow in a
        second call: FigCraft keeps the flow position when `absolute` is set)."""
        self.set(nid, absolute=True)
        p = {"x": x, "y": y}
        if w is not None:
            p["width"] = w
        if h is not None:
            p["height"] = h
        self.set(nid, **p)

    def effect_style(self, ids, style):
        self.R("style.apply", {"ids": ids if isinstance(ids, list) else [ids], "style": style})

    def bind_stroke(self, ids, var):
        self.R("variable.bind", {"ids": ids if isinstance(ids, list) else [ids], "field": "strokes/0/color",
                                 "variable": var})

    # --- components ----------------------------------------------------------------------------
    def inst(self, comp, parent=None, x=0, y=0, props=None, w=None, h=None, name=None):
        cid = self.C[comp] if isinstance(comp, str) else comp
        p = {"component": cid, "x": x, "y": y}
        if parent is not None:
            p["parent"] = parent
        iid = self.R("instance.create", p)["id"]
        if props:
            self.R("instance.setProperties", {"ids": [iid], "properties": props})
        sz = {}
        if w is not None:
            sz["width"] = w
        if h is not None:
            sz["height"] = h
        if name:
            sz["name"] = name
        if sz:
            self.set(iid, **sz)
        return iid

    def find(self, root, name):
        """Exact-name descendants of `root` (`node.find {name}` matches substrings)."""
        r = self.R("node.find", {"root": root, "name": name, "limit": 200})
        return [i for i in r["ids"] if self.get(i, "name")["name"] == name]

    def sub(self, root, name):
        ids = self.find(root, name)
        if not ids:
            raise FigCraftError(f"no layer {name!r} under {root}")
        return ids[0]

    def prop(self, comp_id, layer_id, name, typ="text"):
        self.R("component.addProperty", {"id": comp_id, "name": name, "type": typ, "layer": layer_id})

    def variants(self, ids, name, x, y):
        s = self.R("component.combineVariants", {"ids": ids})["id"]
        self.set(s, name=name, x=x, y=y)
        self.C[name] = s
        return s

    def export(self, ids, path, scale=1):
        return self.fc.tool("export", {"ids": ids if isinstance(ids, list) else [ids], "format": "png",
                                       "scale": scale, "path": path})

    def save(self, path):
        return self.fc.tool("save_file", {"path": path})


# --- layout helpers (CraftCloud design/tools/screens.py and v2.py) --------------------------------

def side_stroke(d, ids, color="line", **sides):
    """Hairline on some sides only (top/right/bottom/left = 1)."""
    if not isinstance(ids, list):
        ids = [ids]
    w = {f"stroke{k.capitalize()}Weight": sides.get(k, 0) for k in ("top", "right", "bottom", "left")}
    d.set(ids, strokes=[{"type": "solid", "color": "#ffffff"}], strokeAlign="inside", **w)
    d.bind_stroke(ids, color)


def box_stroke(d, nid, color="line", weight=1):
    d.set(nid, strokes=[{"type": "solid", "color": "#ffffff"}], strokeWeight=weight, strokeAlign="inside")
    d.bind_stroke(nid, color)


def hud(d, parent, text, color="muted", name=None, x=0, y=0):
    return d.text(parent, text, "HUD/11", color, x=x, y=y, name=name or text[:40])


def vstack(d, parent, name, x=0, y=0, w=10, gap=8, pad=None, fill=None, **kw):
    kw.setdefault("sizingV", "hug")
    return d.frame(parent, name, x, y, w, 10, fill=fill, layout="vertical", gap=gap, padding=pad or [0, 0, 0, 0],
                   clip=False, **kw)


def hstack(d, parent, name, x=0, y=0, gap=8, h=None, **kw):
    kw.setdefault("crossAlign", "center")
    kw.setdefault("sizingH", "hug")
    kw.setdefault("sizingV", "hug" if h is None else "fixed")
    return d.frame(parent, name, x, y, 10, h or 10, fill=kw.pop("fill", None), layout="horizontal", gap=gap,
                   clip=False, **kw)


def spacer(d, parent, v=False):
    return d.frame(parent, "Spacer", 0, 0, 1, 1, fill=None, **({"sizingV": "fill"} if v else {"sizingH": "fill"}))
