# MatrixTUI

Multi-account terminal Matrix client. Simultaneous connections to multiple servers in a single responsive TUI.

## Install

Install [Rust](https://rustup.rs), then:

```
cargo install --git https://github.com/marshallumsted/matrixtui.git
```

Run with `mtui`. To update, run the same command again.

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
| `Enter` | Select room / send message / space home |
| `Space` | Collapse / expand space |
| `i` | Room info (members, topic, encryption) |
| `r` | Reply to selected message |
| `e` | React to selected message |
| `Ctrl+K` | Quick room switcher |
| `Ctrl+U` | Upload / attach file |
| `? / h` | Help |
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

Your **password** is sent directly to your selected server over HTTPS and is **never written to the local device disk**. It is held in memory only during the login request and cleared immediately after. MatrixTUI does not store, log, or transmit your password anywhere else.

On successful login the server returns a **session access token**. This token is saved in plaintext in `~/.config/matrixtui/config.json` so the client can restore your session without re-entering your password. The access token grants full account access until revoked. **Protect this file** — anyone who can read it can act as your account. You can revoke a session token from another Matrix client (Element: Settings > Sessions) or by removing the account in MatrixTUI settings, which deletes the token from the config.

### End-to-end encryption (E2EE)

All encrypted rooms use the Matrix E2EE protocol (Olm/Megolm) via [matrix-rust-sdk](https://github.com/matrix-org/matrix-rust-sdk). Encryption keys, cross-signing keys, and sync state are stored in **unencrypted SQLite databases** under `~/.local/share/matrixtui/sessions/<account>/`. These files contain the cryptographic material needed to decrypt your message history. **Protect this directory** — if an attacker copies these files they can decrypt messages from your sessions.

Messages are decrypted in memory for display and are **never cached to disk** by MatrixTUI. When you close the client, decrypted message content only persists on the server (encrypted) and in the SQLite key store (keys only, not message content).

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
- Your server password in any log file

## Matrix Standard Feature Support

| Feature | Status |
|---------|--------|
| Multi-account simultaneous login | Supported |
| Password login | Supported |
| Session token persistence | Supported |
| Send / receive text messages | Supported |
| End-to-end encryption (Olm/Megolm) | Supported |
| Session verification (recovery key) | Supported |
| Session verification (SAS emoji) | Supported |
| Room key backup download | Supported (automatic on decrypt failure) |
| Message history (backward pagination) | Supported (scroll to load more) |
| Read receipts | Supported (sent on room open / new messages) |
| Typing indicators | Supported (send and receive) |
| Unread message count | Supported |
| Reply to messages | Supported (`r` key) |
| Reactions (emoji) | Supported (`e` key, 8 quick-pick emojis) |
| Edit messages | Supported (via message action menu) |
| Delete / redact messages | Supported (via message action menu) |
| Create rooms (public/private/encrypted) | Supported |
| Edit room name / topic | Supported |
| Invite users | Supported |
| Leave rooms | Supported |
| Room info (topic, members, encryption) | Supported (`i` in chat) |
| Favorites / room pinning | Supported (`f` key, manual reorder) |
| Profile editing (display name, avatar) | Supported |
| Fuzzy room search | Supported (`Ctrl+K`) |
| Responsive layout (3/2/1 column) | Supported |
| Inline image viewing | Supported (Sixel/Kitty/halfblock, async download) |
| File / video / audio messages | Supported (display + download via action menu) |
| File upload / attachment | Supported (`Ctrl+U`, native file picker) |
| Drag-and-drop file send | Supported (bracketed paste detection, confirm overlay) |
| Media download | Supported (saves to ~/Downloads via action menu) |
| Clickable media links | Supported (OSC 8 terminal hyperlinks, unencrypted rooms) |
| User presence | Supported (online/away/offline dots in chat and room info) |
| Push notifications (Termux) | Supported (auto-detect, `termux-notification` for background messages) |
| Member list | Supported (browsable in room info overlay, `i` key) |
| Spaces | Supported (hierarchy in room list, collapse/expand, space home) |
| Paste in all fields | Supported (login, recovery, room switcher, profile, etc.) |
| E2EE room lock indicator | Supported (🔒 prefix in room list) |
| Unverified session warning | Supported (status bar prompt on encrypted room entry) |
| Loading indicator | Supported (animated spinner while fetching messages) |
| Smooth message scrolling | Supported (per-message scroll, partial top-message clipping) |
| Reset All | Supported (wipe all data, logout, restore to fresh-install state) |

## To Be Implemented

| Feature | Notes |
|---------|-------|
| Threads | Matrix threading support |
| Message search | Search within room or across rooms |
| Room directory | Browse and join public rooms |
| Command mode | `/join`, `/leave`, `/invite`, `/topic`, etc. |
| Per-account notification rules | Mute rooms, keyword alerts |
| User profiles | View other users' profiles |
| Message formatting | Markdown rendering, code blocks |
| URL previews | Inline link previews |
| Voice / video calls | Matrix VoIP support |
| Package distribution | `cargo install` / AUR / brew |

## Target Platforms

- Linux (Debian, Arch, Fedora) — x86_64, aarch64
- macOS — Apple Silicon, Intel
- Android (Termux) — aarch64
