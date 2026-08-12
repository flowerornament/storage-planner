{
  description = "sp - Storage planning and purchase decision CLI";

  nixConfig = {
    extra-substituters = [ "https://flowerornament.cachix.org" ];
    extra-trusted-public-keys = [
      "flowerornament.cachix.org-1:gSODgIXgfRANrEGITBOF8XWaEKNy8hkNGfRVwqUG46c="
    ];
  };

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "aarch64-linux" "x86_64-linux" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f {
        pkgs = nixpkgs.legacyPackages.${system};
      });
    in
    {
      packages = forAllSystems ({ pkgs }: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "storage-planner";
          version = "1.0.2";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          meta = with pkgs.lib; {
            description = "CLI tool for evaluating storage and purchase decisions";
            license = licenses.mit;
            mainProgram = "sp";
          };
        };
      });
      apps = forAllSystems ({ pkgs }: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/sp";
        };
      });
    };
}
