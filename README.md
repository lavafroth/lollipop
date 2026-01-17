# Lollipop

Modifier key remapper bringing Android's AOSP keyboard sticky keys to Linux.

## Behavior

- Single tap a modifier to latch
- If the next tap is within 500ms the modifier is locked
- If 500ms is elapsed, the next tap unlatches the key
- Single tap in locked state unlocks the key

The 500ms delay is configurable.

## Features
- Ridiculously fast.
- Release binary size is smaller than 1MB.
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
  inputs.lollipop.url = "github:lavafroth/lollipop";

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

Lollipop is configured with a simple `ini` file with `key=value` pair syntax.
Check out the example [config file](./config.ini) which shows the use of all the
config options.

### `modifiers`

Comma separated list of modifier keys.


### `timeout`

The admissible delay between the taps of a double-tap for locking a key.

### `device`

The input device whose inputs get augmented.

Generally this is some `/dev/inputX` where X is a positive integer. Can also be
set to the default value `autodetect` which will automatically grab the first
device that appears as a keyboard.

> [!NOTE]
> this option is only available to specify a keyboard when certain peripheral
devices may get incorrectly reported as keyboards.

### `clear_all_with_escape`

The escape key clears all locked and latched states on all keys.

Defaults to `true` or `yes`, set it to `no` or `false` to disable.

