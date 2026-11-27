//! PingCraft's shortcuts through craft-ui's command board: the tabs, search, Refresh, moving between chats (all and
//! unread), marking read, Escape back to the list, and the app menu's Go and Chat menus.

use egui::accesskit::Role;
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use pingcraft_core::app::{Command, Event};
use pingcraft_core::{ChatId, Filter, InboxChange};

use super::{Fake, chat, signed_in};
use crate::{PingCraftApp, Tab};

/// Signed in with three chats: two with unread messages and one read between them in the inbox's order.
fn three_chats(fake: &Fake) -> Harness<'static, PingCraftApp> {
    let mut h = signed_in(fake);
    let mut newer = chat("!new", "Neighbours", "signal", 1, Some(("Mei", "Bins out tonight")));
    if let Some(m) = newer.last_message.as_mut() {
        m.at = pingcraft_core::Timestamp(m.at.0 + 60_000);
    }
    fake.tell([Event::Inbox(InboxChange::Upsert(Box::new(newer)))]);
    h.run();
    fake.take();
    h
}

/// The inbox's order now, as its list shows it.
fn order(h: &Harness<'static, PingCraftApp>) -> Vec<String> {
    let s = &h.state().state;
    s.inbox.visible(Filter::All, &s.search).map(|c| c.id.0.clone()).collect()
}

fn open(h: &Harness<'static, PingCraftApp>) -> Option<String> {
    h.state().state.open.as_ref().map(|o| o.id.0.clone())
}

fn press(h: &mut Harness<'static, PingCraftApp>, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    h.run();
    h.run();
}

#[test]
fn ctrl_1_and_2_switch_tabs_ctrl_f_searches_and_f5_refreshes() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    press(&mut h, Modifiers::COMMAND, Key::Num2);
    assert_eq!(h.state().state.tab, Tab::Accounts);
    assert!(fake.take().iter().any(|c| matches!(c, Command::LoadNetworks)));
    press(&mut h, Modifiers::COMMAND, Key::Num1);
    assert_eq!(h.state().state.tab, Tab::Inbox);
    press(&mut h, Modifiers::COMMAND, Key::Num2);
    press(&mut h, Modifiers::COMMAND, Key::F);
    assert_eq!(h.state().state.tab, Tab::Inbox, "search shows the inbox");
    assert!(h.get_by_role_and_label(Role::TextInput, "Search chats").accesskit_node().is_focused(), "with its search focused");
    fake.take();
    h.key_press(Key::Escape);
    h.run();
    press(&mut h, Modifiers::NONE, Key::F5);
    assert!(fake.take().iter().any(|c| matches!(c, Command::Refresh)));
}

#[test]
fn ctrl_tab_moves_through_the_chats_and_wraps_even_from_the_composer() {
    let fake = Fake::default();
    let mut h = three_chats(&fake);
    let order = order(&h);
    assert_eq!(order.len(), 3);
    press(&mut h, Modifiers::CTRL, Key::Tab);
    assert_eq!(open(&h).as_deref(), Some(order[0].as_str()), "with none open, the first");
    assert!(fake.take().iter().any(|c| matches!(c, Command::OpenChat(ChatId(id)) if *id == order[0])), "opened, so marked read");
    // From the composer: the shortcut wins over the field's Tab.
    let composer = h
        .get_all_by_role(Role::TextInput)
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Message")))
        .map(|n| n.accesskit_node().label().unwrap_or_default());
    let label = composer.expect("the open chat's composer");
    h.get_by_role_and_label(Role::TextInput, &label).focus();
    h.run();
    assert!(h.get_by_role_and_label(Role::TextInput, &label).accesskit_node().is_focused(), "the composer has the keys");
    press(&mut h, Modifiers::CTRL, Key::Tab);
    assert_eq!(open(&h).as_deref(), Some(order[1].as_str()));
    press(&mut h, Modifiers::CTRL, Key::Tab);
    press(&mut h, Modifiers::CTRL, Key::Tab);
    assert_eq!(open(&h).as_deref(), Some(order[0].as_str()), "wraps at the end");
    press(&mut h, Modifiers::CTRL | Modifiers::SHIFT, Key::Tab);
    assert_eq!(open(&h).as_deref(), Some(order[2].as_str()), "and at the start");
    press(&mut h, Modifiers::ALT, Key::ArrowUp);
    assert_eq!(open(&h).as_deref(), Some(order[1].as_str()), "Alt+Up");
    press(&mut h, Modifiers::ALT, Key::ArrowDown);
    assert_eq!(open(&h).as_deref(), Some(order[2].as_str()), "Alt+Down");
    press(&mut h, Modifiers::COMMAND, Key::PageDown);
    assert_eq!(open(&h).as_deref(), Some(order[0].as_str()), "Ctrl+Page Down");
    press(&mut h, Modifiers::COMMAND, Key::PageUp);
    assert_eq!(open(&h).as_deref(), Some(order[2].as_str()), "Ctrl+Page Up");
}

#[test]
fn the_unread_shortcuts_skip_read_chats() {
    let fake = Fake::default();
    let mut h = three_chats(&fake);
    let unread: Vec<String> = {
        let s = &h.state().state;
        s.inbox.visible(Filter::All, "").filter(|c| c.is_unread()).map(|c| c.id.0.clone()).collect()
    };
    assert_eq!(unread.len(), 2);
    press(&mut h, Modifiers::ALT | Modifiers::SHIFT, Key::ArrowDown);
    assert_eq!(open(&h).as_deref(), Some(unread[0].as_str()));
    press(&mut h, Modifiers::CTRL | Modifiers::ALT, Key::Tab);
    assert_eq!(open(&h).as_deref(), Some(unread[1].as_str()), "the read chat between is skipped");
    press(&mut h, Modifiers::ALT | Modifiers::SHIFT, Key::ArrowUp);
    assert_eq!(open(&h).as_deref(), Some(unread[0].as_str()));
}

#[test]
fn escape_goes_back_to_the_list_and_ctrl_shift_a_marks_read() {
    let fake = Fake::default();
    let mut h = three_chats(&fake);
    press(&mut h, Modifiers::ALT, Key::ArrowDown);
    let id = open(&h).expect("a chat is open");
    fake.take();
    press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::A);
    assert!(fake.take().iter().any(|c| matches!(c, Command::MarkRead(ChatId(c)) if *c == id)));
    // Nothing has the focus: Escape is the window's, and the chat closes.
    h.key_press(Key::Escape);
    h.run();
    h.run();
    assert_eq!(open(&h), None, "back to the list");
    assert!(fake.take().iter().any(|c| matches!(c, Command::CloseChat)));
}

#[test]
fn the_go_menu_lists_the_moves_with_their_keys() {
    let fake = Fake::default();
    let mut h = signed_in(&fake);
    h.get_by_label("Go").click();
    h.run();
    for item in ["Inbox", "Accounts", "Search chats", "Next chat", "Previous chat", "Next unread chat", "Previous unread chat"] {
        h.get_by_role(Role::Menu).get_by_label_contains(item);
    }
    h.get_by_label_contains("Next chat").click();
    h.run();
    h.run();
    assert!(open(&h).is_some(), "the menu row opens a chat");
}

/// The inbox's filter and search sit on their own lines without overlapping, and every unread count is inside its
/// chat's row: none is drawn loose between the filter's options.
#[test]
fn the_filter_does_not_overlap_and_counts_stay_in_their_rows() {
    let fake = Fake::default();
    let h = three_chats(&fake);
    let bounds = |label: &str| h.get_by_label(label).accesskit_node().raw_bounds().expect("bounds");
    let parts = [bounds("All"), bounds("Unread"), bounds("Archived")];
    let search = h.get_by_role_and_label(Role::TextInput, "Search chats").accesskit_node().raw_bounds().expect("bounds");
    let overlap = |a: egui::accesskit::Rect, b: egui::accesskit::Rect| a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
    for (i, a) in parts.iter().enumerate() {
        assert!(!overlap(*a, search), "a filter option over the search field");
        for b in &parts[i + 1..] {
            assert!(!overlap(*a, *b), "two filter options overlap");
        }
    }
    assert!(parts.windows(2).all(|w| w[0].x1 <= w[1].x0 + 0.5), "in order, left to right");
    let rows: Vec<egui::accesskit::Rect> = h
        .get_all_by_role(Role::Button)
        .filter(|n| n.accesskit_node().label().is_some_and(|l| l.contains(", on ")))
        .filter_map(|n| n.accesskit_node().raw_bounds())
        .collect();
    let is_count = |text: &str| {
        let t = text.trim().trim_start_matches('(').trim_end_matches(')');
        !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '+')
    };
    let mut counts = 0;
    for node in h.root().query_all(egui_kittest::kittest::by()) {
        let n = node.accesskit_node();
        let text = n.label().or_else(|| n.value()).unwrap_or_default();
        if !is_count(&text) {
            continue;
        }
        counts += 1;
        let Some(b) = n.raw_bounds() else { continue };
        let inside = rows.iter().any(|r| b.x0 >= r.x0 - 0.5 && b.x1 <= r.x1 + 0.5 && b.y0 >= r.y0 - 0.5 && b.y1 <= r.y1 + 0.5);
        assert!(inside, "a count {text:?} outside every chat row, at {b:?}");
    }
    assert!(counts >= 2, "the unread counts were found");
}
