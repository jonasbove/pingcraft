# ADR 0004: Encryption, and where plaintext lives

- **Status:** accepted by the maintainer (2026-11-24)

## Context

Losing or leaking messages is the worst bug PingCraft can have. End-to-end encryption on Matrix (Olm and Megolm,
through `matrix-sdk-crypto` and `vodozemac`, both Apache-2.0) protects messages between Matrix devices, but a
bridged message is not end-to-end encrypted from the other person to the user:

- the **remote network** (WhatsApp, Signal, ...) protects the message only as far as the bridge;
- the **bridge** decrypts it, holds it in plaintext, and re-encrypts it for Matrix;
- the **homeserver** sees only ciphertext, if the bridge encrypts on the Matrix side;
- the **client** decrypts it and keeps it in its local store.

`matrix-sdk` stores encrypt values, not whole files: the SQLite store on desktop (`matrix-sdk-sqlite`) and the
IndexedDB store on the web (`matrix-sdk-indexeddb`) encrypt each value with XChaCha20-Poly1305 under a key or
passphrase the application supplies. Browsers have no equivalent of an OS keychain.

## Decision

1. **Bridges** encrypt on the Matrix side and require it (`encryption.allow`, `default` and `require` on), with
   `pickle_key: generate` so the bridge's own Matrix keys are encrypted in its database. Bridges use the default
   sync-based encryption, not the sync-less appservice mode.
2. **Desktop client:** every store is encrypted with a random 32-byte key kept in the OS keychain (the `keyring`
   crate, MIT OR Apache-2.0). If no keychain is available, PingCraft asks for a passphrase instead of storing the
   key in a file.
3. **Web client:** the IndexedDB store is encrypted with a passphrase the user types to unlock PingCraft. Nothing
   secret stays in browser storage between visits.
4. **First sign-in** sets up cross-signing, key backup and a recovery key, and asks the user to save the recovery
   key. Losing every device and the recovery key loses encrypted history; PingCraft says so in plain words.
5. **Server volumes and backups** contain readable messages and the bridges' logins to each network. The
   deployment docs say so and treat them as secrets.
6. Access tokens, recovery keys, store keys and bridge logins are never logged, never committed and never sent
   anywhere but the server they belong to.

## Alternatives

- **Web keeps nothing:** every visit is a new device restored from the recovery key. Safer on shared computers,
  but slower and it fills the device list.
- **Web keeps its store key in browser storage:** no passphrase to type, but anyone with the browser profile can
  read the messages.
- **No Matrix-side encryption for bridges:** simpler, but the homeserver's database would then hold plaintext too.

## Consequences

- The bridge is the place where plaintext lives on the server. Hardening, backups and access to bridge
  containers matter as much as the homeserver's.
- The web build needs an unlock screen, and the desktop build depends on a working keychain on each platform.
- craft-libs has no secrets crate; keychain access is written in PingCraft for now and proposed to craft-libs
  later (house rule 16).
