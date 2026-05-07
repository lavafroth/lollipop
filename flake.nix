{
  description = "flake for github:lavafroth/lollipop";

  outputs =
    {
      self,
      nixpkgs,
      ...
    }:
    let
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs nixpkgs.lib.systems.flakeExposed (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "lollipop";
          version = "1.0.0";

          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };
      });

      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.services.lollipop;
        in
        {
          options.services.lollipop = {
            enable = lib.mkEnableOption "Lollipop sticky keys service";
            timeout = lib.mkOption {
              type = lib.types.int;
              description = "lock a key if pressed twice within this time window";
              default = 500;
            };
            modifiers = lib.mkOption {
              type = lib.types.str;
              description = "the modifiers to register and augment";
              default = "leftshift,leftctrl,compose,leftmeta,fn";
            };
            device = lib.mkOption {
              type = lib.types.str;
              description = "the keyboard device to listen on";
              default = "autodetect";
            };
            clearAllWithEscape = lib.mkOption {
              type = lib.types.bool;
              description = "clear all latched and locked keys by pressing escape";
              default = true;
            };
            touchpad.enable = lib.mkOption {
              type = lib.types.bool;
              description = "clear latched and locked keys when touchpad is clicked";
              default = true;
            };
            touchpad.timeout = lib.mkOption {
              type = lib.types.int;
              description = "how long a touchpad can dwell in the touched state before considering the input as a click";
              default = 400;
            };
          };

          config = lib.mkIf cfg.enable {

            systemd.services.lollipop = {
              description = "Lollipop sticky keys service";
              wantedBy = [ "multi-user.target" ];
              serviceConfig = {

                ExecStart = "${self.packages.${pkgs.system}.default}/bin/lollipop ${
                  let configContents = lib.generators.toINIWithGlobalSection { } {

                    globalSection = {
                      timeout = cfg.timeout;
                      modifiers = cfg.modifiers;
                      device = cfg.device;
                      clear_all_with_escape = cfg.clearAllWithEscape;
                    };

                      sections = {
                        touchpad = cfg.touchpad;
                      };

                  }; in
                  pkgs.writeText "config.ini" configContents
                }";

                Type = "exec";
              };
            };
          };
        };

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          buildInputs = with pkgs; [
            stdenv.cc.cc.lib
            rust-analyzer
            cargo
            rustc
          ];
        };

      });

      overlays.default = final: prev: {
        lollipop = self.packages.${final.system}.default;
      };
    };
}
