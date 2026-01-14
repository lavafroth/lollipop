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

      nixosModules.default = {config, lib, pkgs, ...}:
        let cfg = config.services.lollipop;
        in {
        options.services.lollipop = {
          enable = lib.mkEnableOption "Lollipop sticky keys service";
        };

        config = lib.mkIf cfg.enable {
          systemd.services.lollipop = {
            description = "Lollipop sticky keys service";
            wantedBy = [ "multi-user.target" ];
            serviceConfig = {
              ExecStart = "${self.packages.${pkgs.system}.default}/bin/lollipop";
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
