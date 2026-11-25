//! The screens: the window's header and footer, the sign-in and unlock sheets, the inbox with the open chat, and
//! the encryption sheets (recovery key, entering it, verifying a device). Each turns what the user does into
//! [`Command`]s; nothing here talks to a server.

use egui::{Align, Color32, Layout, Margin, RichText, ScrollArea, Sense, Stroke, Ui, Vec2};
use pingcraft_core::app::{Command, SignInProblem};
use pingcraft_core::encryption::{CancelReason, RecoveryKey, RecoveryStatus, SasSymbol, Verification};
use pingcraft_core::{Chat, ConnectionState, Filter, MessageBody};
use zeroize::Zeroizing;

use crate::i18n::{fmt, trn};
use crate::state::{NetworkEntry, Screen, State, Tab};
use crate::theme::Tokens;
use crate::time::short_time;
use crate::tl;
use crate::widgets::{Kind, avatar, button, field, hud, mark, network_name, network_tag, note, problem, unread};

/// The commands the screens collected in one frame.
pub type Out = Vec<Command>;

pub fn header(ui: &mut Ui, state: &mut State, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal_centered(|ui| {
        ui.add_space(16.0);
        mark(ui, 24.0);
        ui.label(RichText::new("Ping").size(18.0).strong().color(t.ink_strong));
        ui.add_space(-ui.spacing().item_spacing.x);
        ui.label(RichText::new("Craft").size(18.0).strong().color(t.accent_ink));
        if state.screen == Screen::Inbox {
            ui.add_space(24.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            if nav_cell(ui, tl!("Inbox"), state.tab == Tab::Inbox, true).clicked() {
                state.tab = Tab::Inbox;
            }
            if nav_cell(ui, tl!("Accounts"), state.tab == Tab::Accounts, true).clicked() && state.tab != Tab::Accounts {
                open_accounts(state, out);
            }
            // Settings has no screen yet.
            nav_cell(ui, tl!("Settings"), false, false);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(16.0);
            let (word, dot) = status(state, &t);
            ui.label(RichText::new(word).size(13.0).color(t.ink));
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, dot);
            // On every tab: reconnect now, and ask the bridges again.
            if state.screen == Screen::Inbox {
                ui.add_space(12.0);
                if button(ui, tl!("Refresh"), Kind::Secondary, true).clicked() {
                    out.push(Command::Refresh);
                }
            }
        });
    });
}

/// One of the header's tabs: inverted when it is the one shown. Its accessible name is the label; it says whether
/// it is selected.
fn nav_cell(ui: &mut Ui, label: &str, on: bool, enabled: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let ink = if on { t.invert_fg } else { t.muted };
    let cell = egui::Button::new(RichText::new(label.to_uppercase()).monospace().size(11.0).color(ink))
        .stroke(Stroke::NONE)
        .corner_radius(egui::CornerRadius::ZERO)
        .selected(on);
    let (fill, hover) = if on { (t.invert_bg, t.invert_bg) } else { (Color32::TRANSPARENT, t.line) };
    crate::widgets::animated(ui, cell, enabled, fill, hover)
}

/// Shows the ACCOUNTS tab, and asks the bridges what they offer and which accounts the user has.
pub fn open_accounts(state: &mut State, out: &mut Out) {
    state.tab = Tab::Accounts;
    for id in crate::accounts::NETWORKS {
        state.accounts.networks.entry(pingcraft_core::Network::from_id(id)).or_insert(NetworkEntry::Loading);
    }
    out.push(Command::LoadNetworks);
}

/// The word and the dot colour for the header's status cell.
fn status(state: &State, t: &Tokens) -> (String, Color32) {
    match (&state.screen, &state.connection) {
        (Screen::Locked { .. }, _) => (tl!("Locked").to_owned(), t.muted),
        (Screen::AlreadyRunning, _) => (tl!("Already open").to_owned(), t.muted),
        (Screen::SignIn | Screen::Starting, _) => (tl!("Not signed in").to_owned(), t.muted),
        (Screen::Inbox, ConnectionState::Online) => (tl!("Connected").to_owned(), t.good),
        (Screen::Inbox, ConnectionState::Connecting) => (tl!("Connecting…").to_owned(), t.warn),
        (Screen::Inbox, ConnectionState::Offline) => (tl!("Offline").to_owned(), t.bad),
        (Screen::Inbox, ConnectionState::Stopped) => (tl!("Stopped").to_owned(), t.muted),
        (Screen::Inbox, ConnectionState::Error(_)) => (tl!("Not connected").to_owned(), t.bad),
    }
}

pub fn footer(ui: &mut Ui, state: &State) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal_centered(|ui| {
        ui.add_space(20.0);
        let who = state.user.clone().unwrap_or_else(|| tl!("Not signed in").to_uppercase());
        // Matrix IDs are shown as typed, never in capitals.
        ui.label(RichText::new(format!("PINGCRAFT {} · {who}", env!("CARGO_PKG_VERSION"))).monospace().size(11.0).color(t.muted));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(20.0);
            let right = match state.screen {
                Screen::Inbox if state.tab == Tab::Accounts => trn(state.accounts.count() as u64, "{n} account", "{n} accounts"),
                Screen::Inbox => trn(state.inbox.len() as u64, "{n} chat", "{n} chats"),
                Screen::Locked { .. } => tl!("Locked").to_owned(),
                _ => tl!("Your server, your messages").to_owned(),
            };
            ui.label(RichText::new(right).size(12.0).color(t.muted));
        });
    });
}

// -----------------------------------------------------------------------------------------------------------------
// Signing in and unlocking

pub fn sign_in(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let form = &mut state.sign_in;
    let ready = !form.server.trim().is_empty() && !form.username.trim().is_empty() && !form.password.is_empty() && !form.busy;
    let submit = std::cell::Cell::new(false);
    let busy = form.busy;
    crate::widgets::sheet(
        ctx,
        "sign-in",
        520.0,
        (tl!("Sign in"), tl!("To your PingCraft server")),
        |ui| {
            field(ui, tl!("Server address"), &mut form.server, false, "example.org");
            note(
                ui,
                tl!(
                    "The server you run, or the one your admin runs for you. PingCraft talks only to this server, and your messages are kept there, not with us or anyone else."
                ),
            );
            field(ui, tl!("Username"), &mut form.username, false, "");
            let password = field(ui, tl!("Password"), &mut form.password, true, "");
            if password.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit.set(true);
            }
            if let Some(p) = &form.problem {
                problem(ui, &sign_in_problem(p));
            }
        },
        |ui| {
            note(ui, tl!("Forgot your password? Ask your server's admin."));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if busy { tl!("Signing in…") } else { tl!("Sign in") };
                if button(ui, label, Kind::Primary, ready).clicked() {
                    submit.set(true);
                }
            });
        },
    );
    if submit.get() && ready {
        let form = &mut state.sign_in;
        out.push(Command::SignIn {
            server: form.server.trim().to_owned(),
            username: form.username.trim().to_owned(),
            password: std::mem::take(&mut form.password),
        });
    }
}

fn sign_in_problem(p: &SignInProblem) -> String {
    match p {
        SignInProblem::ServerNotFound => tl!("No PingCraft server answered at that address. Check it, or ask your admin.").to_owned(),
        SignInProblem::Unreachable => tl!("Could not reach the server. Check your internet connection and the address, then try again.").to_owned(),
        SignInProblem::WrongPassword => tl!("The server did not accept that username and password.").to_owned(),
        SignInProblem::Secrets => tl!("PingCraft could not keep your sign-in safely on this computer.").to_owned(),
        SignInProblem::Other(why) => fmt(tl!("Signing in did not work: {why}"), &[("why", why)]),
    }
}

pub fn unlock(ctx: &egui::Context, state: &mut State, first: bool, out: &mut Out) {
    if state.unlock.confirm_forget {
        return confirm_forget(ctx, state, out);
    }
    let form = &mut state.unlock;
    let same = !first || *form.passphrase == *form.again;
    let ready = !form.passphrase.is_empty() && same;
    let submit = std::cell::Cell::new(false);
    let mut forget = false;
    let (title, caption) = if first {
        (tl!("Choose a passphrase"), tl!("No keychain on this computer"))
    } else {
        (tl!("Unlock PingCraft"), tl!("No keychain on this computer"))
    };
    crate::widgets::sheet(
        ctx,
        "unlock",
        520.0,
        (title, caption),
        |ui| {
            if first {
                note(
                    ui,
                    tl!(
                        "This computer has no keychain, so PingCraft locks what it keeps here (your sign-in and your messages) with a passphrase you choose. You type it each time PingCraft starts."
                    ),
                );
            } else {
                note(
                    ui,
                    tl!(
                        "Type your passphrase to open the messages kept on this computer. There is no keychain here, so PingCraft locks them with your passphrase instead."
                    ),
                );
            }
            let response = field(ui, tl!("Passphrase"), &mut form.passphrase, !form.show, "");
            if first {
                field(ui, tl!("The same passphrase again"), &mut form.again, !form.show, "");
                if !same && !form.again.is_empty() {
                    problem(ui, tl!("The two passphrases are not the same."));
                }
            } else if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit.set(true);
            }
            let show = if form.show { tl!("Hide") } else { tl!("Show") };
            if button(ui, show, Kind::Secondary, true).clicked() {
                form.show = !form.show;
            }
            if form.wrong {
                problem(ui, tl!("That passphrase does not unlock PingCraft."));
            }
            if !first {
                note(
                    ui,
                    tl!(
                        "Forgot it? Sign out, then sign in again with your password and recovery key. What's kept on this computer is cleared and comes back from your server."
                    ),
                );
            }
        },
        |ui| {
            if !first {
                forget = button(ui, tl!("Sign out"), Kind::Secondary, true).clicked();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if first { tl!("Continue") } else { tl!("Unlock") };
                if button(ui, label, Kind::Primary, ready).clicked() {
                    submit.set(true);
                }
            });
        },
    );
    if forget {
        state.unlock.confirm_forget = true;
    } else if submit.get() && ready {
        let passphrase = std::mem::take(&mut state.unlock.passphrase);
        state.unlock.again = Zeroizing::default();
        state.unlock.wrong = false;
        out.push(Command::Unlock { passphrase });
    }
}

/// Another PingCraft has this computer's data open. Two at once could lose the keys to the messages kept here, so
/// this one stops before touching anything, and says so.
pub fn already_running(ctx: &egui::Context) {
    crate::widgets::sheet(
        ctx,
        "already-running",
        520.0,
        (tl!("PingCraft is already open"), tl!("On this computer")),
        |ui| {
            ui.label(RichText::new(tl!("Another PingCraft window is using your messages on this computer.")).size(14.0));
            note(
                ui,
                tl!(
                    "Only one can use them at a time, so that neither overwrites what the other keeps. Switch to the PingCraft that is already open, or close it and start PingCraft again."
                ),
            );
        },
        |_| {},
    );
}

/// Signing out without the passphrase clears everything kept on this computer, so it is asked twice.
fn confirm_forget(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let mut cancel = false;
    let mut clear = false;
    crate::widgets::sheet(
        ctx,
        "unlock",
        520.0,
        (tl!("Sign out on this computer?"), tl!("No keychain on this computer")),
        |ui| {
            ui.label(RichText::new(tl!("Without the passphrase, PingCraft can only clear everything it keeps on this computer.")).size(14.0));
            note(
                ui,
                tl!(
                    "Your messages stay on your server. When you sign in again, encrypted ones come back only with your recovery key, or from another device where you're signed in. If this is your only device and you don't have your recovery key, your encrypted history is lost."
                ),
            );
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                clear = button(ui, tl!("Clear and sign out"), Kind::Primary, true).clicked();
            });
        },
    );
    if cancel {
        state.unlock.confirm_forget = false;
    } else if clear {
        state.unlock.confirm_forget = false;
        out.push(Command::SignOut);
    }
}

// -----------------------------------------------------------------------------------------------------------------
// The inbox

pub fn inbox(ui: &mut Ui, state: &mut State, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::left("inbox").exact_size(380.0).resizable(false).frame(egui::Frame::new().fill(t.bg)).show(ui, |ui| {
        inbox_list(ui, state, out);
    });
    let mut leave = false;
    egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show(ui, |ui| match state.open.as_mut() {
        Some(open) => {
            let chat = state.inbox.get(&open.id).cloned();
            leave = crate::chat::chat_view(ui, open, chat.as_ref(), out);
        }
        None => empty_chat(ui, state, out),
    });
    if leave {
        state.open = None;
    }
    forward_sheet(ui.ctx(), state, out);
}

/// Forwarding messages from the open chat: search, pick a chat, FORWARD.
fn forward_sheet(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let Some(open) = state.open.as_mut() else { return };
    let Some(forwarding) = open.forwarding.as_mut() else { return };
    let from = open.id.clone();
    let count = forwarding.messages.len();
    let title = match count {
        1 => tl!("Forward a message").to_owned(),
        n => fmt(tl!("Forward {n} messages"), &[("n", &n.to_string())]),
    };
    let chats: Vec<Chat> = state.inbox.visible(Filter::All, &forwarding.search).filter(|c| c.id != from).take(50).cloned().collect();
    let (mut close, mut send) = (false, false);
    let chosen = forwarding.to.is_some();
    crate::widgets::sheet(
        ctx,
        "forward",
        600.0,
        (&title, tl!("To one of your chats")),
        |ui| {
            let search = ui.add(egui::TextEdit::singleline(&mut forwarding.search).hint_text(tl!("Search chats")).desired_width(f32::INFINITY));
            search.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, tl!("Search chats")));
            ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for chat in &chats {
                    let on = forwarding.to.as_ref() == Some(&chat.id);
                    let label = fmt(tl!("{chat}, on {network}"), &[("chat", &chat.name), ("network", &network_name(&chat.network))]);
                    let response = ui.add(egui::RadioButton::new(on, &chat.name));
                    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, &label));
                    if response.clicked() {
                        forwarding.to = Some(chat.id.clone());
                    }
                }
            });
            note(
                ui,
                tl!("They are sent as new messages from you, on that chat's network. They don't say “forwarded”: the bridges don't carry that yet."),
            );
        },
        |ui| {
            close = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                send = button(ui, tl!("Forward"), Kind::Primary, chosen).clicked();
            });
        },
    );
    if send && let Some(to) = forwarding.to.clone() {
        out.push(Command::Forward { from, messages: forwarding.messages.clone(), to });
        open.selected = None;
    }
    if send || close {
        open.forwarding = None;
    }
}

fn inbox_list(ui: &mut Ui, state: &mut State, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    egui::Frame::new().inner_margin(Margin { left: 16, right: 16, top: 16, bottom: 8 }).show(ui, |ui| {
        ui.horizontal(|ui| {
            let all = format!("{} {}", tl!("All").to_uppercase(), state.inbox.len());
            let unread = format!("{} {}", tl!("Unread").to_uppercase(), state.inbox.unread_chats());
            for (label, filter) in [(all, Filter::All), (unread, Filter::Unread)] {
                let on = state.filter == filter;
                let text = RichText::new(label).monospace().size(11.0).color(if on { t.invert_fg } else { t.muted });
                let cell = egui::Button::new(text).stroke(Stroke::new(1.0, t.line_strong)).selected(on);
                let (fill, hover) = if on { (t.invert_bg, t.invert_bg) } else { (Color32::TRANSPARENT, t.line) };
                if crate::widgets::animated(ui, cell, true, fill, hover).clicked() {
                    state.filter = filter;
                }
            }
            let search = egui::TextEdit::singleline(&mut state.search)
                .hint_text(tl!("Search chats"))
                .align(egui::Align2::LEFT_CENTER)
                .desired_width(f32::INFINITY);
            let search = ui.add(search);
            search.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, tl!("Search chats")));
        });
        ui.add_space(8.0);
        hud(ui, &format!("{} · {}", tl!("Inbox"), trn(state.inbox.len() as u64, "{n} chat", "{n} chats")), t.muted);
        if state.recovery == RecoveryStatus::NeedsKey {
            needs_key(ui, state, out);
        }
    });
    ui.painter().hline(ui.max_rect().x_range(), ui.min_rect().bottom(), t.hairline());

    let visible: Vec<Chat> = state.inbox.visible(state.filter, &state.search).cloned().collect();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if visible.is_empty() {
            egui::Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
                let text = if !state.inbox_loaded {
                    tl!("Loading your chats…")
                } else if state.inbox.is_empty() {
                    tl!("No chats yet. Chats from the networks you connect, and your Matrix chats, show up here.")
                } else {
                    tl!("No chat matches.")
                };
                note(ui, text);
            });
        }
        for chat in &visible {
            let selected = state.open.as_ref().is_some_and(|o| o.id == chat.id);
            if chat_row(ui, chat, selected).clicked() && !selected {
                state.open_chat(chat.id.clone());
                out.push(Command::OpenChat(chat.id.clone()));
            }
        }
    });
}

/// The notice for a device that cannot read the account's encrypted history yet.
fn needs_key(ui: &mut Ui, state: &mut State, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(8.0);
    egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.label(RichText::new(tl!("This device can't read your earlier encrypted messages yet.")).size(13.0));
        note(ui, tl!("Type your recovery key, or verify this device from another one where you're signed in."));
        ui.horizontal(|ui| {
            if button(ui, tl!("Enter recovery key"), Kind::Secondary, true).clicked() {
                state.recovery_forms.entering = true;
                state.recovery_forms.wrong = false;
            }
            if button(ui, tl!("Verify this device"), Kind::Secondary, state.verification.is_none()).clicked() {
                out.push(Command::VerifyThisDevice);
            }
        });
    });
}

/// What a message shows as in one line.
pub(crate) fn preview_text(body: &MessageBody) -> String {
    match body {
        MessageBody::Text(text) => text.lines().next().unwrap_or_default().to_owned(),
        MessageBody::WaitingForKey => tl!("Waiting for the key to this message").to_owned(),
        MessageBody::Deleted => unsupported("deleted"),
        MessageBody::Loading => String::new(),
        MessageBody::Unsupported(kind) => unsupported(kind),
    }
}

pub(crate) fn unsupported(kind: &str) -> String {
    match kind {
        "image" => tl!("An image").to_owned(),
        "file" => tl!("A file").to_owned(),
        "audio" => tl!("A voice or audio message").to_owned(),
        "video" => tl!("A video").to_owned(),
        "location" => tl!("A location").to_owned(),
        "sticker" => tl!("A sticker").to_owned(),
        "poll" => tl!("A poll").to_owned(),
        "deleted" => tl!("This message was deleted").to_owned(),
        _ => tl!("A message PingCraft can't show yet").to_owned(),
    }
}

/// The marks on a pinned and a muted chat's row.
pub(crate) const PINNED_MARK: &str = "📌";
pub(crate) const MUTED_MARK: &str = "🔕";

/// One chat in the list. Its accessible name says the chat, its network, the unread count and the last message.
fn chat_row(ui: &mut Ui, chat: &Chat, selected: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::click());
    let preview = chat.last_message.as_ref().map(|m| {
        let who = if m.own { tl!("You").to_owned() } else { m.sender.clone() };
        format!("{who}: {}", preview_text(&m.body))
    });
    let mut name = fmt(tl!("{chat}, on {network}"), &[("chat", &chat.name), ("network", &network_name(&chat.network))]);
    if chat.pinned {
        name.push_str(", ");
        name.push_str(tl!("pinned"));
    }
    if chat.muted.is_some() {
        name.push_str(", ");
        name.push_str(tl!("muted"));
    }
    if chat.unread > 0 {
        name.push_str(", ");
        name.push_str(&trn(u64::from(chat.unread), "{n} unread", "{n} unread"));
    } else if chat.marked_unread {
        name.push_str(", ");
        name.push_str(tl!("marked unread"));
    }
    if let Some(p) = &preview {
        name.push_str(". ");
        name.push_str(p);
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::Button, true, &name);
        info.selected = Some(selected);
        info
    });
    if selected {
        ui.painter().rect_filled(rect, 0.0, t.sunken);
        ui.painter().vline(rect.left() + 1.0, rect.y_range(), Stroke::new(2.0, t.accent_ink));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, t.sunken);
    }
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, t.hairline());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(16.0, 10.0))).layout(Layout::left_to_right(Align::Center)));
    // The theme's tall interact height is for buttons and fields; here each line is only as tall as its text.
    child.spacing_mut().interact_size.y = 0.0;
    child.spacing_mut().item_spacing.y = 4.0;
    // The row's text is not selectable, so a click anywhere on the row opens the chat.
    child.style_mut().interaction.selectable_labels = false;
    avatar(&mut child, &chat.name, 36.0);
    child.add_space(4.0);
    let two_lines = preview.is_some() || chat.is_unread();
    child.vertical(|ui| {
        let strong = chat.is_unread();
        // Centre the one or two lines in the row by hand: a vertical block in a centred row starts at the top.
        let name_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(14.0)));
        let preview_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(12.5)));
        let height = if two_lines { name_h + 4.0 + preview_h } else { name_h };
        ui.add_space(((ui.available_height() - height) / 2.0).max(0.0));
        // Time and network tag first, from the right, so a long name truncates instead of pushing them down.
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let when = chat.last_message.as_ref().map(|m| short_time(m.at)).unwrap_or_default();
                ui.label(RichText::new(when).monospace().size(12.0).color(if strong { t.ink } else { t.muted }));
                network_tag(ui, &chat.network);
                // Marks drawn from the bundled emoji font (a test checks they have glyphs).
                if chat.muted.is_some() {
                    ui.label(RichText::new(MUTED_MARK).size(12.0).color(t.muted));
                }
                if chat.pinned {
                    ui.label(RichText::new(PINNED_MARK).size(12.0).color(t.muted));
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let mut name = RichText::new(&chat.name).size(14.0).color(if strong { t.ink_strong } else { t.ink });
                    if strong {
                        name = name.strong();
                    }
                    ui.add(egui::Label::new(name).truncate());
                });
            });
        });
        if !two_lines {
            return;
        }
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if chat.unread > 0 {
                    unread(ui, chat.unread, chat.muted.is_some());
                } else if chat.marked_unread {
                    crate::widgets::marked_unread(ui);
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let text = preview.clone().unwrap_or_default();
                    ui.add(egui::Label::new(RichText::new(text).size(12.5).color(if strong { t.ink } else { t.muted })).truncate());
                });
            });
        });
    });
    response
}

fn empty_chat(ui: &mut Ui, state: &mut State, out: &mut Out) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(ui.available_height() / 3.0);
    ui.vertical_centered(|ui| {
        if state.inbox_loaded && state.inbox.is_empty() {
            ui.label(RichText::new(tl!("Your chats, one inbox.")).size(26.0).strong().color(t.ink_strong));
            ui.add_space(8.0);
            note(ui, tl!("Networks are connected through bridges, small programs on your server. Connect one, and its chats show up here."));
            ui.add_space(8.0);
            if button(ui, tl!("Connect a network"), Kind::Primary, true).clicked() {
                open_accounts(state, out);
            }
        } else {
            note(ui, tl!("Choose a chat on the left."));
        }
    });
}

// -----------------------------------------------------------------------------------------------------------------
// Encryption sheets

/// The new recovery key, shown once after the first sign-in.
pub fn recovery_key(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    let Some(key) = state.recovery_forms.new_key.clone() else { return };
    let user = state.user.clone().unwrap_or_default();
    let caption = format!("{} · {user}", tl!("First sign-in").to_uppercase());
    let ticked = std::cell::Cell::new(state.recovery_forms.saved_ticked);
    let copied = std::cell::Cell::new(state.recovery_forms.copied);
    let mut done = false;
    crate::widgets::sheet(
        ctx,
        "recovery-key",
        640.0,
        (tl!("Save your recovery key"), &caption),
        |ui| {
            let t = Tokens::get(ui.ctx());
            ui.label(RichText::new(tl!("Your messages are encrypted. This key unlocks them on a new device: PingCraft keeps a backup of your message keys on your server, locked with it.")).size(14.0));
            egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(Margin::symmetric(20, 14)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for row in key.groups().chunks(6) {
                    ui.label(RichText::new(row.join(" ")).monospace().size(22.0).color(t.ink_strong));
                }
            });
            let label = if copied.get() { tl!("Copied") } else { tl!("Copy") };
            if button(ui, label, Kind::Secondary, true).clicked() {
                ui.ctx().copy_text(key.expose().to_owned());
                copied.set(true);
            }
            egui::Frame::new().stroke(t.hairline()).inner_margin(Margin::same(14)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                hud(ui, tl!("If you lose it"), t.muted);
                ui.label(RichText::new(tl!("If you lose every device you're signed in on and this key, your encrypted message history is gone for good. Nobody can get it back for you, not even your server's admin.")).size(13.0));
            });
            let mut on = ticked.get();
            ui.checkbox(&mut on, tl!("I've saved my recovery key somewhere safe, away from this computer."));
            ticked.set(on);
        },
        |ui| {
            note(ui, tl!("Keep it like a password: never send it in a chat."));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                done = button(ui, tl!("Continue"), Kind::Primary, ticked.get()).clicked();
            });
        },
    );
    state.recovery_forms.saved_ticked = ticked.get();
    state.recovery_forms.copied = copied.get();
    if done {
        state.recovery_forms.new_key = None;
        state.recovery_forms.saved_ticked = false;
        state.recovery_forms.copied = false;
        out.push(Command::RecoveryKeySaved);
    }
}

/// Typing the account's recovery key on this device.
pub fn enter_recovery_key(ctx: &egui::Context, state: &mut State, out: &mut Out) {
    if !state.recovery_forms.entering {
        return;
    }
    let forms = &mut state.recovery_forms;
    let mut cancel = false;
    let mut submit = false;
    let ready = !forms.typed.trim().is_empty();
    crate::widgets::sheet(
        ctx,
        "enter-recovery-key",
        560.0,
        (tl!("Enter your recovery key"), tl!("This device")),
        |ui| {
            note(
                ui,
                tl!(
                    "The key you saved when you first signed in: 48 letters and digits in groups of four. It lets this device read your earlier encrypted messages."
                ),
            );
            field(ui, tl!("Recovery key"), &mut forms.typed, false, "EsTc 8Jq2 …");
            if forms.wrong {
                problem(ui, tl!("That key doesn't open your key backup. Check it and try again."));
            }
        },
        |ui| {
            cancel = button(ui, tl!("Cancel"), Kind::Secondary, true).clicked();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                submit = button(ui, tl!("Use this key"), Kind::Primary, ready).clicked();
            });
        },
    );
    if cancel {
        forms.entering = false;
        forms.typed = Zeroizing::default();
    } else if submit && ready {
        forms.wrong = false;
        out.push(Command::Recover(RecoveryKey::new(forms.typed.trim())));
    }
}

/// Verifying this device with another of the user's, by comparing symbols.
pub fn verification(ctx: &egui::Context, state: &State, out: &mut Out) {
    let Some(v) = &state.verification else { return };
    let title = tl!("Verify your other device");
    let caption = match v {
        Verification::Incoming { device } => device.clone(),
        _ => tl!("Your devices").to_owned(),
    };
    crate::widgets::sheet(
        ctx,
        "verify",
        720.0,
        (title, &caption),
        |ui| match v {
            Verification::Incoming { device } => {
                ui.label(fmt(
                    tl!("Your device {device} asks to be verified. Start only if you asked for it yourself, on that device."),
                    &[("device", device)],
                ));
            }
            Verification::Waiting => {
                ui.label(tl!("Waiting for your other device. Open PingCraft (or another Matrix app) there and accept."));
            }
            Verification::Compare(symbols) => {
                ui.label(tl!("Your other device is showing 7 symbols. Check that they are the same here, in the same order. If they are, the device is really yours and can read your encrypted messages."));
                symbol_tiles(ui, symbols);
                note(
                    ui,
                    tl!(
                        "If they don't match, someone may be trying to get into your account. Choose \"They don't match\": nothing is shared, and you can try again."
                    ),
                );
            }
            Verification::Confirmed => {
                ui.label(tl!("Waiting for your other device to confirm too."));
            }
            Verification::Done => {
                ui.label(tl!("Verified. Your devices trust each other, and this one can read your encrypted messages."));
            }
            Verification::Cancelled(reason) => {
                ui.label(match reason {
                    CancelReason::Mismatch => tl!("Stopped: the symbols didn't match. Nothing was shared."),
                    CancelReason::Cancelled => tl!("Stopped on one of the devices. Nothing was shared."),
                    CancelReason::TimedOut => tl!("Your other device didn't answer in time. Nothing was shared."),
                    CancelReason::Other(_) => tl!("The verification stopped. Nothing was shared."),
                });
            }
        },
        |ui| {
            let (secondary, primary) = match v {
                Verification::Incoming { .. } => {
                    (Some((tl!("Not now"), Command::CancelVerification)), Some((tl!("Start"), Command::AcceptVerification)))
                }
                Verification::Compare(_) => {
                    (Some((tl!("They don't match"), Command::SymbolsDiffer)), Some((tl!("They match"), Command::SymbolsMatch)))
                }
                Verification::Waiting | Verification::Confirmed => (Some((tl!("Cancel"), Command::CancelVerification)), None),
                Verification::Done | Verification::Cancelled(_) => (None, Some((tl!("Close"), Command::CancelVerification))),
            };
            if let Some((label, command)) = secondary
                && button(ui, label, Kind::Secondary, true).clicked()
            {
                out.push(command);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some((label, command)) = primary
                    && button(ui, label, Kind::Primary, true).clicked()
                {
                    out.push(command);
                }
            });
        },
    );
}

fn symbol_tiles(ui: &mut Ui, symbols: &[SasSymbol]) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        for s in symbols {
            egui::Frame::new().fill(t.sunken).stroke(t.hairline()).inner_margin(Margin::symmetric(4, 10)).show(ui, |ui| {
                ui.set_width(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(&s.symbol).size(34.0));
                    // The spec's English name; translated through the catalog like any other text.
                    ui.label(RichText::new(tl!(&s.name)).size(12.0).color(t.muted));
                });
            });
        }
    });
}

/// A problem the user should know about, with a way to close it.
pub fn problem_bar(ui: &mut Ui, state: &mut State) {
    let Some(p) = state.problem.clone() else { return };
    let t = Tokens::get(ui.ctx());
    egui::Frame::new().fill(t.sunken).inner_margin(Margin::symmetric(16, 6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(fmt(tl!("Something went wrong: {problem}"), &[("problem", &p)])).size(13.0).color(t.bad));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, tl!("Close"), Kind::Secondary, true).clicked() {
                    state.problem = None;
                }
            });
        });
    });
}
