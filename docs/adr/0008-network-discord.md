# ADR 0008: Discord comes last, with bot login by default

- **Status:** accepted by the maintainer (2026-11-24)

## Context

Discord is bridged by mautrix-discord (AGPL-3.0, Go). Its latest release, v0.7.7 (2026-08-16), is still on the
old bridge framework; the bridgev2 rewrite lives on the unreleased `megadiscord` branch. Its current release has
only an older, Discord-specific provisioning API (`/v1/login/qr`, `/v1/login/token`, ...), not the bridgev2 one,
so PingCraft's bridge provider (ADR 0003) could not connect it from inside the app.

It logs in three ways: a QR code scanned in the Discord mobile app (fails when Discord asks for a CAPTCHA), a
user token copied from a browser, or a bot token. Discord forbids automating a user account outside its bot API
("self-bots") and may close accounts that do
(https://discord.com/safety/platform-manipulation-policy-explainer). A bot account is allowed, but only sees the
servers it has been added to and never the user's direct messages.

## Decision

- Discord is the last of the four networks, and is bridged only once mautrix-discord's bridgev2 rewrite is
  released.
- Bot login is the default. Its risk level is **low**.
- Logging in as the user (QR code or user token) is offered only behind an explicit warning that Discord may
  close the account. Its risk level is **high**.

## Alternatives

- Bot login only: no ban risk, but no direct messages, which is most of what people want from Discord.
- Drop Discord: it is one of the four networks the maintainer decided on.

## Consequences

- Discord's milestone depends on upstream's release date, which PingCraft does not control.
- The default Discord account shows only servers the user adds the bot to; the interface says so.
