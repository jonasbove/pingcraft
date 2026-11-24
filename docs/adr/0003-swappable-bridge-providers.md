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
   with federation off is not yet verified, and is a spike in milestone 3. If it does not, federation stays off
   and only that endpoint is made reachable to the bridges, over the internal compose network. The user's
   access token is never handed to a bridge.
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

## Addendum (2026-11-24): OpenID with federation off, the milestone 3 spike

Run against a throwaway compose project built from `deploy/` (tuwunel v2.0.0, mautrix-signal v0.2611.0 built on
mautrix-go v0.33.0, the pinned digests), with federation off and a throwaway user. No network account was used.

Findings:

- **tuwunel serves the endpoint.** `GET /_matrix/federation/v1/openid/userinfo` answered 200 with the right
  `sub` for a token from `/_matrix/client/v3/user/{userId}/openid/request_token`, and 401 for a wrong token, while
  every other federation endpoint answered 403 "Federation is disabled".
- **The bridge does not ask the homeserver it is configured with.** It checks an `openid:` token with mautrix-go's
  federation client (`bridgev2/matrix/provisioning.go`, `federation/httpclient.go`), which:
  - resolves the user's server name the federation way (`.well-known/matrix/server`, SRV, then port 8448);
  - connects over HTTPS with ordinary certificate checks;
  - refuses private addresses (`federation.DefaultAllowIP`: RFC 1918, loopback and link-local).

  So the compose network alone is not enough: the server name did not resolve there ("no such host"), and once it
  did, the bridge refused the container's address ("refusing to connect to 172.26.0.3").
- **It works with three additions inside the stack.**
  1. The server name resolves, on a network outside RFC 1918, to a TLS front.
  2. The front forwards only the userinfo endpoint to tuwunel.
  3. The bridge trusts the front's certificate in addition to the system's roots.

  With those, `GET /_matrix/provision/v3/login/flows` with `Authorization: Bearer openid:<token>` answered 200.
- **The bridge logs the token when a check fails:** the error line includes the userinfo URL with the token in
  it. An OpenID token only proves who the user is, and tuwunel's last an hour, but it ends up in the bridge's
  container log.

The fix chosen, within point 4 above. It is built (milestone 3, ticket 3) but **waits for the maintainer's
approval**: it adds a reverse proxy to the server side that CLAUDE.md's "Decided" list and ADR 0010 describe.

- Federation stays off. No bridge image changes (ADR 0002).
- The compose project gets one reverse proxy (Caddy, Apache-2.0, pinned by digest).
  - On an internal network `openid`, in 198.18.0.0/24 (RFC 2544's benchmarking range: not private by the
    bridges' definition, and never routed on the internet), it answers to the server name on port 8448.
  - It uses a certificate `setup.sh` makes, self-signed and valid for 100 years. Its private key is given to the
    proxy only.
  - It forwards only `/_matrix/federation/v1/openid/userinfo`, and serves `.well-known/matrix/server` on port 443
    of the same network, so the bridge never follows a public SRV record.
- Each bridge joins `openid` and adds that certificate to its trusted roots with `SSL_CERT_DIR`. The system's own
  roots are still read.
- On the client side, the same proxy passes a bridge's `/_matrix/provision/v3/` only with `Bearer openid:`, so the
  bridges' other ways in (their shared secret, a user's access token) are closed from outside the stack.

For the maintainer:

- **The bridges' guard against private addresses no longer applies to one name.** Inside the stack, it does not
  apply to the server name, on purpose.
- **The clean fix is upstream.** A bridge could check tokens for its own server against the `homeserver.address`
  it is configured with. Proposing that to mautrix-go is a public contribution, so it waits for the maintainer.
- **Ruled out:** handing the bridge the user's access token (point 4), and turning federation on (ADR 0010).
