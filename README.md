# Lollipop

Opinionated key remapper that brings sticky keys functionality like Android's
AOSP keyboard to Linux.


## Behavior

For a modifier key `M`, the following table illustrates latching and locking.
On first run all keys are unlatched.

Initial State | Next `M` struck at | Sticky state
----|---|---
Unlatched | Whenever | Latched
Latched | < 500ms | Locked
Latched | >= 500ms | Unlatched
Locked | Whenever | Unlatched

## Features
- Ridiculously fast.
- Release binary size is smaller than an average wallpaper.
- Simple `ini` config file with example provided in the repo.
- Indicates latched/locked state by switching on the Caps Lock LED.

## Getting Started

### Build

```sh
cargo build --release
```
### Install

```sh
install -o root -g root {./target/release,/usr/bin}/lollipop
cp ./systemd/lollipop.service /etc/systemd/system/lollipop.service
systemctl daemon-reload
systemctl enable --now lollipop
```

## SystemD service for NixOS

Add the input to your flake

```nix
{
  inputs.nixos-cosmic.url = "github:lavafroth/lollipop";

  outputs = { self, nixpkgs, lollipop, ... }: {
    nixosConfigurations = {
      yourMachine = nixpkgs.lib.nixosSystem {
        modules = [
          lollipop.nixosModules.default
          ./configuration.nix
        ];
      };
    };
  };
}
```

Enable the service in your `configuration.nix` file.

```nix
services.lollipop.enable = true;
```
