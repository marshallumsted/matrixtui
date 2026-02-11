# MatrixTUI

Multi-account terminal Matrix client. Simultaneous connections to multiple homeservers in a single responsive TUI.

**Design doc:** [`brain/Design Concepts/matrixtui.md`](../../Design%20Concepts/matrixtui.md)

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
source ~/.cargo/env
```

**Fedora:**
```
sudo dnf install gcc openssl-devel pkg-config git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

**macOS:**
```
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

**Termux (Android):**
```
pkg install rust git binutils
```

### Build & Run

```
git clone https://github.com/starlessoblivion/matrixtui.git
cd matrixtui
cargo build --release
./target/release/matrixtui
```

### Update

```
cd matrixtui
git pull
cargo build --release
```

## Usage

- `a` — add an account
- `Tab` / arrow keys — navigate panels
- `Enter` — select room / send message
- `Ctrl+K` — quick room switcher
- `?` — help
- `Ctrl+Q` — quit

## Stack

- **Language:** Rust
- **TUI:** ratatui
- **Matrix:** matrix-rust-sdk 0.16
- **Async:** tokio

## Target Platforms

- Linux (Debian, Arch, Fedora) — x86_64, aarch64
- macOS — Apple Silicon, Intel
- Android (Termux) — aarch64
