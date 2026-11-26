# ADR 0010: Deployment, and running next to CraftCloud

- **Status:** accepted by the maintainer (2026-11-24)

## Context

PingCraft runs either on the user's own computer or next to a CraftCloud server. CraftCloud runs as its own
compose project. It serves other Craft apps' web builds from `apps.toml` at `/apps/<id>/`, behind its own
sign-in gate. It can sign people in through an external OIDC provider, but it cannot act as one for other
services, and it gives apps no tokens.

A Matrix server's name is part of every user ID (`@name:example.com`) and cannot change without starting over.

## Decision

1. **One compose project, `pingcraft`:** tuwunel, one container per enabled bridge, and Postgres for the bridges.
   The same file serves local use and use next to CraftCloud.
2. **Server name:** the setup refuses to start without an explicit server name. The docs say it is permanent and
   suggest a domain the admin owns, delegated with `.well-known` when the homeserver lives on a subdomain.
3. **Users and federation:** the admin adds users; there is no open registration. Federation is off. Turning it
   on is a later decision.
4. **Sign-in:** users sign in to the homeserver with a password. A shared sign-in with CraftCloud would need an
   external identity provider used by both; that is a later, optional ADR.
5. **Next to CraftCloud:**
   - PingCraft stays a separate compose project.
   - The homeserver gets its own hostname behind the same reverse proxy.
   - CraftCloud serves the PingCraft web client from an `apps.toml` entry at `/apps/pingcraft/`, so the web
     client sits behind CraftCloud's sign-in, and the user then signs in to Matrix.
   - Adding that entry is a change to the CraftCloud repository, made only with the maintainer's go-ahead.
6. **Backups:** a documented procedure stops the stack, dumps the bridges' Postgres, and archives the
   homeserver's database and the bridge volumes. The archive contains readable messages and network logins,
   and is treated as a secret (ADR 0004).
7. **Updates:** every image is pinned by version and digest, and bumped through pull requests. Nothing updates
   itself.

## Alternatives

- **Add PingCraft's services to CraftCloud's compose project:** one thing to back up, but it ties two release
  cycles together and makes PingCraft harder to run alone.
- **Serve the web client only from PingCraft's own project:** no change to CraftCloud, but users get a second
  place to find their apps.
- **Bridges follow upstream's latest tag automatically:** protocol breaks get fixed sooner, at the cost of
  unreviewed changes to the programs that hold every message.

## Consequences

- When a network breaks the bridge, the fix waits for upstream's release and a pull request that bumps the
  version.
- Moving a server to a new name means new user IDs and an empty homeserver.
