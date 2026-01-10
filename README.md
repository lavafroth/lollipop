# Lollipop

Opinionated key remapper that brings sticky keys functionality like Android's
AOSP keyboard to Linux.

## Core Logic

For a modifier key `M`, the following table illustrates latching and locking.
On first run all keys are unlatched.

Initial State | Next `M` struck at | Sticky state
----|---|---
Unlatched | Whenever | Latched
Latched | < 500ms | Locked
Latched | >= 500ms | Unlatched
Locked | Whenever | Unlatched

## Getting Started

### Build

```sh
cargo build --release
```

Optionally place the binary in /usr/local/bin/

```sh
mkdir -p /usr/local/bin
cp ./target/release/lollipop /usr/local/bin/lollipop
```

### Run

```sh
sudo lollipop || sudo ./target/release/lollipop
```

## Systemd service

Coming soon
