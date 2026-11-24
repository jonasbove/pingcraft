# ADR 0007: WhatsApp, risk level medium-high

- **Status:** accepted by the maintainer (2026-11-24)

## Context

WhatsApp is bridged by mautrix-whatsapp (AGPL-3.0, Go, bridgev2; v0.2611.0 on 2026-11-16), built on the
`whatsmeow` protocol library (MPL-2.0). It logs in as a linked device, by scanning a QR code in WhatsApp's linked
devices screen or by entering an eight-character pairing code; some accounts must also confirm with a passkey.
The phone must come online regularly: the link drops after about 14 days offline, and the bridge warns at 12.

WhatsApp's terms (https://www.whatsapp.com/legal/terms-of-service) forbid accessing the service by automated or
unauthorised means and building software that works like it. The bridge's documentation says WhatsApp bans
accounts it finds suspicious, and that the risk rises with new accounts, VoIP numbers, emulators, and messaging
people who are not contacts.

## Decision

- Risk level: **medium-high**: unofficial clients are against the terms, and bans happen.
- PingCraft shows the risk, including the 14-day rule, and the user confirms before connecting a WhatsApp
  account.

## Alternatives

- Leave WhatsApp out: it is one of the four networks the maintainer decided on, and the risk is the user's to
  take once it is explained.

## Consequences

- PingCraft surfaces the bridge's "phone offline" warnings in the inbox, so the link does not silently drop.
