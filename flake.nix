{
  description = "Budgeteur — personal finance web app and TUI client";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable."1.96.0".default;

        budgeteur = pkgs.rustPlatform.buildRustPackage {
          pname = "budgeteur";
          version = "0.31.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ rustToolchain ];

          # Build the TUI crate from the workspace root so path dependencies
          # (e.g., budgeteur_shared) resolve correctly.
          buildAndTestSubdir = ".";
          cargoBuildFlags = [ "-p" "budgeteur_tui" ];

          postInstall = ''
            mv $out/bin/budgeteur_tui $out/bin/budgeteur-tui
          '';

          meta = with pkgs.lib; {
            description = "Terminal client for Budgeteur";
            homepage = "https://github.com/AnthonyDickson/budgeteur-rs";
            license = licenses.mit;
            mainProgram = "budgeteur-tui";
          };
        };
      in
      {
        packages = {
          default = budgeteur;
          budgeteur-tui = budgeteur;
        };

        apps = {
          default = {
            type = "app";
            program = "${budgeteur}/bin/budgeteur-tui";
          };
          budgeteur-tui = {
            type = "app";
            program = "${budgeteur}/bin/budgeteur-tui";
          };
        };

        devShells.default = with pkgs; mkShell {
          buildInputs = [
            rust-bin.stable."1.96.0".default
            rust-analyzer
          ];

          packages = with pkgs; [
            bacon
            dockerfile-language-server
            tailwindcss-language-server
            tailwindcss_4
            codex
          ];

          # environment variable for running dev server.
          SECRET="AVERYSECRETSECRET";
        };
      }
    );
}
