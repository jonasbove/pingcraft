"""The "Messages and chats" frames (page "Messages · Draft"): what you can do with a message and with a chat.

A draft for the maintainer's review (PLAN.md, "Messages and chats"): every frame carries the "Draft · for review"
tag. Built in the same language and with the same helpers as screens.py. The emoji are monochrome placeholders:
the headless build has no emoji font (as for the verification symbols), so reactions are drawn with text forms
where one exists. Sample people, chats and messages are invented, as in screens.py.
"""

from lib import ACCENT_INK, box_stroke, hstack, hud, side_stroke, spacer, vstack
from screens import (H, LEFT_W, W, btn, centre, chat_pane, divider, ibtn, icon, inbox_column, mine, note, panel, para,
                     scrim, set_text, sheet, sheet_actions, screen, theirs, tint_tag)

DRAFT = "Draft · for review"
# The quick reactions: a small fixed set that egui's built-in emoji font draws (PLAN.md: no emoji font yet).
# Drawn here as text placeholders; the app shows the emoji.
QUICK = ("+1", "♥", "ha", "oh", "sad", "thx")
CHAT = ("Book club", "BC", "Signal", "personal · 6 people", (False, "Encrypted · via your bridge"))


def draft_tag(d, parent, x=None, y=None):
    t = d.inst("Preview tag", parent, props={"Label": DRAFT}, name="Draft tag")
    if x is not None:
        d.set(t, x=x, y=y)
    return t


def frame_tag(d, f):
    """The draft tag in the middle of the footer, where it covers nothing."""
    return draft_tag(d, f, 520, H - 34 + 7)


def insert(d, parent, nid, index):
    """Move `nid` (already a child of `parent`) to `index` in its auto layout flow."""
    d.R("node.reorder", {"id": nid, "index": index})
    return nid


def where(d, nid, root):
    """`nid`'s position relative to `root`."""
    a = d.get(nid, "absoluteBounds")["absoluteBounds"]
    b = d.get(root, "absoluteBounds")["absoluteBounds"]
    return round(a["x"] - b["x"]), round(a["y"] - b["y"])


def detached_theirs(d, tl, sender, ini, time, body):
    """Someone's message, detached so layers can be added to its text column."""
    m = theirs(d, tl, sender, ini, time, body)
    d.R("component.detach", {"ids": [m]})
    return m, d.sub(m, "Text")


def reactions(d, col, chips, add=True):
    r = hstack(d, col, "Reactions", gap=6)
    for emoji, count, on in chips:
        d.inst("Reaction chip", r, props={"Emoji": emoji, "Count": str(count), "State": "On" if on else "Off"},
               name=f"Reaction {emoji}")
    if add:
        a = hstack(d, r, "Add reaction", gap=0, h=24, padding=[0, 6, 0, 6])
        box_stroke(d, a, "line")
        icon(d, a, "smile", "muted")
    return r


def quote(d, col, sender, text, index):
    q = d.inst("Reply quote", col, props={"Sender": sender, "Text": text}, name="Reply quote")
    d.set(q, sizingH="fill")
    return insert(d, col, q, index)


def menu(d, f, x, y, items, w=240, title=None):
    """A context menu: items are (icon, label, shortcut, state) or None for a hairline."""
    m = d.frame(f, "Menu", x, y, w, 10, fill="bg", stroke="line_strong", layout="vertical", gap=0,
                padding=[4, 0, 4, 0], sizingV="hug", clip=False)
    d.effect_style(m, "Shadow/Dialog")
    if title:
        h = hstack(d, m, "Menu title", gap=0, h=28, padding=[0, 12, 0, 12])
        d.set(h, sizingH="fill")
        hud(d, h, title, name="Menu title text")
    for it in items:
        if it is None:
            r = d.rect(m, 0, 0, w, 1, "line", name="Menu rule")
            d.set(r, sizingH="fill")
            continue
        ic, label, short, state = it
        mi = d.inst("Menu item", m, props={"Label": label, "Shortcut": short, "State": state}, name=f"Item {label}")
        d.set(mi, sizingH="fill")
        d.R("instance.swap", {"ids": [d.sub(mi, "Icon")], "component": d.C[f"Icon/{ic}"]})
        d.set(d.sub(d.sub(mi, "Icon"), "glyph"), stroke="bad" if state == "Danger" else "muted")
    return m


def pinned_bar(d, pane, w):
    """The pinned messages bar, between the chat header and the timeline."""
    pb = d.frame(pane, "Pinned bar", 0, 0, w, 44, fill="sunken", layout="horizontal", gap=10,
                 padding=[0, 24, 0, 24], crossAlign="center", sizingH="fill", sizingV="fixed", clip=False)
    side_stroke(d, pb, bottom=1)
    rule = d.rect(pb, 0, 0, 2, 44, "accent_ink", name="Pinned rule")
    d.absolute(rule, 0, 0, 2, 44)
    icon(d, pb, "pin", "accent_ink")
    hud(d, pb, "Pinned 1 of 2", color="accent_ink", name="Pinned count")
    t = d.text(pb, "Lars Holm: Next meeting on Thursday at mine? I'll make soup.", "Small/12.5", "ink",
               name="Pinned text", w=500, lines=1)
    d.set(t, sizingH="fill")
    ibtn(d, pb, "All pinned", "chevron-down", "Secondary", name="All pinned")
    return insert(d, pane, pb, 1)


def chat_frame(d, name, x, y, selected="Book club", hidden=()):
    f = screen(d, name, x, y)
    inbox_column(d, f, selected, hidden=hidden)
    tl = chat_pane(d, f, *CHAT)
    pane = d.sub(f, "Chat")
    return f, pane, tl


def composer_bar(d, pane, w, kind, title, text, index=2):
    """A bar over the composer: replying to a message, or editing one of yours."""
    b = d.frame(pane, f"{kind} bar", 0, 0, w, 10, fill="sunken", layout="horizontal", gap=10,
                padding=[10, 24, 10, 24], crossAlign="center", sizingH="fill", sizingV="hug", clip=False)
    side_stroke(d, b, top=1)
    icon(d, b, "reply" if kind == "Reply" else "edit", "accent_ink")
    col = vstack(d, b, "Text", gap=2)
    d.set(col, sizingH="fill")
    hud(d, col, title, color="accent_ink", name="Bar title")
    t = d.text(col, text, "Small/12.5", "muted", name="Bar text", w=600, lines=1)
    d.set(t, sizingH="fill")
    x = hstack(d, b, f"Cancel {kind.lower()}", gap=0, h=34, padding=[0, 10, 0, 10])
    box_stroke(d, x, "line_strong")
    icon(d, x, "x", "ink")
    return insert(d, pane, b, index)


# --- frames ---------------------------------------------------------------------------------------------

def chat_actions(d, name, x, y):
    """A message's actions: the bar on hover, the menu, reactions, a reply, edited and deleted messages, and the
    pinned bar."""
    f, pane, tl = chat_frame(d, name, x, y)
    pinned_bar(d, pane, W - LEFT_W)
    divider(d, tl, "Today · 24 Nov")
    m, col = detached_theirs(d, tl, "Ana Ortiz", "AO", "09:02",
                             "Has everyone got the book? The library on Elm Street had two copies left.")
    reactions(d, col, (("+1", 3, False), ("♥", 1, True)), add=False)
    del m
    mi = mine(d, tl, "09:10", "Got mine yesterday.")
    set_text(d, mi, "Time", "09:10 · edited")
    dl = hstack(d, tl, "Deleted", gap=12)
    d.set(dl, sizingH="fill", padding=[0, 0, 0, 40])
    icon(d, dl, "trash", "muted")
    d.text(dl, "Message deleted", "Small/12.5", "muted", name="Deleted text")
    hud(d, dl, "Lars Holm · 10:14", name="Deleted meta")
    m, col = detached_theirs(d, tl, "Lars Holm", "LH", "14:28", "Same here. Chapter 9 so far. No spoilers, please!")
    quote(d, col, "You", "Got mine yesterday.", 1)
    reactions(d, col, (("ha", 2, False),))
    # The hovered message: its row is sunken, with the quick reactions, REPLY and "…" on its right.
    d.set(m, fill="sunken")
    mx, my = where(d, m, f)
    mw = d.get(m, "width")["width"]
    bar = hstack(d, f, "Message actions", gap=0, h=32)
    d.set(bar, fill="bg")
    box_stroke(d, bar, "line_strong")
    d.effect_style(bar, "Shadow/Dialog")
    for e in QUICK:
        c = hstack(d, bar, f"React {e}", gap=0, h=32, padding=[0, 8, 0, 8])
        d.text(c, e, "Body/13", "ink_strong", name="Emoji")
    sep = d.rect(bar, 0, 0, 1, 32, "line", name="sep")
    del sep
    for ic in ("smile", "reply", "more"):
        c = hstack(d, bar, f"Action {ic}", gap=0, h=32, padding=[0, 9, 0, 9])
        if ic == "more":
            d.set(c, fill="sunken")
        icon(d, c, ic, "ink")
    bw = d.get(bar, "width")["width"]
    d.set(bar, x=mx + mw - bw - 8, y=my - 18)
    mn = menu(d, f, mx + mw - 248, my + 18, [
        ("reply", "Reply", "R", "Hover"), ("smile", "React", "E", "Default"),
        ("copy", "Copy text", "Ctrl+C", "Default"), ("forward", "Forward", "F", "Default"),
        ("pin", "Pin in this chat", "P", "Default"), ("select", "Select", "S", "Default"),
        ("info", "Details", "I", "Default"),
    ])
    # Opened from "…": to the left of the bar, its bottom on the bar's.
    bx = d.get(bar, "x")["x"]
    d.set(mn, x=bx - 248, y=my + 14 - d.get(mn, "height")["height"])
    frame_tag(d, f)
    return f


def own_menu(d, name, x, y):
    """Your own message's menu adds EDIT and DELETE; DELETE asks first."""
    f, pane, tl = chat_frame(d, name, x, y)
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!")
    mi = mine(d, tl, "14:31", "I won't say a word. See you Thursday.")
    d.set(mi, fill="sunken")
    mx, my = where(d, mi, f)
    mn = menu(d, f, mx - 250, my - 40, [
        ("reply", "Reply", "R", "Default"), ("smile", "React", "E", "Default"),
        ("copy", "Copy text", "Ctrl+C", "Default"), ("forward", "Forward", "F", "Default"),
        ("pin", "Pin in this chat", "P", "Default"), ("select", "Select", "S", "Default"),
        ("info", "Details", "I", "Default"), None,
        ("edit", "Edit", "Up", "Hover"), ("trash", "Delete", "Del", "Danger"),
    ])
    d.set(mn, y=my + d.get(mi, "height")["height"] - d.get(mn, "height")["height"])
    frame_tag(d, f)
    return f


def delete_message(d, name, x, y):
    f = own_menu(d, name, x, y)
    scrim(d, f)
    w = 560
    s, b = sheet(d, f, w, "Delete this message?", "Book club · Signal", right=lambda hd: draft_tag(d, hd))
    q = d.inst("Reply quote", b, props={"Sender": "You · 14:31", "Text": "I won't say a word. See you Thursday."},
               name="Message")
    d.set(q, sizingH="fill")
    para(d, b, "It is deleted for everyone in this chat, and a “Message deleted” line stays in its place.", "ink")
    note(d, b, "PingCraft asks the Signal bridge to delete it on Signal too. Whether it disappears there depends on "
               "Signal and on how long ago it was sent.")
    sheet_actions(d, s, left=lambda r: btn(d, r, "Cancel"),
                  right=lambda r: ibtn(d, r, "Delete", "trash", "Danger", name="Delete"))
    centre(d, s, w)
    return f


def chat_reply(d, name, x, y):
    """Replying: the bar over the composer. Also: someone typing, JUMP TO LATEST, and a draft in another chat."""
    f = screen(d, name, x, y)
    col = inbox_column(d, f, "Book club")
    row = d.sub(col, "Chat Ana Ortiz")
    d.R("component.detach", {"ids": [row]})
    pv = d.sub(row, "Preview")
    text = "Draft: I'll bring the second copy"
    d.set(pv, characters=text, fill="muted")
    d.R("text.style", {"id": pv, "start": 0, "end": 5, "textFills": [ACCENT_INK[False]]})
    tl = chat_pane(d, f, *CHAT)
    pane = d.sub(f, "Chat")
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Ana Ortiz", "AO", "09:02", "Has everyone got the book? The library on Elm Street had two copies left.")
    theirs(d, tl, "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!")
    ty = hstack(d, tl, "Typing", gap=8)
    d.set(ty, padding=[0, 0, 0, 40])
    d.text(ty, "Lars Holm is typing…", "Small/12.5", "muted", name="Typing text")
    composer_bar(d, pane, W - LEFT_W, "Reply", "Replying to Ana Ortiz",
                 "Has everyone got the book? The library on Elm Street had two copies left.")
    inp = d.sub(d.sub(pane, "Composer"), "Message input")
    d.R("instance.setProperties", {"ids": [inp], "properties": {"Value": "Yes, got it on Monday", "State": "Focus"}})
    d.set(d.sub(inp, "Value"), fill="ink")
    # JUMP TO LATEST, over the bottom right of the timeline when it is scrolled up.
    tx, ty0 = where(d, tl, f)
    th = d.get(tl, "height")["height"]
    j = ibtn(d, f, "Jump to latest", "arrow-down", "Secondary", name="Jump to latest")
    jw = d.get(j, "width")["width"]
    d.set(j, x=tx + (W - LEFT_W) - 24 - jw, y=ty0 + th - 34 - 12)
    d.effect_style(j, "Shadow/Dialog")
    frame_tag(d, f)
    return f


def chat_edit(d, name, x, y):
    """Editing one of your messages: the message is marked, the composer holds its text."""
    f, pane, tl = chat_frame(d, name, x, y)
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!")
    mi = mine(d, tl, "14:31", "I won't say a word. See you Thursday.")
    d.set(mi, fill="sunken")
    set_text(d, mi, "State label", "Editing")
    composer_bar(d, pane, W - LEFT_W, "Edit", "Editing your message",
                 "Edited messages say “edited”. Whether the edit reaches Signal depends on its bridge.")
    inp = d.sub(d.sub(pane, "Composer"), "Message input")
    d.R("instance.setProperties", {"ids": [inp], "properties": {"Value": "I won't say a word. See you on Thursday!",
                                                                "State": "Focus"}})
    d.set(d.sub(inp, "Value"), fill="ink")
    snd = d.sub(d.sub(pane, "Composer"), "Send")
    d.R("instance.setProperties", {"ids": [snd], "properties": {"Label": "Save"}})
    frame_tag(d, f)
    return f


def chat_select(d, name, x, y):
    """Selecting messages: a box on each, the selected ones sunken, and a bar instead of the composer."""
    f, pane, tl = chat_frame(d, name, x, y)
    divider(d, tl, "Today · 24 Nov")
    rows = (("theirs", "Ana Ortiz", "AO", "09:02", "Has everyone got the book?", False),
            ("theirs", "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!", True),
            ("mine", None, None, "14:31", "I won't say a word. See you Thursday.", True),
            ("theirs", "Lars Holm", "LH", "14:32", "The ending of chapter 8, though.", False))
    for kind, sender, ini, time, body, on in rows:
        r = hstack(d, tl, f"Select {time}", gap=12, crossAlign="min")
        d.set(r, sizingH="fill", padding=[6, 8, 6, 8])
        if on:
            d.set(r, fill="sunken")
        cb = d.inst("Checkbox", r, props={"State": "On" if on else "Off"}, name="Checkbox")
        del cb
        if kind == "theirs":
            m = d.inst("Message/Theirs", r, props={"Sender": sender, "Time": time, "Body": body}, name="Message")
            d.set(m, sizingH="fill")
            set_text(d, m, "Initials", ini)
        else:
            spacer(d, r)
            m = d.inst("Message/Mine", r, props={"Time": time, "State": "Sent", "Body": body}, name="Message")
            d.set(m, width=320)
    comp = d.sub(pane, "Composer")
    d.set(comp, visible=False)
    bar = d.frame(pane, "Selection bar", 0, 0, W - LEFT_W, 74, fill=None, layout="horizontal", gap=8,
                  padding=[20, 24, 20, 24], crossAlign="center", sizingH="fill", sizingV="fixed", clip=False)
    side_stroke(d, bar, top=1)
    hud(d, bar, "2 selected", color="ink", name="Selected count")
    note(d, bar, "Only your own messages can be deleted.", name="Delete note")
    ibtn(d, bar, "Copy", "copy", "Secondary", name="Copy")
    ibtn(d, bar, "Forward", "forward", "Secondary", name="Forward")
    ibtn(d, bar, "Delete", "trash", "Secondary", name="Delete", disabled=True)
    btn(d, bar, "Cancel")
    frame_tag(d, f)
    return f


def message_details(d, name, x, y):
    f, pane, tl = chat_frame(d, name, x, y)
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!")
    scrim(d, f)
    w = 620
    s, b = sheet(d, f, w, "Message details", "Lars Holm · Book club", right=lambda hd: draft_tag(d, hd))
    q = d.inst("Reply quote", b, props={"Sender": "Lars Holm", "Text": "Halfway through chapter 9. No spoilers, please!"},
               name="Message")
    d.set(q, sizingH="fill")
    grid = vstack(d, b, "Facts", gap=0)
    d.set(grid, sizingH="fill")
    box_stroke(d, grid, "line")
    facts = (("Sent", "Tuesday 24 November 2026 · 14:28:03"),
             ("Network", "Signal, through your bridge · account personal"),
             ("Encryption", "Encrypted between your server and you · the bridge reads it to pass it on"),
             ("Read by", "Ana Ortiz 14:30 · You 14:31"),
             ("Edited", "Yes, once · 14:29"))
    for i, (k, v) in enumerate(facts):
        r = hstack(d, grid, f"Fact {k}", gap=12, h=36, padding=[0, 14, 0, 14])
        d.set(r, sizingH="fill")
        if i:
            side_stroke(d, r, top=1)
        lab = hud(d, r, k, name="Fact label")
        d.set(lab, width=96, textAutoResize="height")
        d.text(r, v, "Small/12.5", "ink", name="Fact value")
    p = panel(d, b, "Edit history", gap=6)
    for time, text, cur in (("14:28", "Halfway through chapter 8. No spoilers, please!", False),
                            ("14:29", "Halfway through chapter 9. No spoilers, please!", True)):
        r = hstack(d, p, f"Version {time}", gap=12, crossAlign="min")
        d.set(r, sizingH="fill")
        d.text(r, time + (" · now" if cur else ""), "Mono/12", "muted", name="Version time")
        note(d, r, text, "ink" if cur else "muted", "Small/12.5", name="Version text")
    note(d, b, "Read receipts and edits show only where Signal and its bridge carry them.")
    sheet_actions(d, s, left=lambda r: ibtn(d, r, "Copy text", "copy", "Secondary", name="Copy text"),
                  right=lambda r: btn(d, r, "Close", "Primary"))
    centre(d, s, w)
    return f


def forward(d, name, x, y):
    f, pane, tl = chat_frame(d, name, x, y)
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!")
    scrim(d, f)
    w = 600
    s, b = sheet(d, f, w, "Forward 2 messages", "To one of your chats", right=lambda hd: draft_tag(d, hd))
    sf = d.inst("Search field", b, props={"Placeholder": "Search chats"}, name="Search")
    d.set(sf, sizingH="fill")
    lst = vstack(d, b, "Chats", gap=0)
    d.set(lst, sizingH="fill")
    box_stroke(d, lst, "line")
    for i, (chat, net, on) in enumerate((("Ana Ortiz", "Telegram", True), ("Family", "WhatsApp", False),
                                         ("Lars Holm", "Matrix", False), ("Mei Tanaka", "Telegram", False))):
        r = hstack(d, lst, f"Chat {chat}", gap=12, h=44, padding=[0, 14, 0, 14])
        d.set(r, sizingH="fill")
        if on:
            d.set(r, fill="sunken")
        if i:
            side_stroke(d, r, top=1)
        d.inst("Checkbox", r, props={"State": "On" if on else "Off"}, name="Checkbox")
        av = d.inst("Avatar/28", r, name="Avatar")
        set_text(d, av, "Initials", "".join(w[0] for w in chat.split()[:2]).upper())
        t = d.text(r, chat, "Body/14", "ink", name="Name")
        d.set(t, sizingH="fill")
        tag = d.inst("Network tag", r, props={"Network": net}, name="Tag")
        tint_tag(d, tag, net)
    note(d, b, "They are sent as new messages from you, on that chat's network. They don't say “forwarded”: the "
               "bridges don't carry that yet.")
    sheet_actions(d, s, left=lambda r: btn(d, r, "Cancel"),
                  right=lambda r: ibtn(d, r, "Forward", "forward", name="Forward"))
    centre(d, s, w)
    return f


def chat_menu(d, name, x, y):
    """The chat's "…" menu: pin to the top of the inbox, mark as unread, mute for a while. The inbox shows a pinned
    chat first with a pin, a muted one with a struck bell and a quiet count, and a chat marked unread with a dot."""
    f = screen(d, name, x, y)
    col = inbox_column(d, f, "Book club")
    lst = d.sub(col, "Chats")
    # Book club and Lars Holm are pinned: first, with a pin before the tag.
    for chat, index in (("Book club", 0), ("Lars Holm", 1)):
        r = d.sub(lst, f"Chat {chat}")
        insert(d, lst, r, index)
        d.R("component.detach", {"ids": [r]})
        l1 = d.sub(r, "Line 1")
        p = icon(d, l1, "pin", "muted", label="Pinned")
        insert(d, l1, p, 1)
    # Family is muted: a struck bell, and its count is quiet.
    r = d.sub(lst, "Chat Family")
    d.R("component.detach", {"ids": [r]})
    l1 = d.sub(r, "Line 1")
    insert(d, l1, icon(d, l1, "bell-off", "muted", label="Muted"), 1)
    u = d.sub(r, "Unread")
    d.set(u, fill="sunken")
    box_stroke(d, u, "line_strong")
    d.set(d.sub(u, "Count"), fill="muted")
    # Mei Tanaka is marked unread: a dot where the count goes.
    r = d.sub(lst, "Chat Mei Tanaka")
    d.R("component.detach", {"ids": [r]})
    u = d.sub(r, "Unread")
    d.set(u, visible=True)
    d.set(d.sub(u, "Count"), characters=" ")
    for t in ("Name",):
        d.set(d.sub(r, t), fill="ink_strong")
    tl = chat_pane(d, f, *CHAT)
    divider(d, tl, "Today · 24 Nov")
    theirs(d, tl, "Lars Holm", "LH", "14:28", "Halfway through chapter 9. No spoilers, please!")
    pane = d.sub(f, "Chat")
    more = d.sub(pane, "More")
    d.set(more, fill="sunken")
    mx, my = where(d, more, f)
    m = menu(d, f, mx + 34 - 260, my + 40, [
        ("pin", "Unpin from the top", "", "Default"), ("unread", "Mark as unread", "U", "Default"), None,
    ], w=260)
    mute = menu(d, f, 0, 0, [
        ("bell-off", "For an hour", "", "Default"), ("bell-off", "For a day", "", "Hover"),
        ("bell-off", "For a week", "", "Default"), ("bell-off", "Until you turn it back on", "", "Default"),
    ], w=260, title="Mute notifications")
    d.R("node.reparent", {"ids": [mute], "parent": m})
    d.set(mute, effects=[], strokes=[], padding=[0, 0, 0, 0])
    d.set(mute, sizingH="fill")
    n = d.frame(m, "Mute note", 0, 0, 260, 10, fill=None, layout="vertical", padding=[6, 12, 8, 12],
                sizingH="fill", sizingV="hug", clip=False)
    note(d, n, "A muted chat still counts as unread. Muting is kept on your server, so your other Matrix apps "
               "follow it; the time it ends is kept by PingCraft.", name="Mute note text")
    frame_tag(d, f)
    return f


FRAMES = [("chat-actions", chat_actions), ("chat-own-menu", own_menu), ("delete-message", delete_message),
          ("chat-reply", chat_reply), ("chat-edit", chat_edit), ("chat-select", chat_select),
          ("message-details", message_details), ("forward", forward), ("chat-menu", chat_menu)]

__all__ = ["FRAMES", "DRAFT"]
