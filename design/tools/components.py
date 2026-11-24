"""The PingCraft component library, built in FigCraft (page "Components").

The Craft launcher parts (buttons, cells, badges, inputs, header, footer) are adapted from CraftCloud's
`design/tools/components.py` and `v2.py` (same owner, MIT OR Apache-2.0). The chat parts (chat row,
messages, day divider, network tag, unread count, symbol tile) are PingCraft's own.
"""

import math
import os

LOGO_256 = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "logo", "pingcraft-logo-256.png")

from lib import box_stroke, hstack, hud, side_stroke, spacer, vstack


def circle_path(cx, cy, r, n=28):
    pts = [(cx + r * math.cos(2 * math.pi * i / n), cy + r * math.sin(2 * math.pi * i / n)) for i in range(n)]
    return "M" + " L".join(f"{x:.2f} {y:.2f}" for x, y in pts) + " Z"


# 14 px icons, 1.5 px strokes in ink (buttons recolour them). arrow-right, check, x, plus, external, key, save,
# refresh and logout are CraftCloud's (v2.py ICONS); the rest are drawn for PingCraft.
ICONS = {
    "arrow-right": "M1.5 7 L12 7 M8 3 L12 7 L8 11",
    "check": "M2 7.5 L5.5 11 L12 3",
    "x": "M3 3 L11 11 M11 3 L3 11",
    "plus": "M7 2 L7 12 M2 7 L12 7",
    "external": "M5 2 L12 2 L12 9 M12 2 L3 11",
    "key": circle_path(4.5, 9.5, 2.8, 16) + " M6.5 7.5 L12 2 M10 4 L12 6",
    "save": "M2 2 L10 2 L12 4 L12 12 L2 12 Z M4.5 2 L4.5 5 L9 5 L9 2 M4.5 12 L4.5 8.5 L9.5 8.5 L9.5 12",
    "copy": "M5 5 L12.5 5 L12.5 12.5 L5 12.5 Z M2.5 9.5 L1.5 9.5 L1.5 1.5 L9.5 1.5 L9.5 2.5",
    "refresh": "M11.5 4.5 C10.5 2.8 8.9 2 7 2 C4.2 2 2 4.2 2 7 C2 9.8 4.2 12 7 12 C9.4 12 11.4 10.3 11.9 8 "
               "M12 1.5 L12 4.8 L8.7 4.8",
    "lock": "M2.5 6.5 L11.5 6.5 L11.5 12.5 L2.5 12.5 Z M4.5 6.5 L4.5 4.5 C4.5 1.2 9.5 1.2 9.5 4.5 L9.5 6.5",
    "clock": circle_path(7, 7, 5.5, 28) + " M7 3.8 L7 7 L9.5 8.5",
    "alert": "M7 1.5 L13 12.5 L1 12.5 Z M7 5.5 L7 8.8 M7 10.3 L7 11",
    "info": circle_path(7, 7, 5.5, 28) + " M7 6.2 L7 10.2 M7 3.9 L7 4.6",
    "search": circle_path(6, 6, 4.5, 24) + " M9.4 9.4 L12.8 12.8",
    "chevron-down": "M3.5 5.5 L7 9 L10.5 5.5",
    "logout": "M5.5 2 L2 2 L2 12 L5.5 12 M6 7 L12.5 7 M9.5 4 L12.5 7 L9.5 10",
}
BUTTON_KINDS = {"Primary": ("invert_bg", None, "invert_fg"), "Secondary": ("bg", "line_strong", "ink"),
                "Danger": ("bad", None, "on_bad")}


class Library:
    def __init__(self, d):
        self.d = d
        self.y = 0

    def row(self, h, gap=56):
        y = self.y
        self.y += h + gap
        return y

    def build(self, W):
        self.icons()
        self.brand()
        self.buttons()
        self.badges()
        self.cells()
        self.inputs()
        self.avatars()
        self.chat_row()
        self.messages()
        self.symbol_tile()
        self.header(W)
        self.footer(W)

    # --- icons ---------------------------------------------------------------------------------
    def icons(self):
        d = self.d
        y = self.row(14)
        for i, (n, path) in enumerate(ICONS.items()):
            c = d.comp(f"Icon/{n}", i * 40, y, 14, 14, fill=None, clip=False)
            d.node("vector", c, "glyph", 0, 0, path=path, closed=False, fill=None, stroke="ink", strokeWeight=1.5)

    # --- brand ---------------------------------------------------------------------------------
    def brand(self):
        """The logo (the engraved dolphin on the orchid tile, assets/logo) and the text wordmark."""
        d = self.d
        y = self.row(64)
        c = d.comp("Brand/Mark", 0, y, 64, 64, fill=None, clip=False)
        d.image(c, LOGO_256, 0, 0, 64, 64, name="Logo image")
        d.set(c, description="PingCraft logo: the dolphin on the orchid tile. Built by tools/logo.py from "
                             "assets/logo/pingcraft-logo.pcraft; every header and sheet hero uses this component.")
        w = d.comp("Brand/Wordmark", 120, y + 20, 10, 10, layout="horizontal", sizingH="hug", sizingV="hug")
        d.text(w, "Ping", "Display/Wordmark 18", "ink_strong", name="Ping")
        d.text(w, "Craft", "Display/Wordmark 18", "accent_ink", name="Craft")
        lk = d.comp("Brand/Lockup", 300, y + 18, 10, 10, layout="horizontal", gap=10, crossAlign="center",
                    sizingH="hug", sizingV="hug", clip=False)
        mk = d.inst("Brand/Mark", lk, name="Mark")
        d.R("transform.scale", {"ids": [mk], "factor": 24 / 64})
        d.inst("Brand/Wordmark", lk, name="Wordmark")

    # --- buttons -------------------------------------------------------------------------------
    def buttons(self):
        d = self.d
        y = self.row(34 * 3 + 32)
        ids = []
        for i, (kind, (fill, stroke, ink)) in enumerate(BUTTON_KINDS.items()):
            for j, state in enumerate(("Default", "Hover", "Focus", "Disabled")):
                f, s, k = fill, stroke, ink
                if state == "Hover" and kind == "Secondary":
                    f, s, k = "invert_bg", None, "invert_fg"
                c = d.comp(f"Kind={kind}, State={state}", j * 170, y + i * 50, 120, 34, fill=f, stroke=s,
                           layout="horizontal", padding=[0, 16, 0, 16], mainAlign="center", crossAlign="center",
                           sizingH="hug", sizingV="fixed")
                if state == "Hover" and kind != "Secondary":
                    d.set(c, opacity=0.8)
                if state == "Disabled":
                    d.set(c, opacity=0.5)
                if state == "Focus":
                    d.set(c, strokes=[{"type": "solid", "color": "#d58ae3"}], strokeWeight=2, strokeAlign="outside")
                    d.bind_stroke(c, "focus")
                t = d.text(c, "Retry", "Button/11", k, name="Label")
                d.prop(c, t, "Label")
                ids.append(c)
        d.variants(ids, "Button", 0, y)
        # Icon button: a leading 14 px icon (swapped per use), gap 8, padding 0 16 0 14.
        ids = []
        for i, (kind, (fill, stroke, ink)) in enumerate(BUTTON_KINDS.items()):
            c = d.comp(f"Kind={kind}", 720 + i * 170, y, 120, 34, fill=fill, stroke=stroke, layout="horizontal",
                       padding=[0, 16, 0, 14], gap=8, mainAlign="center", crossAlign="center", sizingH="hug",
                       sizingV="fixed")
            ic = d.inst("Icon/arrow-right", c, name="Icon")
            d.set(d.sub(ic, "glyph"), stroke=ink)
            t = d.text(c, "Sign in", "Button/11", ink, name="Label")
            d.prop(c, t, "Label")
            ids.append(c)
        d.variants(ids, "Icon button", 720, y)
        # Square 34 × 34 "…" button.
        c = d.comp("Button/More", 720, y + 60, 34, 34, fill="bg", stroke="line_strong", layout="horizontal",
                   mainAlign="center", crossAlign="center")
        d.text(c, "…", "Body/14 Medium", "ink", name="Label")

    # --- badges and tags -----------------------------------------------------------------------
    def badges(self):
        d = self.d
        y = self.row(20)
        ids = []
        labels = {"Good": "Low", "Warn": "Medium", "Bad": "Medium-high", "Accent": "Sending…", "Muted": "Later"}
        tones = {"Good": "good", "Warn": "warn", "Bad": "bad", "Accent": "accent_ink", "Muted": "muted"}
        for i, (tone, col) in enumerate(tones.items()):
            c = d.comp(f"Tone={tone}", i * 150, y, 60, 20, fill="sunken", stroke="line", layout="horizontal",
                       padding=[0, 6, 0, 6], mainAlign="center", crossAlign="center", sizingH="hug", sizingV="fixed")
            t = d.text(c, labels[tone], "Badge/10", col, name="Label")
            d.prop(c, t, "Label")
            ids.append(c)
        d.variants(ids, "Status badge", 0, y)
        # Network tag: the network's NAME, 18 tall. Neutral here; screens.tint_tag gives it the network's colour
        # (outline, name, faint wash) (no logos, and our own shades rather than the networks' brand codes).
        c = d.comp("Network tag", 800, y + 1, 60, 18, fill=None, stroke="line_strong", layout="horizontal",
                   padding=[0, 5, 0, 5], mainAlign="center", crossAlign="center", sizingH="hug", sizingV="fixed")
        t = d.text(c, "Telegram", "Badge/10", "muted", name="Network")
        d.prop(c, t, "Network")
        # Unread count: the accent as a fill under white on_accent text (4.5:1).
        c = d.comp("Unread count", 940, y + 1, 18, 18, fill="accent", layout="horizontal", padding=[0, 6, 0, 6],
                   mainAlign="center", crossAlign="center", sizingH="hug", sizingV="fixed")
        t = d.text(c, "3", "Badge/10", "on_accent", name="Count")
        d.prop(c, t, "Count")
        # Preview tag for later-milestone frames (1 px accent_ink border, Badge 10 accent_ink).
        c = d.comp("Preview tag", 1020, y, 60, 20, fill=None, stroke="accent_ink", layout="horizontal",
                   padding=[0, 6, 0, 6], mainAlign="center", crossAlign="center", sizingH="hug", sizingV="fixed")
        t = d.text(c, "Preview · M3", "Badge/10", "accent_ink", name="Label")
        d.prop(c, t, "Label")

    # --- cells ---------------------------------------------------------------------------------
    def cells(self):
        d = self.d
        y = self.row(34)
        ids = []
        for i, state in enumerate(("Off", "On", "Hover")):
            fill, ink = {"Off": (None, "muted"), "On": ("invert_bg", "invert_fg"), "Hover": ("sunken", "ink_strong")}[state]
            c = d.comp(f"State={state}", i * 160, y, 60, 34, fill=fill, layout="horizontal", padding=[0, 12, 0, 12],
                       gap=8, crossAlign="center", sizingH="hug", sizingV="fixed")
            t = d.text(c, "All", "HUD/11", ink, name="Label")
            n = d.text(c, "24", "HUD/11", ink if state == "On" else "muted", name="Count")
            if state == "On":
                d.set(n, opacity=0.6)
            d.prop(c, t, "Label")
            d.prop(c, n, "Count")
            d.R("component.addProperty", {"id": c, "name": "Show count", "type": "boolean", "layer": n,
                                          "default": True})
            ids.append(c)
        d.variants(ids, "Cell", 0, y)
        ids = []
        for i, state in enumerate(("Off", "On")):
            fill, ink = ("invert_bg", "invert_fg") if state == "On" else (None, "muted")
            c = d.comp(f"State={state}", 600 + i * 120, y + 5, 60, 24, fill=fill, layout="horizontal",
                       padding=[0, 8, 0, 8], crossAlign="center", sizingH="hug", sizingV="fixed")
            t = d.text(c, "Inbox", "HUD/11", ink, name="Label")
            d.prop(c, t, "Label")
            ids.append(c)
        d.variants(ids, "Nav cell", 600, y)
        # Checkbox, 16 px: Off is a line_strong box, On is inverted with a check.
        ids = []
        for i, state in enumerate(("Off", "On")):
            c = d.comp(f"State={state}", 900 + i * 60, y + 9, 16, 16,
                       fill="invert_bg" if state == "On" else "sunken",
                       stroke=None if state == "On" else "line_strong", clip=False)
            if state == "On":
                g = d.node("vector", c, "glyph", 3, 4, path="M0 4 L3 7 L9 0", closed=False, fill=None,
                           stroke="invert_fg", strokeWeight=1.75)
                del g
            ids.append(c)
        d.variants(ids, "Checkbox", 900, y + 9)

    # --- inputs --------------------------------------------------------------------------------
    def inputs(self):
        d = self.d
        y = self.row(34)
        ids = []
        for i, state in enumerate(("Default", "Focus")):
            c = d.comp(f"State={state}", i * 300, y, 260, 34, fill="sunken", stroke="line", layout="horizontal",
                       padding=[0, 10, 0, 10], gap=1, crossAlign="center", sizingV="fixed")
            if state == "Focus":
                d.set(c, strokes=[{"type": "solid", "color": "#d58ae3"}], strokeWeight=2)
                d.bind_stroke(c, "focus")
            t = d.text(c, "example.org", "Body/14", "ink", name="Value")
            d.prop(c, t, "Value")
            if state == "Focus":
                d.rect(c, 0, 0, 1, 18, "ink", name="caret")
            spacer(d, c)
            ids.append(c)
        d.variants(ids, "Input", 0, y)
        c = d.comp("Search field", 640, y, 200, 34, fill="sunken", stroke="line", layout="horizontal",
                   padding=[0, 10, 0, 10], gap=8, crossAlign="center", sizingV="fixed")
        ic = d.inst("Icon/search", c, name="Search icon")
        d.set(d.sub(ic, "glyph"), stroke="muted")
        t = d.text(c, "Search chats", "Body/14", "muted", name="Placeholder")
        d.prop(c, t, "Placeholder")

    # --- avatars -------------------------------------------------------------------------------
    def avatars(self):
        d = self.d
        y = self.row(36)
        for size, style, x in ((36, "Mono/13", 0), (28, "Badge/10", 80)):
            c = d.comp(f"Avatar/{size}", x, y, size, size, fill="sunken", stroke="line_strong", layout="horizontal",
                       mainAlign="center", crossAlign="center")
            t = d.text(c, "AO", style, "ink", name="Initials")
            d.prop(c, t, "Initials")

    # --- inbox row -----------------------------------------------------------------------------
    def chat_row(self, w=380):
        """One chat in the inbox: avatar, name, network tag, time / last message, unread count. 64 tall."""
        d = self.d
        y = self.row(64)
        ids = []
        for i, state in enumerate(("Read", "Unread", "Selected")):
            c = d.comp(f"State={state}", i * (w + 40), y, w, 64, fill="sunken" if state == "Selected" else "bg",
                       layout="horizontal", padding=[0, 16, 0, 16], gap=12, crossAlign="center", sizingV="fixed")
            side_stroke(d, c, bottom=1)
            av = d.inst("Avatar/36", c, name="Avatar")
            col = vstack(d, c, "Text", 0, 0, 200, gap=3)
            d.set(col, sizingH="fill")
            l1 = hstack(d, col, "Line 1", gap=8)
            d.set(l1, sizingH="fill")
            nm = d.text(l1, "Ana Ortiz", "Body/14 Medium" if state != "Read" else "Body/14",
                        "ink_strong" if state != "Read" else "ink", name="Name", w=120, lines=1)
            d.set(nm, sizingH="fill")
            tg = d.inst("Network tag", l1, name="Tag")
            tm = d.text(l1, "14:32", "Mono/12", "ink" if state != "Read" else "muted", name="Time", w=48,
                        align="right")
            l2 = hstack(d, col, "Line 2", gap=8)
            d.set(l2, sizingH="fill")
            pv = d.text(l2, "Sounds good, see you at eight!", "Small/12.5", "ink" if state != "Read" else "muted",
                        name="Preview", w=200, lines=1)
            d.set(pv, sizingH="fill")
            ct = d.inst("Unread count", l2, name="Unread")
            if state == "Read":
                d.set(ct, visible=False)
            if state == "Selected":
                rule = d.rect(c, 0, 0, 2, 64, "accent_ink", name="Selected rule")
                d.absolute(rule, 0, 0, 2, 64)
            for lid, pname in ((nm, "Name"), (tm, "Time"), (pv, "Preview")):
                d.prop(c, lid, pname)
            del av, tg
            ids.append(c)
        d.variants(ids, "Chat row", 0, y)

    # --- timeline ------------------------------------------------------------------------------
    def messages(self, w=720):
        d = self.d
        y = self.row(120)
        # Someone else's message: avatar, then name · time over the text. No bubble.
        c = d.comp("Message/Theirs", 0, y, w, 10, fill=None, layout="horizontal", gap=12, sizingV="hug",
                   crossAlign="min", clip=False)
        d.inst("Avatar/28", c, name="Avatar")
        col = vstack(d, c, "Text", 0, 0, 200, gap=2)
        d.set(col, sizingH="fill")
        hd = hstack(d, col, "Head", gap=8)
        n = d.text(hd, "Lars Holm", "Body/13 Medium", "ink_strong", name="Sender")
        t = d.text(hd, "14:28", "Mono/12", "muted", name="Time")
        b = d.text(col, "Halfway through chapter 9. No spoilers, please!", "Body/14", "ink", name="Body", w=300)
        d.set(b, sizingH="fill")
        for lid, pname in ((n, "Sender"), (t, "Time"), (b, "Body")):
            d.prop(c, lid, pname)
        # Your own message: a square box on the right with the accent wash and a 2 px accent_ink rule on
        # its right edge; the head says "You", the time and the state.
        c = d.comp("Message/Mine", w + 60, y, 420, 10, fill="accent_tint", layout="horizontal", gap=0,
                   sizingV="hug", crossAlign="min", clip=False)
        col = vstack(d, c, "Content", 0, 0, 400, gap=2, pad=[10, 14, 10, 14])
        d.set(col, sizingH="fill")
        hd = hstack(d, col, "Head", gap=8)
        d.text(hd, "You", "Body/13 Medium", "ink_strong", name="Sender")
        t = d.text(hd, "14:31", "Mono/12", "muted", name="Time")
        spacer(d, hd)
        st = hstack(d, hd, "State", gap=4)
        ic = d.inst("Icon/check", st, name="State icon")
        d.set(d.sub(ic, "glyph"), stroke="muted")
        s = d.text(st, "Sent", "Mono/12", "muted", name="State label")
        d.set(hd, sizingH="fill")
        b = d.text(col, "I won't say a word. See you Thursday.", "Body/14", "ink", name="Body", w=380)
        d.set(b, sizingH="fill")
        rule = d.rect(c, 0, 0, 2, 10, "accent_ink", name="Mine rule")
        d.set(rule, sizingV="fill")
        for lid, pname in ((t, "Time"), (s, "State"), (b, "Body")):
            d.prop(c, lid, pname)
        # Day divider: hairline, HUD label, hairline.
        y = self.row(20)
        c = d.comp("Day divider", 0, y, w, 20, fill=None, layout="horizontal", gap=12, crossAlign="center",
                   sizingV="fixed")
        a = d.rect(c, 0, 0, 10, 1, "line", name="Rule left")
        d.set(a, sizingH="fill")
        t = hud(d, c, "Today · 24 Nov", name="Day")
        b = d.rect(c, 0, 0, 10, 1, "line", name="Rule right")
        d.set(b, sizingH="fill")
        d.prop(c, t, "Day")

    # --- verification symbol -------------------------------------------------------------------
    def symbol_tile(self):
        """One of the 7 symbols compared when verifying a device (the Matrix SAS emoji, as monochrome
        glyphs: the headless build has no colour emoji font)."""
        d = self.d
        y = self.row(104)
        c = d.comp("Symbol tile", 0, y, 88, 104, fill="sunken", stroke="line", layout="vertical", gap=8,
                   padding=[18, 4, 14, 4], crossAlign="center", sizingV="fixed")
        g = d.text(c, "☂", "Symbol/34", "ink_strong", name="Symbol")
        n = d.text(c, "Umbrella", "Small/12", "muted", name="Symbol name")
        d.prop(c, g, "Symbol")
        d.prop(c, n, "Name")

    # --- window chrome -------------------------------------------------------------------------
    def header(self, W):
        d = self.d
        y = self.row(48)
        h = d.comp("Header/Desktop", 0, y, W, 48, fill="bg", layout="horizontal", crossAlign="center",
                   sizingV="fixed")
        side_stroke(d, h, bottom=1)

        def cell(nm, pad=(0, 16, 0, 16), left=False, right=False, gap=8, fixed=None):
            c = d.frame(h, nm, 0, 0, fixed or 10, 48, fill=None, layout="horizontal", padding=list(pad), gap=gap,
                        crossAlign="center", sizingH="fixed" if fixed else "hug", sizingV="fill", clip=False)
            if fixed:
                d.set(c, mainAlign="center")
            if left or right:
                side_stroke(d, c, left=1 if left else 0, right=1 if right else 0)
            return c

        b = cell("Brand", (0, 20, 0, 20), right=True)
        d.inst("Brand/Lockup", b, name="Lockup")
        n = cell("Nav", (0, 12, 0, 12), gap=4, right=True)
        for lab in ("Inbox", "Accounts", "Settings"):
            d.inst("Nav cell", n, props={"Label": lab, "State": "On" if lab == "Inbox" else "Off"}, name=f"Nav {lab}")
        spacer(d, h)
        st = cell("Status", left=True, gap=8)
        d.node("ellipse", st, "Status dot", 0, 0, 8, 8, fill="good")
        d.text(st, "Connected", "Body/13", "ink", name="Status text")
        for k, path in (("Minimize", "M0 5 L10 5"), ("Maximize", "M0 0 L10 0 L10 10 L0 10 Z"),
                        ("Close", "M0 0 L10 10 M10 0 L0 10")):
            c = cell(k, (0, 0, 0, 0), left=(k == "Minimize"), fixed=46)
            d.node("vector", c, "glyph", 0, 0, path=path, closed=(k == "Maximize"), fill=None, stroke="muted",
                   strokeWeight=1)

    def footer(self, W):
        d = self.d
        y = self.row(34)
        f = d.comp("Footer/Desktop", 0, y, W, 34, fill="bg", layout="horizontal", padding=[0, 20, 0, 20],
                   crossAlign="center")
        side_stroke(d, f, top=1)
        lt = d.text(f, "PingCraft 0.1.0 · @john:example.org", "HUD/11", "muted", name="Left")
        spacer(d, f)
        rt = d.text(f, "4 accounts · synced just now", "Small/12", "muted", name="Right")
        d.prop(f, lt, "Left")
        d.prop(f, rt, "Right")


__all__ = ["Library", "ICONS", "BUTTON_KINDS", "circle_path", "box_stroke"]
