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

## Configuration Options

Lollipop is configured with a simple `ini` file with `key=value` pair syntax.
Being an opinionated tool, all configuration settings are optional.
Check out the example [config file](./config.ini) which shows the use of all the
config options.

### `modifiers`

A comma-separated list of modifier keys to enable.

Example: `modifiers=leftshift,leftctrl,compose`  
Default: `modifiers=leftshift,leftctrl,compose,leftmeta,fn`

### `timeout`

The admissible delay in milliseconds between the taps of a double-tap for locking a key.

Example: `timeout=1000`  
Default: `timeout=500`

### `device`

Specifies the input device to augment. This could bee set to a `/dev/inputX` device, where X is a positive integer.

Example: `device=/dev/input0`  
Default:`device=autodetect`

The default `autodetect` automatically picks the first keyboard device.
*Note:*  Using `autodetect` can sometimes incorrectly identify peripheral devices as keyboards.

> [!NOTE]
> this option is only available to specify a keyboard when certain peripheral
devices may get incorrectly reported as keyboards.

### `clear_all_with_escape`

When set to `true` or `yes`, pressing the escape key clears all latched and locked keys.

Example: `clear_all_with_escape=no`  
Default:`clear_all_with_escape=true`

Possible values: `true`, `yes`, `no`, `false`

