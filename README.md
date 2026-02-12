# MatrixTUI

Multi-account terminal Matrix client. Simultaneous connections to multiple homeservers in a single responsive TUI.

## Install

### Prerequisites

**Arch:**
```
sudo pacman -S base-devel rust git
```

**Debian/Ubuntu:**
```
sudo apt install build-essential libssl-dev pkg-config git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Fedora:**
```
sudo dnf install gcc openssl-devel pkg-config git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**macOS:**
```
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Termux (Android):**
```
pkg install rust git binutils
```

> **Note:** Make sure `~/.cargo/bin` is in your PATH. If not, add it:
> - **bash/zsh:** `echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc`
> - **fish:** `fish_add_path ~/.cargo/bin`

### Install

```
git clone https://github.com/starlessoblivion/matrixtui.git
cd matrixtui
cargo install --path .
```

Then run `mtui` from anywhere.

### Install from GitHub (no clone)

```
cargo install --git https://github.com/starlessoblivion/matrixtui.git
```

### Update

```
cd matrixtui
git pull
cargo install --path .
```

## Usage

| Key | Action |
|-----|--------|
| `a` | Add an account |
| `s` | Settings / themes |
| `n` | New room |
| `e` | Edit active room |
| `f` | Toggle favorite |
| `Shift+Up/Down` | Reorder favorites |
| `Tab` / arrow keys | Navigate panels |
| `Enter` | Select room / send message |
| `Ctrl+K` | Quick room switcher |
| `?` | Help |
| `Ctrl+Q` | Quit |

## Config

Data is stored in `~/.config/matrixtui/`:
- `config.json` — accounts, theme, favorites, sort mode
- `sessions/` — per-account SQLite stores (E2EE keys, sync state)
- `matrixtui.log` — debug log

## Stack

- **Language:** Rust
- **TUI:** ratatui 0.29
- **Matrix:** matrix-rust-sdk 0.16 (E2EE, SQLite store)
- **Async:** tokio

## Security

### Login credentials

Your **password** is sent directly to your homeserver over HTTPS and is **never written to disk**. It is held in memory only during the login request and cleared immediately after. MatrixTUI does not store, log, or transmit your password anywhere else.

On successful login the homeserver returns a **session access token**. This token is saved in plaintext in `~/.config/matrixtui/config.json` so the client can restore your session without re-entering your password. The access token grants full account access until revoked. **Protect this file** — anyone who can read it can act as your account. You can revoke a session token from another Matrix client (Element: Settings > Sessions) or by removing the account in MatrixTUI settings, which deletes the token from the config.

### End-to-end encryption (E2EE)

All encrypted rooms use the Matrix E2EE protocol (Olm/Megolm) via [matrix-rust-sdk](https://github.com/nickel-org/matrix-rust-sdk). Encryption keys, cross-signing keys, and sync state are stored in **unencrypted SQLite databases** under `~/.local/share/matrixtui/sessions/<account>/`. These files contain the cryptographic material needed to decrypt your message history. **Protect this directory** — if an attacker copies these files they can decrypt messages from your sessions.

Messages are decrypted in memory for display and are **never cached to disk** by MatrixTUI. When you close the client, decrypted message content only persists on the homeserver (encrypted) and in the SQLite key store (keys only, not message content).

### Session verification (recovery key)

New sessions cannot decrypt message history until verified. MatrixTUI supports verification by **recovery key** — the key starting with `Es` that you saved when setting up cross-signing (typically in Element). When you enter your recovery key in Settings > Verify Session, it is used once to call the Matrix recovery API and is **immediately discarded** — it is never saved to disk or logged.

### What to protect

| Path | Contains | Risk if leaked |
|------|----------|----------------|
| `~/.config/matrixtui/config.json` | Access tokens, account metadata | Full account access |
| `~/.local/share/matrixtui/sessions/` | E2EE keys, sync state (SQLite) | Decrypt message history |

Recommended: set restrictive permissions on both directories (`chmod 700`). If you use full-disk encryption, these files are protected at rest. If not, consider that anyone with local access to your machine can read them.

### What is NOT stored

- Passwords (only used during initial login API call)
- Recovery keys (used once, then discarded)
- Decrypted message content (only held in memory)
- Your homeserver password in any log file

## Target Platforms

- Linux (Debian, Arch, Fedora) — x86_64, aarch64
- macOS — Apple Silicon, Intel
- Android (Termux) — aarch64
