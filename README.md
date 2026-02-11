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

## Target Platforms

- Linux (Debian, Arch, Fedora) — x86_64, aarch64
- macOS — Apple Silicon, Intel
- Android (Termux) — aarch64
