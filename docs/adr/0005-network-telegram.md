# ADR 0005: Telegram, risk level low

- **Status:** accepted by the maintainer (2026-11-24)

## Context

Telegram is bridged by mautrix-telegram (AGPL-3.0, Go, on bridgev2 since v0.2604.0, 2026-04-16; v0.2611.0 on
2026-11-16). It logs in with a phone number and a code sent to an official Telegram app (plus the account's
two-step password if set), or by scanning a QR code, or as a bot with a bot token. It cannot create a Telegram
account.

Telegram's API terms (https://core.telegram.org/api/terms) allow third-party clients. Each client must use its
own `api_id` and `api_hash` from my.telegram.org, must keep read receipts, typing notifications and
self-destructing messages working, and must not use Telegram data to train AI. A breach gets a notice and a
window to fix it before API access is cut. New accounts are more likely to be flagged.

## Decision

- Risk level: **low**.
- The admin supplies their own `api_id` and `api_hash`; PingCraft never ships one.
- PingCraft shows the risk and the user confirms before connecting a Telegram account.

## Alternatives

- Ship a shared PingCraft `api_id`: easier setup, but the terms tie an `api_id` to one application and its
  developer, and one misbehaving server would put every user's access at risk.

## Consequences

- Setup has one extra step: registering an application at my.telegram.org.
- The `api_hash` is a secret and lives only in the bridge's configuration.
