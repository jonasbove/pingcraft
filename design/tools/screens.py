"""PingCraft screens: the M2 client and the M3 preview (pages "M2 · Client" and "M3 · Preview").

Layout language: the Craft launcher's, as CraftCloud's v2 design uses it (CraftCloud `design/tools/v2.py`,
whose helpers `headline`, `meta`, `sheet`, `field` and `panel` are adapted here). Sample people, chats
and messages are invented. Words follow PingCraft's GLOSSARY.md: network, account, chat, inbox,
message, bridge, risk level; never "room".
"""

from lib import ACCENT_INK, box_stroke, hstack, hud, side_stroke, spacer, vstack
from components import BUTTON_KINDS

W, H = 1200, 760
GAP = 160
HEADER, FOOTER = 48, 34
LEFT_W = 380
ME = "@john:example.org"
VERSION = "0.1.0"

# --- sample data ------------------------------------------------------------------------------------
# name, initials, network, time, preview, unread
CHATS = [
    ("Book club", "BC", "Signal", "14:32", "Lars: The ending of chapter 8, though. I had to put it down", 0),
    ("Ana Ortiz", "AO", "Telegram", "14:05", "Sounds good, see you at eight!", 1),
    ("Family", "FA", "WhatsApp", "12:48", "Mum: The photos from Sunday are in the album", 5),
    ("Lars Holm", "LH", "Matrix", "11:21", "You: I'll bring the printouts.", 0),
    ("Mei Tanaka", "MT", "Telegram", "Mon", "Thanks again for helping with the move!", 0),
    ("Climbing crew", "CC", "WhatsApp", "Mon", "Omar: The wall is open until 22 on Fridays", 0),
    ("Sam Weber", "SW", "Signal", "Sun", "You: Happy birthday, Sam!", 0),
    ("Noor Haddad", "NH", "Telegram", "21 Nov", "Can you send me the address?", 0),
    ("Garden plot", "GP", "Matrix", "20 Nov", "Kofi: The watering rota for December is up", 0),
]
TOTAL_CHATS = 24
UNREAD_CHATS = sum(1 for c in CHATS if c[5])
# The 7 symbols compared when verifying a device: Matrix SAS emoji, drawn as monochrome glyphs.
SAS = [("☂", "Umbrella"), ("⚓", "Anchor"), ("☎", "Telephone"), ("✈", "Aeroplane"), ("♥", "Heart"),
       ("✂", "Scissors"), ("✏", "Pencil")]
RECOVERY_KEY = ("EsTc 8Jq2 Lm4P vR7x Wn3K hY6d", "Qa9B tF2s Zp5G kU8e Nr4V jD7w")


# --- generic helpers --------------------------------------------------------------------------------

def screen(d, name, x, y, light=False, nav="Inbox", status=("good", "Connected"), footer=None, w=W, h=H):
    f = d.frame(None, name, x, y, w, h, fill="bg")
    if light:
        d.light(f)
    hid = d.inst("Header/Desktop", f, 0, 0, name="Header")
    for lab in ("Inbox", "Accounts", "Settings"):
        d.R("instance.setProperties", {"ids": [d.sub(hid, f"Nav {lab}")],
                                       "properties": {"State": "On" if lab == nav else "Off"}})
    if nav is None:
        d.set(d.sub(hid, "Nav"), visible=False)
    d.set(d.sub(hid, "Status dot"), fill=status[0])
    d.set(d.sub(hid, "Status text"), characters=status[1])
    ft = d.inst("Footer/Desktop", f, 0, h - FOOTER, name="Footer")
    if footer:
        d.R("instance.setProperties", {"ids": [ft], "properties": {"Left": footer[0], "Right": footer[1]}})
    return f


def headline(d, parent, text, light=False, name="Headline"):
    """Two lines; the word in *stars* is accent_ink (a literal per mode: text ranges can't bind)."""
    plain = text.replace("*", "")
    t = d.text(parent, plain, "Display/Hero 40", "ink_strong", name=name)
    a = text.index("*")
    b = text.index("*", a + 1) - 1
    d.R("text.style", {"id": t, "start": a, "end": b, "textFills": [ACCENT_INK[light]]})
    return t


def meta(d, parent, parts, link=None, name="Meta"):
    r = hstack(d, parent, name, gap=10)
    for i, p in enumerate(parts):
        if i:
            hud(d, r, "·", name="dot")
        t = hud(d, r, p, name=p[:30])
        if p == link:
            d.set(t, textDecoration="underline")
    return r


def ibtn(d, parent, label, icon, kind="Primary", name=None, disabled=False):
    iid = d.inst("Icon button", parent, props={"Kind": kind, "Label": label}, name=name or label)
    ic = d.sub(iid, "Icon")
    d.R("instance.swap", {"ids": [ic], "component": d.C[f"Icon/{icon}"]})
    d.set(d.sub(d.sub(iid, "Icon"), "glyph"), stroke=BUTTON_KINDS[kind][2])
    if disabled:
        d.set(iid, opacity=0.5)
    return iid


def btn(d, parent, label, kind="Secondary", state="Default", name=None):
    return d.inst("Button", parent, props={"Kind": kind, "Label": label, "State": state}, name=name or label)


def icon(d, parent, name, color="muted", label=None):
    i = d.inst(f"Icon/{name}", parent, name=label or f"{name} icon")
    d.set(d.sub(i, "glyph"), stroke=color)
    return i


def cells(d, parent, labels, sel=None, counts=None, name="Cells"):
    box = d.frame(parent, name, 0, 0, 10, 34, stroke="line", layout="horizontal", sizingH="hug", sizingV="fixed",
                  clip=False)
    for i, lab in enumerate(labels):
        if i:
            d.rect(box, 0, 0, 1, 34, "line", name="sep")
        props = {"Label": lab, "Show count": bool(counts), "State": "On" if lab == sel else "Off"}
        if counts:
            props["Count"] = str(counts[i])
        d.inst("Cell", box, props=props, name=f"{name} {lab}")
    return box


def note(d, parent, text, color="muted", style="Small/12.5", name="Note", w=200):
    t = d.text(parent, text, style, color, name=name, w=w)
    d.set(t, sizingH="fill")
    return t


def para(d, parent, text, color="ink", style="Body/14", name="Paragraph"):
    return note(d, parent, text, color, style, name)


def field(d, parent, label, value, state="Default", hint=None, button=None, placeholder=False):
    fl = vstack(d, parent, f"Field {label}", gap=6)
    d.set(fl, sizingH="fill")
    hud(d, fl, label, color="ink", name=f"{label} label")
    r = hstack(d, fl, f"{label} row", gap=8)
    d.set(r, sizingH="fill")
    i = d.inst("Input", r, props={"State": state, "Value": value}, name=f"{label} input")
    d.set(i, sizingH="fill")
    if placeholder:
        d.set(d.sub(i, "Value"), fill="muted")
    if button:
        btn(d, r, button)
    if hint:
        note(d, fl, hint, name=f"{label} hint")
    return fl


def checkbox(d, parent, text, on=True):
    r = hstack(d, parent, "Confirm", gap=12, crossAlign="min")
    d.set(r, sizingH="fill")
    cb = d.inst("Checkbox", r, props={"State": "On" if on else "Off"}, name="Checkbox")
    d.set(cb, y=2)
    note(d, r, text, "ink", "Body/14", name="Checkbox label")
    return r


def panel(d, parent, title, gap=8, pad=(12, 14, 12, 14)):
    p = vstack(d, parent, f"Panel {title}", gap=gap, pad=list(pad))
    d.set(p, sizingH="fill")
    box_stroke(d, p, "line")
    hud(d, p, title, name="Panel title")
    return p


def scrim(d, f):
    return d.rect(f, 0, 0, W, H, "scrim", name="Scrim")


def sheet(d, f, w, title, caption, right=None, name="Sheet", top=None):
    """A sheet over the window: bg, 1 px line_strong, dialog shadow; hero with the accent wash, a 2 px
    accent rule and the 44 px mark; body padding 24, gap 16. It hugs its content and is centred."""
    s = d.frame(f, name, 0, 0, w, 10, fill="bg", stroke="line_strong", layout="vertical", sizingV="hug", clip=True)
    d.effect_style(s, "Shadow/Dialog")
    hd = d.frame(s, "Hero", 0, 0, w, 10, fill="accent_tint", layout="horizontal", gap=14, padding=[18, 24, 18, 24],
                 crossAlign="center", sizingH="fill", sizingV="hug", clip=False)
    side_stroke(d, hd, bottom=1)
    rule = d.rect(hd, 0, 0, w, 2, "accent", name="Accent rule")
    d.absolute(rule, 0, 0, w, 2)
    mk = d.inst("Brand/Mark", hd, name="Hero mark")
    d.R("transform.scale", {"ids": [mk], "factor": 44 / 64})
    col = vstack(d, hd, "Titles", gap=4)
    d.set(col, sizingH="fill")
    d.text(col, title, "Display/Sheet title 26", "ink_strong", name="Sheet title")
    if caption:
        hud(d, col, caption, name="Sheet caption")
    if right:
        right(hd)
    body = vstack(d, s, "Sheet body", gap=16, pad=[24, 24, 24, 24])
    d.set(body, sizingH="fill")
    return s, body


def sheet_actions(d, s, left=None, right=None):
    r = d.frame(s, "Sheet actions", 0, 0, 10, 62, fill=None, layout="horizontal", gap=8, padding=[14, 24, 14, 24],
                crossAlign="center", sizingH="fill", sizingV="hug", clip=False)
    side_stroke(d, r, top=1)
    if left:
        left(r)
    spacer(d, r)
    if right:
        right(r)
    return r


def centre(d, s, w):
    h = d.get(s, "height")["height"]
    d.set(s, x=round((W - w) / 2), y=round(HEADER + (H - HEADER - FOOTER - h) / 2))


def set_text(d, root, layer, text):
    d.set(d.sub(root, layer), characters=text)


# Network tags take their network's colour (lib.THEME net_*): outline and name in it, over a faint wash of it.
# Matrix stays the neutral outline.
NET_COLOUR = {"Telegram": "net_telegram", "Signal": "net_signal", "Discord": "net_discord", "WhatsApp": "net_whatsapp"}


def tint_tag(d, root, net):
    """Colour the network tag under `root` (an instance, or the tag itself) for `net`."""
    colour = NET_COLOUR.get(net.title() if net.isupper() else net)
    if not colour:
        return
    tag = root if d.get(root, "name")["name"] == "Tag" else d.sub(root, "Tag")
    d.set(tag, fill=colour + "_wash", stroke=colour)
    d.set(d.sub(tag, "Network"), fill=colour)


# --- the inbox ----------------------------------------------------------------------------------------

# The networks the inbox can show, in the order of the network filter. Discord is not connectable yet (ADR 0008).
FILTER_NETWORKS = ("Telegram", "Signal", "WhatsApp", "Matrix")


def network_chip(d, parent, net, on):
    """One toggle in the network filter: on = the network's colour (outline, name, wash); off = neutral and faint."""
    colour = NET_COLOUR.get(net)
    c = hstack(d, parent, f"Network {net}", gap=0, h=24, padding=[0, 8, 0, 8])
    if on and colour:
        d.set(c, fill=colour + "_wash")
        box_stroke(d, c, colour)
        d.text(c, net, "Badge/10", colour, name="Network")
    elif on:
        box_stroke(d, c, "line_strong")
        d.text(c, net, "Badge/10", "ink", name="Network")
    else:
        box_stroke(d, c, "line")
        d.text(c, net, "Badge/10", "faint", name="Network")
    return c


def inbox_column(d, f, selected, hidden=()):
    """The inbox. `hidden` names networks switched off in the network filter: their chats leave the list."""
    col = d.frame(f, "Inbox", 0, HEADER, LEFT_W, H - HEADER - FOOTER, fill="bg", clip=True)
    side_stroke(d, col, right=1)
    tb = hstack(d, col, "Filter row", 16, 16, gap=8, h=34)
    d.set(tb, sizingH="fixed", width=LEFT_W - 32)
    chats = [c for c in CHATS if c[2] not in hidden]
    shown = TOTAL_CHATS - (len(CHATS) - len(chats)) * 2 if hidden else TOTAL_CHATS
    unread = sum(1 for c in chats if c[5])
    cells(d, tb, ("All", "Unread"), sel="All", counts=(shown, unread), name="Filter")
    sf = d.inst("Search field", tb, props={"Placeholder": "Search chats"}, name="Search")
    d.set(sf, sizingH="fill")
    nf = hstack(d, col, "Network filter", 16, 60, gap=6, h=24)
    for net in FILTER_NETWORKS:
        network_chip(d, nf, net, net not in hidden)
    parts = ("Inbox", "4 accounts", f"{shown} chats") + ((f"{len(hidden)} hidden",) if hidden else ())
    meta(d, col, parts, name="Inbox meta")
    d.set(d.sub(col, "Inbox meta"), x=16, y=98)
    lst = vstack(d, col, "Chats", 0, 122, LEFT_W, gap=0)
    side_stroke(d, lst, top=1)
    for name, ini, net, time, preview, unread in chats:
        state = "Selected" if name == selected else ("Unread" if unread else "Read")
        r = d.inst("Chat row", lst, props={"State": state, "Name": name, "Time": time, "Preview": preview},
                   name=f"Chat {name}")
        set_text(d, r, "Initials", ini)
        set_text(d, r, "Network", net)
        tint_tag(d, r, net)
        if unread:
            set_text(d, r, "Count", str(unread))
        else:
            d.set(d.sub(r, "Unread"), visible=False)
    return col


def chat_pane(d, f, name, ini, net, account, enc):
    x0, w = LEFT_W, W - LEFT_W
    pane = d.frame(f, "Chat", x0, HEADER, w, H - HEADER - FOOTER, fill="bg", layout="vertical", clip=True)
    hd = d.frame(pane, "Chat header", 0, 0, w, 64, fill=None, layout="horizontal", gap=12, padding=[0, 24, 0, 24],
                 crossAlign="center", sizingH="fill", sizingV="fixed", clip=False)
    side_stroke(d, hd, bottom=1)
    av = d.inst("Avatar/36", hd, name="Avatar")
    set_text(d, av, "Initials", ini)
    tc = vstack(d, hd, "Chat title", gap=3)
    d.text(tc, name, "Body/16 Medium", "ink_strong", name="Chat name")
    l2 = hstack(d, tc, "Where", gap=8)
    tag = d.inst("Network tag", l2, props={"Network": net}, name="Tag")
    tint_tag(d, tag, net)
    d.text(l2, account, "Small/12.5", "muted", name="Account")
    spacer(d, hd)
    ec = hstack(d, hd, "Encryption", gap=8, h=30, padding=[0, 10, 0, 10])
    box_stroke(d, ec, "line")
    icon(d, ec, "lock", "good" if enc[0] else "muted")
    d.text(ec, enc[1], "Small/12.5", "ink", name="Encryption label")
    d.inst("Button/More", hd, name="More")
    tl = d.frame(pane, "Timeline", 0, 0, w, 10, fill=None, layout="vertical", gap=14, padding=[20, 24, 20, 24],
                 mainAlign="max", sizingH="fill", sizingV="fill", clip=True)
    cp = d.frame(pane, "Composer", 0, 0, w, 74, fill=None, layout="horizontal", gap=8, padding=[20, 24, 20, 24],
                 crossAlign="center", sizingH="fill", sizingV="fixed", clip=False)
    side_stroke(d, cp, top=1)
    inp = d.inst("Input", cp, props={"Value": f"Message {name}"}, name="Message input")
    d.set(inp, sizingH="fill")
    d.set(d.sub(inp, "Value"), fill="muted")
    ibtn(d, cp, "Send", "arrow-right", name="Send")
    return tl


def divider(d, tl, day):
    dv = d.inst("Day divider", tl, props={"Day": day}, name=f"Divider {day}")
    d.set(dv, sizingH="fill")


def theirs(d, tl, sender, ini, time, body):
    m = d.inst("Message/Theirs", tl, props={"Sender": sender, "Time": time, "Body": body}, name=f"From {sender}")
    d.set(m, sizingH="fill")
    set_text(d, m, "Initials", ini)
    return m


def mine(d, tl, time, body, state="Sent", w=None):
    row = hstack(d, tl, f"Mine {time}", gap=0)
    d.set(row, sizingH="fill", mainAlign="max")
    m = d.inst("Message/Mine", row, props={"Time": time, "State": state, "Body": body}, name="Mine")
    # Box width follows the text: measured once, between 260 and 460 (egui: wrap at 460).
    probe = d.text(row, body, "Body/14", "ink", name="probe")
    tw = d.get(probe, "width")["width"]
    d.R("edit.delete", {"ids": [probe]})
    d.set(m, width=max(260, min(460, round(tw) + 32)))
    return m


def inbox_frame(d, name, x, y, light=False, hidden=()):
    f = screen(d, name, x, y, light=light)
    inbox_column(d, f, "Book club", hidden=hidden)
    tl = chat_pane(d, f, "Book club", "BC", "Signal", "personal · 6 people", (False, "Encrypted · via your bridge"))
    divider(d, tl, "Yesterday · 23 Nov")
    theirs(d, tl, "Lars Holm", "LH", "21:10", "Next meeting on Thursday at mine? I'll make soup.")
    mine(d, tl, "21:14", "Thursday works. I'll bring bread.")
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Ana Ortiz", "AO", "09:02",
           "Has everyone got the book? The library on Elm Street had two copies left this morning.")
    theirs(d, tl, "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!")
    mine(d, tl, "14:31", "I won't say a word. See you Thursday.")
    theirs(d, tl, "Lars Holm", "LH", "14:32", "The ending of chapter 8, though. I had to put it down for a minute.")
    return f


def inbox_empty(d, name, x, y):
    f = screen(d, name, x, y, footer=(f"PingCraft {VERSION} · {ME}", "No accounts yet"))
    left = vstack(d, f, "Hero", 48, 196, 560, gap=12)
    hud(d, left, "Inbox · no accounts yet", name="Kicker")
    headline(d, left, "Your chats,\n*one* inbox.")
    body = vstack(d, left, "Body", gap=10)
    d.set(body, sizingH="fill", padding=[4, 0, 12, 0])
    para(d, body, "PingCraft gathers your chats from WhatsApp, Signal, Telegram and Discord into this one list. "
                  "Each network is connected through a bridge: a small program on your server that carries "
                  "messages between that network and you.", "muted")
    para(d, body, "Connecting a network from inside PingCraft comes in a later version. For now, the bridge is "
                  "set up on your server and you log in to the network through it.", "muted")
    br = hstack(d, left, "Buttons", gap=8)
    ibtn(d, br, "How to connect", "external", name="How to connect")
    ibtn(d, br, "Check again", "refresh", "Secondary", name="Check again")
    m = meta(d, left, ("Signed in as " + ME, "Encryption set up", "0 accounts"))
    d.set(m, padding=[8, 0, 0, 0])
    right = vstack(d, f, "Networks", 680, 196, 472, gap=12)
    p = panel(d, right, "Networks you can connect", gap=0, pad=(12, 14, 4, 14))
    d.set(d.sub(p, "Panel title"), name="Panel title")
    rows = (("Telegram", "Good", "Low", "Phone number and a code, or a QR code"),
            ("Signal", "Warn", "Medium", "Linked device: scan a QR code"),
            ("WhatsApp", "Bad", "Medium-high", "Linked device: QR code or pairing code"),
            ("Discord", "Muted", "Later", "Waits for its bridge's next version"))
    for i, (net, tone, risk, how) in enumerate(rows):
        r = hstack(d, p, f"Network {net}", gap=12, h=56)
        d.set(r, sizingH="fill")
        if i:
            side_stroke(d, r, top=1)
        tc = vstack(d, r, "Text", gap=2)
        d.set(tc, sizingH="fill")
        d.text(tc, net, "Body/14 Medium", "ink_strong", name="Network name")
        d.text(tc, how, "Small/12", "muted", name="How")
        d.inst("Status badge", r, props={"Tone": tone, "Label": risk}, name="Risk")
    d.set(p, padding=[12, 14, 0, 14])
    note(d, right, "Risk level: how likely the network is to restrict or ban an account for being connected "
                   "through a bridge. PingCraft shows it again before you connect.")
    return f


def chat_states(d, name, x, y):
    f = screen(d, name, x, y)
    inbox_column(d, f, "Lars Holm")
    tl = chat_pane(d, f, "Lars Holm", "LH", "Matrix", "example.org · direct chat", (True, "End-to-end encrypted"))
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Lars Holm", "LH", "10:02", "Morning! Did you get a chance to look at the draft?")
    # A message this device can't decrypt yet.
    u = d.frame(tl, "Waiting for key", 0, 0, 10, 10, fill=None, layout="horizontal", gap=12, sizingH="fill",
                sizingV="hug", crossAlign="min", clip=False)
    av = d.inst("Avatar/28", u, name="Avatar")
    set_text(d, av, "Initials", "LH")
    col = vstack(d, u, "Text", gap=6)
    d.set(col, sizingH="fill")
    hd = hstack(d, col, "Head", gap=8)
    d.text(hd, "Lars Holm", "Body/13 Medium", "ink_strong", name="Sender")
    d.text(hd, "10:05", "Mono/12", "muted", name="Time")
    bx = vstack(d, col, "Locked message", gap=4, pad=[10, 14, 10, 14], fill="sunken")
    box_stroke(d, bx, "line")
    d.set(bx, width=520)
    t1 = hstack(d, bx, "Title", gap=8)
    icon(d, t1, "lock", "muted")
    d.text(t1, "Waiting for the key to this message", "Body/14 Medium", "ink", name="Locked title")
    note(d, bx, "It was sent before this device could read it. It opens by itself once one of your other "
                "devices, or your key backup, shares the key.")
    lm = d.text(bx, "Why this happens", "Small/12.5", "accent_ink", name="Learn more")
    d.set(lm, textDecoration="underline")
    mine(d, tl, "11:12", "Not yet, sorry. I'll send it tonight.", "Sent")
    m = mine(d, tl, "11:20", "Also: are we still on for Thursday?", "Sending…")
    st = d.sub(m, "State icon")
    d.R("instance.swap", {"ids": [st], "component": d.C["Icon/clock"]})
    d.set(d.sub(d.sub(m, "State icon"), "glyph"), stroke="muted")
    m = mine(d, tl, "11:21", "I'll bring the printouts.", "Not sent")
    d.set(m, width=460)
    st = d.sub(m, "State icon")
    d.R("instance.swap", {"ids": [st], "component": d.C["Icon/alert"]})
    d.set(d.sub(d.sub(m, "State icon"), "glyph"), stroke="bad")
    d.set(d.sub(m, "State label"), fill="bad")
    d.set(d.sub(m, "Mine rule"), fill="bad")
    # The failed message's own row: what happened, and RETRY (the instance is detached to add it).
    d.R("component.detach", {"ids": [m]})
    box = d.sub(d.sub(tl, "Mine 11:21"), "Mine")
    content = d.sub(box, "Content")
    fr = hstack(d, content, "Failed", gap=8)
    d.set(fr, sizingH="fill", padding=[8, 0, 0, 0])
    note(d, fr, "The server didn't answer.", "ink", "Small/12.5", name="Failed text")
    btn(d, fr, "Delete", name="Delete")
    ibtn(d, fr, "Retry", "refresh", "Primary", name="Retry")
    d.set([content, box], sizingV="hug")
    return f


def sign_in(d, name, x, y):
    f = screen(d, name, x, y, nav=None, status=("muted", "Not signed in"),
               footer=(f"PingCraft {VERSION} · not signed in", "Your server, your messages"))
    scrim(d, f)
    w = 560
    s, b = sheet(d, f, w, "Sign in", "To your PingCraft server")
    field(d, b, "Server address", "example.org",
          hint="The server you run, or the one your admin runs for you. PingCraft talks only to this server, "
               "and your messages are kept there, not with us or anyone else.")
    field(d, b, "Username", "john", state="Focus")
    field(d, b, "Password", "••••••••••••")
    sheet_actions(d, s, left=lambda r: d.text(r, "Forgot your password? Ask your server's admin.", "Small/12.5",
                                              "muted", name="Forgot"),
                  right=lambda r: ibtn(d, r, "Sign in", "arrow-right", name="Sign in"))
    centre(d, s, w)
    return f


def recovery_key(d, name, x, y):
    f = screen(d, name, x, y, nav=None, status=("good", "Signed in"),
               footer=(f"PingCraft {VERSION} · {ME}", "First sign-in"))
    scrim(d, f)
    w = 680
    s, b = sheet(d, f, w, "Save your recovery key", f"First sign-in · {ME}")
    para(d, b, "Your messages are encrypted. This key unlocks them on a new device: PingCraft keeps a backup of "
               "your message keys on your server, locked with it.", "ink")
    kb = vstack(d, b, "Key", gap=0, pad=[14, 20, 14, 20], fill="sunken")
    d.set(kb, sizingH="fill")
    box_stroke(d, kb, "line")
    for i, line in enumerate(RECOVERY_KEY):
        d.text(kb, line, "Mono/22 Key", "ink_strong", name=f"Key line {i + 1}")
    br = hstack(d, b, "Key buttons", gap=8)
    ibtn(d, br, "Copy", "copy", "Secondary", name="Copy")
    ibtn(d, br, "Save to file", "save", "Secondary", name="Save to file")
    p = panel(d, b, "If you lose it")
    note(d, p, "If you lose every device you're signed in on and this key, your encrypted message history is "
               "gone for good. Nobody can get it back for you, not even your server's admin.", "ink", "Body/13")
    checkbox(d, b, "I've saved my recovery key somewhere safe, away from this computer.")
    sheet_actions(d, s, left=lambda r: d.text(r, "Keep it like a password: never send it in a chat.", "Small/12.5",
                                              "muted", name="Hint"),
                  right=lambda r: ibtn(d, r, "Continue", "arrow-right", name="Continue"))
    centre(d, s, w)
    return f


def verify_device(d, name, x, y):
    f = inbox_frame(d, name, x, y)
    scrim(d, f)
    w = 720
    s, b = sheet(d, f, w, "Verify your other device", "Laptop · asked at 14:40")
    para(d, b, "Your laptop is showing 7 symbols. Check that they are the same here, in the same order. If they "
               "are, the laptop is really yours and can read your encrypted messages.", "ink")
    row = hstack(d, b, "Symbols", gap=8)
    for g, n in SAS:
        d.inst("Symbol tile", row, props={"Symbol": g, "Name": n}, name=f"Symbol {n}")
    r = hstack(d, b, "Mismatch note", gap=8, crossAlign="min")
    d.set(r, sizingH="fill")
    icon(d, r, "info", "muted")
    note(d, r, "If they don't match, someone may be trying to get into your account. Choose “They don't match”: "
               "nothing is shared, and you can try again.")
    sheet_actions(d, s, left=lambda r: ibtn(d, r, "They don't match", "x", "Secondary", name="They don't match"),
                  right=lambda r: ibtn(d, r, "They match", "check", name="They match"))
    centre(d, s, w)
    return f


def unlock(d, name, x, y):
    f = screen(d, name, x, y, nav=None, status=("muted", "Locked"),
               footer=(f"PingCraft {VERSION} · {ME}", "Locked"))
    scrim(d, f)
    w = 560
    s, b = sheet(d, f, w, "Unlock PingCraft", ME)
    para(d, b, "Type your passphrase to open the messages kept on this computer. There is no keychain here, so "
               "PingCraft locks them with your passphrase instead.", "ink")
    field(d, b, "Passphrase", "••••••••••••••••", state="Focus", button="Show")
    note(d, b, "Forgot it? Sign out, then sign in again with your password and recovery key. What's kept on this "
               "computer is cleared and comes back from your server.")
    sheet_actions(d, s, left=lambda r: ibtn(d, r, "Sign out", "logout", "Secondary", name="Sign out"),
                  right=lambda r: ibtn(d, r, "Unlock", "key", name="Unlock"))
    centre(d, s, w)
    return f


# --- M3 preview -------------------------------------------------------------------------------------------

NETWORKS = [
    # name, tone, risk, how you log in, what it says about accounts, available
    ("Telegram", "Good", "Low", "Log in with your phone number and a code sent to Telegram, or scan a QR code.",
     "1 account · personal", True),
    ("Signal", "Warn", "Medium", "Link PingCraft as a device: scan a QR code with Signal on your phone.",
     "1 account · personal", True),
    ("WhatsApp", "Bad", "Medium-high", "Link PingCraft as a device: scan a QR code or type a pairing code.",
     "No accounts yet", True),
    ("Discord", "Muted", "Later", "Comes with its bridge's next version. A bot login is low risk; logging in as "
     "yourself is high.", "Not available yet", False),
]


def preview_tag(d, parent):
    return d.inst("Preview tag", parent, props={"Label": "Preview · M3"}, name="Preview tag")


def connect_network(d, name, x, y, light=False):
    f = screen(d, name, x, y, light=light, nav="Accounts",
               footer=(f"PingCraft {VERSION} · {ME}", "3 accounts"))
    hero = vstack(d, f, "Hero", 24, 72, 760, gap=12)
    kr = hstack(d, hero, "Kicker row", gap=12)
    hud(d, kr, "Accounts · connect a network", name="Kicker")
    preview_tag(d, kr)
    headline(d, hero, "Connect a\n*network*.", light=light)
    para(d, hero, "Each network is connected through a bridge on your server. Choose one to see what connecting "
                  "it means for your account, before you log in.", "muted")
    grid = d.frame(f, "Networks", 24, 296, W - 48, 10, fill=None, layout="horizontal", gap=0, sizingH="fixed",
                   sizingV="hug", clip=True)
    box_stroke(d, grid, "line")
    cw = (W - 48) / 4
    for i, (net, tone, risk, how, accounts, ok) in enumerate(NETWORKS):
        c = vstack(d, grid, f"Card {net}", gap=0)
        d.set(c, sizingH="fixed", width=cw, sizingV="fill")
        if i:
            side_stroke(d, c, left=1)
        strip = hstack(d, c, "Strip", gap=8, h=40, padding=[0, 16, 0, 20])
        d.set(strip, sizingH="fill")
        side_stroke(d, strip, bottom=1)
        hud(d, strip, f"{i + 1:02d}", name="Number")
        spacer(d, strip)
        d.inst("Status badge", strip, props={"Tone": tone, "Label": risk}, name="Risk")
        body = vstack(d, c, "Body", gap=10, pad=[18, 20, 20, 20])
        d.set(body, sizingH="fill", sizingV="fill")
        d.text(body, net, "Display/Heading 22", "ink_strong" if ok else "muted", name="Network name")
        note(d, body, how, "muted", "Small/12.5", name="How")
        spacer(d, body, v=True)
        hud(d, body, accounts, name="Accounts")
        if ok:
            ibtn(d, body, "Connect", "plus", "Primary" if net == "WhatsApp" else "Secondary", name="Connect")
        else:
            ibtn(d, body, "Not yet", "clock", "Secondary", name="Not yet", disabled=True)
    d.set(grid, height=290)
    for k in d.find(grid, "Body"):
        d.set(k, sizingV="fill")
    d.text(f, "Risk level: how likely the network is to restrict or ban an account for being connected through a "
              "bridge. Each network's risk is explained before you connect it.", "Small/12.5", "muted",
           x=24, y=610, w=760, name="Risk note")
    return f


def connect_risk(d, name, x, y):
    f = connect_network(d, name, x, y)
    scrim(d, f)
    w = 700
    s, b = sheet(d, f, w, "Before you connect WhatsApp", "Risk level · medium-high",
                 right=lambda hd: preview_tag(d, hd))
    sc = hstack(d, b, "Scale", gap=12)
    hud(d, sc, "Risk level", name="Scale label")
    cells(d, sc, ("Low", "Medium", "Medium-high", "High"), sel="Medium-high", name="Risk scale")
    for title, text in (
        ("What could happen",
         "WhatsApp's terms don't allow unofficial apps, and WhatsApp does ban accounts it finds suspicious. "
         "PingCraft can't promise that yours won't be one of them."),
        ("What makes it more likely",
         "A new account, a number from an internet phone service, and messaging people who haven't saved "
         "your number."),
        ("Keep your phone online",
         "PingCraft joins as a linked device. If your phone stays offline for about 14 days, the link drops. "
         "PingCraft warns you in the inbox after 12."),
        ("Where your messages are",
         "The WhatsApp bridge on your server can read your messages so it can pass them on. Keep your "
         "server's backups as safe as your phone."),
    ):
        blk = vstack(d, b, f"About {title}", gap=4)
        d.set(blk, sizingH="fill")
        hud(d, blk, title, color="ink", name="Block title")
        note(d, blk, text, "muted", "Body/13", name="Block text")
    checkbox(d, b, "I understand that WhatsApp may restrict or ban this account, and I want to connect it anyway.")
    sheet_actions(d, s, left=lambda r: btn(d, r, "Cancel"),
                  right=lambda r: ibtn(d, r, "Connect WhatsApp", "arrow-right", name="Connect WhatsApp"))
    centre(d, s, w)
    return f


# --- pages --------------------------------------------------------------------------------------------

M2 = [("sign-in", sign_in), ("recovery-key", recovery_key), ("inbox", inbox_frame),
      ("inbox-light", lambda d, n, x, y: inbox_frame(d, n, x, y, light=True)),
      ("inbox-networks", lambda d, n, x, y: inbox_frame(d, n, x, y, hidden=("WhatsApp", "Matrix"))),
      ("inbox-empty", inbox_empty),
      ("chat-states", chat_states), ("verify-device", verify_device), ("unlock", unlock)]
M3 = [("connect-network", connect_network), ("connect-risk", connect_risk)]


def build(d, only=None):
    out = []
    for page, items in (("M2 · Client", M2), ("M3 · Preview", M3)):
        d.page(page)
        for i, (name, fn) in enumerate(items):
            if only and name not in only:
                continue
            x = (i % 3) * (W + GAP)
            y = (i // 3) * (H + GAP)
            out.append((name, fn(d, name, x, y)))
    return out
