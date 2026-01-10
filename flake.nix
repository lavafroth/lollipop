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

      devShells = forAllSystems (pkgs: {

        default = pkgs.mkShell {
          buildInputs = with pkgs; [
            stdenv.cc.cc.lib
          ];
        };

      });

      overlays.default = final: prev: {
        lollipop = self.packages.${final.system}.default;
      };
    };
}
