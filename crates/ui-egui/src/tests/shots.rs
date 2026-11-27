//! Screenshots of the main screens, for looking at by eye: the inbox, an open chat, Accounts and each Settings page,
//! dark and light, wide and narrow. Rendered with wgpu only when `PINGCRAFT_SHOTS` names a folder to write them to;
//! otherwise it renders nothing (there is no GPU in CI). It checks nothing: the AccessKit tests do that.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pingcraft_core::app::Event;
use pingcraft_core::{ChatId, InboxChange};

use super::{Fake, chat, message};
use crate::{Appearance, PingCraftApp};

fn app(fake: &Fake, appearance: Appearance, size: egui::Vec2) -> Harness<'static, PingCraftApp> {
    let ctx = super::context();
    let mut app = PingCraftApp::new(Box::new(fake.clone()));
    app.set_prefs(crate::Prefs { appearance, previews: true });
    app.prime(&ctx);
    Harness::builder().with_context(ctx).with_size(size).with_max_steps(12).wgpu().build_ui_state(super::frame, app)
}

fn chats() -> Vec<pingcraft_core::Chat> {
    let mut long =
        chat("!long", "The extremely long name of a family group chat that goes on and on", "whatsapp", 12, Some(("Mor", "Husk middag på søndag")));
    long.pinned = true;
    vec![
        chat("!book", "Book club", "telegram", 2, Some(("Lars", "The ending of chapter 8"))),
        chat("!lars", "Lars Holm", "matrix", 0, Some(("Lars", "See you at eight"))),
        long,
        chat("!work", "Work", "signal", 1, Some(("Mei", "Standup moved to 10"))),
        chat("!disc", "Gaming night", "discord", 0, None),
    ]
}

fn save(h: &mut Harness<'static, PingCraftApp>, dir: &std::path::Path, name: &str) {
    h.run();
    match h.render() {
        Ok(image) => {
            let path = dir.join(format!("{name}.png"));
            image.save(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        Err(e) => panic!("rendering {name}: {e}"),
    }
}

#[test]
fn screenshots_of_the_main_screens() {
    let Some(dir) = std::env::var_os("PINGCRAFT_SHOTS").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for (mode, appearance) in [("dark", Appearance::Dark), ("light", Appearance::Light)] {
        for (width, size) in [("wide", egui::vec2(1200.0, 800.0)), ("narrow", egui::vec2(800.0, 600.0))] {
            let fake = Fake::default();
            let mut h = app(&fake, appearance, size);
            fake.tell([
                Event::SignedIn { user: "@ana:example.org".into() },
                Event::Connection(pingcraft_core::ConnectionState::Online),
                Event::Recovery(pingcraft_core::encryption::RecoveryStatus::On),
                Event::Inbox(InboxChange::Reset(chats())),
            ]);
            h.run();
            let tag = format!("{mode}-{width}");
            save(&mut h, &dir, &format!("inbox-{tag}"));
            h.get_by_label_contains("Book club, on Telegram").click();
            h.run();
            fake.tell([Event::Messages {
                chat: ChatId("!book".into()),
                messages: vec![
                    message(
                        "1",
                        "Lars",
                        false,
                        pingcraft_core::MessageBody::Text("Has everyone finished chapter 8?".into()),
                        pingcraft_core::SendState::Sent,
                    ),
                    message(
                        "2",
                        "@ana:example.org",
                        true,
                        pingcraft_core::MessageBody::Text("Yes! What an ending.".into()),
                        pingcraft_core::SendState::Sent,
                    ),
                ],
            }]);
            h.run();
            save(&mut h, &dir, &format!("chat-{tag}"));
            h.get_by_label("Accounts").click();
            h.run();
            save(&mut h, &dir, &format!("accounts-{tag}"));
            h.get_by_label("Settings").click();
            h.run();
            for page in ["Appearance", "Notifications", "Emoji", "Security"] {
                h.get_by_label("Pages").get_by_label(page).click();
                h.run();
                save(&mut h, &dir, &format!("settings-{}-{tag}", page.to_lowercase()));
            }
        }
    }
}
