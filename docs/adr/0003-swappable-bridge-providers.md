# ADR 0003: Bridges sit behind a swappable bridge provider

- **Status:** accepted by the maintainer (2026-11-24)

## Context

PingCraft starts with mautrix bridges (ADR 0002), but may later want its own bridges, another bridge provider, or
a different provider for one network. If the client talks to mautrix in mautrix-specific ways everywhere,
swapping becomes a rewrite.

Two mautrix interfaces matter to a client:

- the **provisioning API** of bridges built on `mautrix-go`'s bridgev2 (`/_matrix/provision/v3/...`, OpenAPI
  spec `bridgev2/matrix/provisioning.yaml`, MPL-2.0, v0.20.0): list login flows, start a login, answer each step
  (user input, cookies, a QR code or code to display and wait on), list and remove logins;
- the **bridge-info room state** that tells a client which network a chat belongs to: `m.bridge`, and the older
  `uk.half-shot.bridge`, from MSC2346. mautrix-go sends both. MSC2346 is not yet in the Matrix specification.

## Decision

1. `pingcraft-core` defines a bridge provider interface in PingCraft's own terms (GLOSSARY.md): which networks a
   provider offers, starting a login for an account and walking its steps (show a QR code, show a code, ask for
   input such as a phone number or token, wait), listing a user's accounts, and removing one.
2. mautrix is the first implementation, in its own module, using the bridgev2 provisioning API.
3. The client decides a chat's network only from the bridge-info room state, never from mautrix-specific data
   such as user ID prefixes or bot names.
4. The client should authenticate to a bridge with a Matrix OpenID token, not with the user's access token: an
   OpenID token only proves who the user is and cannot act for them. The provisioning API accepts one when it is
   sent as `openid:<token>` and `allow_matrix_auth` is on. The bridge then checks it against the homeserver's
   federation OpenID userinfo endpoint (`bridgev2/matrix/provisioning.go`). Whether tuwunel serves that endpoint
   with federation off is not yet verified, and is a spike in milestone 3. If it does not, the fallback is an
   open question for the maintainer, because the alternative means handing the bridge the user's access token.
5. On the server, each network's bridge is a compose service chosen by configuration: its image, its
   registration file and its provider.

## Alternatives

- **Talk to the bridge's management bot in a chat** (send `login qr`, read replies): works with every mautrix
  bridge, including the old Discord one, but ties the client to one provider's command text.
- **A different provider per network:** possible later on the same interface, but not decided.
- **Swappable on the server only:** cheaper now, but every login screen would need rewriting to change provider.

## Consequences

- Connecting a network from inside PingCraft needs a bridge built on bridgev2. mautrix-discord is not yet
  (ADR 0008), so Discord waits for its rewrite.
- Each bridge's `/_matrix/provision` endpoint has to be reachable from the client, through the reverse proxy.
  How it is routed (ADR 0010) is a milestone 3 ticket.
- The provisioning API is a mautrix-go interface, not a Matrix standard. Another provider either implements the
  same API or gets its own implementation of the interface.
- A bridge that sets no bridge-info state shows its chats under the Matrix network.
