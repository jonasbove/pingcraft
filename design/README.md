# PingCraft UI design (first draft)

`pingcraft.figcraft` is a real FigCraft document, built with FigCraft 0.17.0 (`figcraft-cli`, headless, through its
MCP server). Every layer, component, style and variable in it was made through FigCraft's own engine commands;
nothing writes the `.figcraft` JSON directly. The PNGs in `exports/` were exported by FigCraft at 1x, with 2x
copies in `exports/2x/`.

- **Status:** a first draft for the maintainer's review. Nothing here is decided until it is approved.
- **Layout language:** the Craft launcher's, as CraftCloud's v2 design uses it: dark `#121316`, 1 px hairlines,
  square corners, Archivo wide headlines with one accent word, Geist Mono caps for HUD labels, buttons and badges,
  Inter for body text, inverted primary buttons, mono meta lines with middots.
- **Brand:** PingCraft's own accent, a "ping" yellow. There is no logo yet: `Brand/Mark` is a placeholder (a yellow
  square with a dot and one ring), and every header and sheet uses that one component, so swapping it is one change.
- **Words** follow `GLOSSARY.md`: network, account, chat, inbox, message, bridge, risk level. "Room" never appears.

## Pages and frames

| Page | Frames |
|---|---|
| Tokens & type | text styles, colour tokens (dark, light); `exports/_tokens.png` |
| Components | the library below; `exports/_components.png` |
| **M2 · Client** (1200×760 window) | `sign-in`, `recovery-key`, `inbox`, `inbox-light`, `inbox-networks`, `inbox-empty`, `chat-states`, `verify-device`, `unlock` |
| **M3 · Preview** (1200×760 window) | `connect-network`, `connect-risk` (both carry the "Preview · M3" tag) |

`inbox.png` is the main screen.

## What each screen shows

- **Window chrome** (every frame): the header (48 tall) has the brand cell (24 px mark and the wordmark "Ping" in
  ink_strong, "Craft" in accent_ink), the nav cells INBOX / ACCOUNTS / SETTINGS, a spacer, the status cell (8 px
  dot and a word: Connected, Not signed in, Signed in, Locked) and the window buttons. The footer (34 tall) has
  "PINGCRAFT 0.1.0 · @JOHN:EXAMPLE.ORG" in HUD on the left and a short state on the right.
- **sign-in:** a sheet over the dimmed window: SERVER ADDRESS (`example.org`) with the plain-words note that it is
  the user's own server and that messages are kept there and nowhere else, USERNAME (focused), PASSWORD, then
  "Forgot your password? Ask your server's admin." and SIGN IN.
- **recovery-key:** the first sign-in sheet. One sentence on what the key is for, the key in Mono 22 in blocks of 4
  (two rows of six, invented), COPY and SAVE TO FILE, an "If you lose it" panel that says losing every device and the
  key loses encrypted history for good (ADR 0004), the "I've saved my recovery key…" checkbox (shown ticked), and
  CONTINUE.
- **inbox** (dark) and **inbox-light:**
  - **Left column, 380 wide: the inbox.** The filter cells ALL 24 / UNREAD 2, search, the meta line "INBOX · 4
    ACCOUNTS · 24 CHATS", then the chats, 64 tall. Each row: a 36 px initials avatar; the chat name (truncated with
    an ellipsis); the network tag; the time (Mono 12, a fixed 48 px right-aligned slot so the tags line up); the last
    message (truncated); and the unread count. The open chat has been read, so it has no count. Unread rows have the name in Medium and the time and preview in ink;
    read rows are muted. The open chat is sunken with a 2 px accent_ink rule on the left.
  - **Network tag:** the network's name (TELEGRAM, SIGNAL, WHATSAPP, MATRIX) in Badge 10, with a 1 px outline and the
    name in the network's colour over a 12 % wash of it (tokens `net_telegram`, `net_signal`, `net_discord`,
    `net_whatsapp` and their `_wash`). Matrix stays the neutral line_strong outline. The shades are our own, chosen to
    evoke each network (Telegram a deep blue, Signal a bright blue, Discord a blue-violet, WhatsApp a green), not the
    networks' brand codes, and no logos. Text on the wash is 4.6:1 or better in both modes. Requested by the
    maintainer on 2026-11-24, after the first review.
  - **Network filter** (below the filter cells): one toggle per network, in the tag's colours when on and neutral
    with faint text when off. **inbox-networks** shows WhatsApp and Matrix switched off: their chats leave the list,
    the counts follow, and the meta line says "2 HIDDEN". Design only for now: PLAN.md lists it as a later decision.
  - **Right: the open chat.** The header has the avatar, chat name, network tag and the account ("personal · 6
    people"), the encryption indicator (a muted lock for a bridged chat) and a "…" button. The timeline is anchored to the bottom, with day dividers
    (hairline, HUD "TODAY · 24 NOV", hairline). Other people's messages have no box: a 28 px avatar, the sender in
    Body 13 Medium, the time in Mono 12, then the text. Your own messages sit on the right in a square box with the
    accent wash and a 2 px accent_ink rule on the right edge; the head says "You", the time and the state ("✓ Sent").
    The box is as wide as the text, from 260 to 460 px. The composer has a message field and SEND.
- **inbox-empty:** no accounts yet. The hero "Your chats, *one* inbox.", two paragraphs (networks are connected
  through bridges, a small program on your server; connecting from inside PingCraft comes in a later version),
  HOW TO CONNECT and CHECK AGAIN, and a meta line. On the right, the networks PingCraft can connect with how you log
  in and their risk level, and what a risk level is.
- **chat-states:** a direct Matrix chat with Lars Holm, showing every message state: a message that can't be
  decrypted yet (a sunken box: lock, "Waiting for the key to this message", why in plain words, and a "Why this
  happens" link), Sent (check), Sending… (clock), and Not sent (the rule and state in `bad`, "The server didn't
  answer.", DELETE and RETRY). Its header shows the Matrix-only indicator "End-to-end encrypted", with the lock in `good`.
- **verify-device:** a sheet over the dimmed inbox. One paragraph on what to compare, the 7 symbols in 88 × 104 tiles
  with their names (Umbrella, Anchor, Telephone, Aeroplane, Heart, Scissors, Pencil: all from the Matrix SAS emoji
  list), a note on what a mismatch means, and THEY DON'T MATCH / THEY MATCH.
- **unlock:** used on the web and on desktops without a keychain (ADR 0004). Why a passphrase is needed here,
  PASSPHRASE with SHOW, what to do if it is forgotten, SIGN OUT and UNLOCK.
- **connect-network** (Preview · M3): the hero "Connect a *network*.", then the four networks as cards that share
  hairlines: a strip with the number and the risk level badge, the name in Archivo 22, how you log in, the user's
  accounts on that network, and CONNECT. WhatsApp's CONNECT is shown as the primary (it is the one opened in
  `connect-risk`). These frames show the state before WhatsApp was connected (3 accounts: Telegram, Signal and
  Matrix), so they don't match the M2 inbox's 4 accounts on purpose. Discord is muted with LATER and "NOT YET": its bridge is not ready, and it has two risk levels
  (bot login low, your own account high; ADR 0008).
- **connect-risk** (Preview · M3): the risk screen before connecting WhatsApp. A risk scale (LOW / MEDIUM /
  MEDIUM-HIGH / HIGH with MEDIUM-HIGH on), then four short blocks in our own words from ADR 0007: what could happen,
  what makes it more likely, keeping the phone online (the link drops after about 14 days; a warning at 12), and
  where the messages are (the bridge on your server can read them). Then the "I understand… and I want to connect it
  anyway" checkbox, CANCEL and CONNECT WHATSAPP.

## Colours (`Theme` variables; dark / light)

| token | dark | light | note |
|---|---|---|---|
| bg | #121316 | #f2f1ee | as CraftCloud |
| sunken | #060607 | #e9e8e4 | |
| ink / ink_strong | #f2f1ee / #ffffff | #101014 / #000000 | |
| muted / faint | ink 60 % / 38 % | ink 62 % / 42 % | muted 6.4:1 / 5.1:1 on bg; no text is drawn in faint |
| line / line_strong | ink 15 % / 40 % | ink 16 % / 40 % | |
| **accent** | **#ffd23f** | **#ffd23f** | only as a fill under on_accent text (unread counts, the mark) and the sheet's 2 px rule |
| **accent_ink** | **#ffd23f** (12.9:1 on bg, 14.0:1 on sunken) | **#7a5c00** (5.5:1 on bg, 5.1:1 on sunken) | text, links, the headline accent word, selected-row and own-message rules, focus ring |
| accent_tint | accent at 9 % | accent at 14 % | sheet heroes and your own messages; ink on it is 13.7:1 (dark), 16.2:1 (light) |
| on_accent | #121316 | #121316 | 12.9:1 on accent |
| invert_bg / invert_fg | #ffffff / #0b0b0c | #101014 / #f2f1ee | primary buttons, selected cells |
| good / warn / bad | #3fd6b8 / #ffc14d / #ff6259 | #0a7563 / #8a5800 / #b3261e | as CraftCloud |
| on_bad, shadow, scrim, focus | | | as CraftCloud; focus = accent_ink |

Contrast is WCAG 2.x, computed from the hex values (alpha tokens composited on bg).

**Accent caveats** (worth a decision):

- **The yellow on the light background is 1.3:1**, so `accent` is never used for thin marks or text in light mode;
  those use accent_ink (#7a5c00). The sheet hero's 2 px accent rule is the one exception and would be faint in light mode (no light sheet is drawn
  yet).
- **The yellow is close to `warn`:** #ffd23f against #ffc14d is 1.1:1, and the light accent_ink #7a5c00 against light
  warn #8a5800 is 1.0:1. They must never carry different meanings side by side. Here, risk levels and message states
  are always said in words (MEDIUM, Sending…, Not sent), never by colour alone. The M3 frames do show the accent
  Preview tag and a MEDIUM badge (warn) on one screen, told apart only by their words. If PingCraft keeps the yellow, the theme may want a more orange `warn`.

## Components (page "Components")

Adapted from CraftCloud: Icon/* (16 simple 14 px stroke icons; nine of the paths are CraftCloud's, seven are new), Button (Primary / Secondary /
Danger × Default / Hover / Focus / Disabled), Icon button, Button/More, Status badge (Good / Warn / Bad / Accent /
Muted), Cell (Off / On / Hover, with count), Nav cell, Input (Default / Focus), Search field, Header/Desktop,
Footer/Desktop. New for PingCraft: Brand/Mark, Brand/Wordmark, Brand/Lockup, Network tag, Unread count, Preview tag,
Checkbox (Off / On), Avatar/36, Avatar/28, Chat row (Read / Unread / Selected), Message/Theirs, Message/Mine, Day
divider and Symbol tile.

| Component | Spec |
|---|---|
| Chat row | 380 × 64, padding 0 16, gap 12, hairline under. Avatar 36. Line 1: name (Body 14, Medium when unread; fills, one line), network tag, time (Mono 12, 48 px slot, right-aligned). Line 2, 3 px under: preview (Small 12.5, fills, one line), unread count. Selected: sunken fill, 2 px accent_ink rule on the left |
| Network tag | 18 tall, padding 0 5, 1 px line_strong, Badge 10 muted, the network's name; on screens tinted per network (see above) |
| Unread count | 18 tall, padding 0 6, accent fill, Badge 10 on_accent |
| Message/Theirs | avatar 28, gap 12; head (sender Body 13 Medium ink_strong, time Mono 12 muted, gap 8), 2 under it the text (Body 14) |
| Message/Mine | right-aligned, 260 to 460 wide (follows the text), accent_tint fill, padding 10 14, 2 px accent_ink rule on the right edge; head "You", time, then the state on the right (14 px icon and Mono 12: ✓ Sent, ◷ Sending…, ⚠ Not sent in bad) |
| Day divider | 20 tall: hairline, HUD muted label, hairline, gap 12 |
| Timeline | padding 20 24, gap 14, anchored to the bottom |
| Chat header | 64 tall, padding 0 24: avatar 36, name Body 16 Medium, then tag + account (Small 12.5 muted); the encryption indicator (30 tall hairline box, lock icon, Small 12.5); "…" |
| Composer | 74 tall, hairline on top, padding 20 24: the message field (fills) and SEND |
| Sheet | as CraftCloud: bg, 1 px line_strong, dialog shadow; hero with accent_tint, 2 px accent rule, the 44 px mark, title Archivo 26, caption HUD; body padding 24, gap 16; actions row padding 14 24 with secondary left, primary right. Sheets hug their content and are centred in the window |
| Symbol tile | 88 × 104, sunken, hairline, the symbol at 34 px, its name Small 12 muted |
| Preview tag | 20 tall, 1 px accent_ink border, Badge 10 accent_ink, "Preview · M3" |

## Sample data

All invented: the demo user John Doe (`@john:example.org`, server `example.org`), Ana Ortiz, Lars Holm, Mei Tanaka,
Sam Weber, Noor Haddad, Omar and Kofi, and the chats Book club, Family, Climbing crew and Garden plot. The recovery
key is random letters, not a real key. Dates are around Tuesday 24 November 2026.

## How to rebuild

The fonts are not in this repository (house rule: fonts live in `storytold/craft-fonts`). Put
`Inter-Variable.ttf`, `Archivo-Variable.ttf` and `GeistMono-Variable.ttf` from craft-fonts (all SIL OFL 1.1) in one
directory and pass it with `--fonts`; the build stops if one is missing.

```sh
nice -n 10 python3 -I -B design/tools/build.py --cli /path/to/figcraft-cli --fonts /path/to/craft-fonts
```

Every run builds and saves the whole document. `--only inbox,chat-states` exports just those frames; `--no-2x`
skips the 2x copies. A full run takes a few seconds.

- `tools/fc.py`: the MCP client for `figcraft-cli mcp`.
- `tools/lib.py`: tokens (`THEME`), text styles, the `Doc` helpers and layout helpers.
- `tools/components.py`: the component library.
- `tools/screens.py`: the frames and the sample data.
- `tools/build.py`: builds, saves and exports.

`fc.py`, `lib.py` and parts of `components.py` and `screens.py` are adapted from CraftCloud's `design/tools/` (same
owner, MIT OR Apache-2.0), with CraftCloud's app list, logo and server facts removed.

## Notes and deviations

- **Encryption wording.** A bridged chat is not end-to-end encrypted from the other person to the user: the bridge on
  the user's server decrypts it and re-encrypts it for Matrix (ADR 0004). So bridged chats say "Encrypted · via your
  bridge" with a muted lock, and only Matrix-to-Matrix chats say "End-to-end encrypted" with the lock in `good`.
  The exact words need the maintainer's eye.
- **Verification symbols are monochrome placeholders.** The headless build has only FigCraft's built-in fonts and no
  colour emoji font, so the seven symbols are the monochrome text forms of the same code points (U+2602, U+2693,
  U+260E, U+2708, U+2665, U+2702, U+270F). The app draws the emoji from the SAS list with their names.
- **Your own messages** are a square box on the right with a wash and an edge rule, in the launcher's language, not a
  rounded bubble.
- **Network names appear only as the names of networks.** No network logos, icons or colours are used; the tag is
  text in the same neutral style for every network.
- **The headline accent word is a literal per mode** (#ffd23f / #7a5c00), because FigCraft can't bind a variable to
  part of a text; those headlines don't follow a theme switch.
- **The failed message is a detached instance**: FigCraft doesn't allow adding layers inside an instance.
- **The overview PNGs** get a page-coloured backdrop at export time (a multi-layer export is transparent); the backdrop
  is not saved in the file.
- **`figcraft-cli info`** lists Archivo and Geist Mono text layers as "missing font": a `.figcraft` file doesn't carry
  its fonts. Load them (`text.loadFont`, or install them) before opening it.

## Credits

- **The look** is based on ArtCraft Launcher by the ArtCraft team, by way of CraftCloud's v2 design. No ArtCraft
  logo, mark or name is used in PingCraft's branding.
- **Fonts:** Inter, Archivo and Geist Mono (SIL OFL 1.1), from `storytold/craft-fonts`; not committed here.
- **Tools:** adapted from CraftCloud's `design/tools/` (MIT OR Apache-2.0).

## FigCraft 0.17.0 notes

CraftCloud's design was built with 0.18.1. The ported helpers needed no changes for 0.17.0: every command they use
exists under the same name, and
`style.create {from}`, `style.bind` with a plain variable name, per-side stroke weights, boolean component
properties, `text.style {textFills}` and `component.combineVariants` all worked. The new code ran into the
following (not checked against 0.18.1, so they may not be differences):

1. `crossAlign` takes `min|center|max|baseline`; there is no `stretch`. A child fills the cross axis with
   `sizingV: fill` instead.
2. `constraints.horizontal` takes `left|right|leftRight|center|scale` (not `min|max|stretch`).
3. `sizingH: fill` on a layer whose parent has no auto layout is an error ("Fill needs an auto layout parent").
4. Layers can't be added inside an instance; `component.detach` first.
5. `text.maxLines {lines: 1}` on fixed-width text truncates with an ellipsis, which the chat rows use.
6. `node.find {name}` matches substrings and nested instances share names (`Brand/Mark` has a "Dot" layer, so the
   header's status dot is called "Status dot"); `Doc.find` filters exact names.
