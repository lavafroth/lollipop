# Lollipop

Keyboard modifier remapper that brings sticky keys functionality like Android's
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
- Release binary size is smaller 1MB.
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

## NixOS service

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

## Configuration

- `modifiers`: Comma separated list of modifier keys.
- `timeout`: The admissible delay between the taps of a double-tap for locking a key.
- `device`: The input device whose inputs get augmented. Generally this is some
`/dev/inputX` where X is a positive integer. Can also be set to the default
value `autodetect` which will automatically grab the first device that appears
as a keyboard. Note: this option is only available to specify a keyboard when
certain peripheral devices may get incorrectly reported as keyboards.
- `clear_all_with_escape`: The escape key clears all locked and latched states on all keys. Defaults to `true` or `yes`, set it to `no` or `false` to disable.
