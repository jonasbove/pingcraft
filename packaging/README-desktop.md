# PingCraft desktop

PingCraft puts all your chats in one inbox, on a Matrix server you run yourself
(<https://github.com/jonasbove/pingcraft>). Sign in with your server's address, your user name and your
password. The first time, PingCraft shows you a recovery key: keep it somewhere safe, away from this computer.
It is what lets a new device read your encrypted messages.

## Which file

| System | File | Install |
|---|---|---|
| Debian, Ubuntu, Mint | `pingcraft-<version>-linux-<arch>.deb` | `sudo apt install ./pingcraft-….deb` |
| Fedora, openSUSE | `pingcraft-<version>-linux-<arch>.rpm` | `sudo dnf install ./pingcraft-….rpm` |
| Arch Linux | `pingcraft-<version>-…pkg.tar.zst` | `sudo pacman -U ./pingcraft-….pkg.tar.zst` |
| Any Linux | `pingcraft-<version>-linux-<arch>.AppImage` | make it executable and run it |
| Any Linux, sandboxed | `pingcraft-<version>-linux-<arch>.flatpak` | `flatpak install --user ./pingcraft-….flatpak` |
| Any Linux, no root | `pingcraft-<version>-linux-<arch>.tar.gz` | unpack it and run `./install.sh` |
| Windows | `pingcraft-<version>-windows-<arch>.msi` | run it (no administrator rights needed) |
| Windows, portable | `pingcraft-<version>-windows-<arch>-portable.zip` | unpack it and run `pingcraft.exe` |
| macOS | `pingcraft-<version>-macos-universal.dmg` | open it and drag PingCraft to Applications |
| FreeBSD | `pingcraft-<version>-freebsd-<arch>.tar.gz` | `tar -xzf … --strip-components 1 -C /usr/local` |

## Where PingCraft keeps things

- **Your messages** stay on your server. PingCraft keeps an encrypted copy on this computer, in its data
  folder (`~/.local/share/pingcraft` on Linux, `%LOCALAPPDATA%\pingcraft` on Windows,
  `~/Library/Application Support/pingcraft` on macOS).
- **The key to that copy and your sign-in** go to the system keychain (Keychain on macOS, Credential Manager on
  Windows, the Secret Service on Linux and FreeBSD: GNOME Keyring, KWallet or KeePassXC). Where there is no
  keychain, PingCraft asks for a passphrase of your choosing each time it starts, and keeps them in a file locked
  with it.

## SQLite

On Linux, FreeBSD and macOS PingCraft uses the system's SQLite; it should be 3.54.0 or newer
(RUSTSEC-2026-0459). The Windows build carries its own SQLite 3.54.1.
