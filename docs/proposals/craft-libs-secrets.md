# Proposal: a `secrets` crate in craft-libs

- **Status:** draft, not opened anywhere. House rule 16 asks for reusable code to go to `storytold/craft-libs`;
  this proposal is for the maintainer to take there, or not. Nothing is opened on GitHub without the maintainer.
- **Source:** `crates/keychain` in PingCraft (ADR 0004).

## What it would hold

Somewhere for a desktop app to keep a few small secrets (keys, tokens) that never touch a plain file:

| Piece | What it does |
|---|---|
| `SecretStore` trait | `get`, `set`, `delete` by name; secrets come back in `Zeroizing<Vec<u8>>` |
| `OsKeychain` | the OS keychain through `keyring` (macOS Keychain, Windows Credential Manager, the Secret Service on Linux and the BSDs), filed under the app's service name; `probe()` says whether it works without changing anything |
| `PassphraseVault` | the fallback when there is no keychain: one file, XChaCha20-Poly1305 under a key derived from a passphrase with Argon2id (64 MiB, 3 passes by default; the parameters live in the authenticated header and are bounded when read). A wrong passphrase is its own error and changes nothing; writes are atomic and the file is `0600` |
| `MemoryStore` | for tests |
| `store_key(store, name)` | a random 32-byte key, created once and reused, with a redacted `Debug` |

Dependencies, all on the Craft allowlist: `keyring` (MIT OR Apache-2.0), `argon2` and `chacha20poly1305`
(RustCrypto, MIT OR Apache-2.0), `getrandom`, `zeroize`.

## What stays in PingCraft

- The names of PingCraft's secrets (`store-key:<profile>`, `session:<profile>`), its service name and its profile
  folders.
- Choosing between the keychain and the vault, and the unlock screen.
- The web build's passphrase handling, which goes through `matrix-sdk`'s own IndexedDB store encryption.

## Open questions for the maintainer

- The service name: one per app (`PingCraft`), as now, or a shared `Craft` prefix?
- Whether other Craft apps want the vault at all, or only the keychain.
- Flatpak: the Secret Service needs `--talk-name=org.freedesktop.secrets`; the Secret portal was not tried
  (ADR 0009's addendum).
