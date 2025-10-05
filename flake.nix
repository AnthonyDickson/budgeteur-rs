{
  description = "Rust development environment";

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
      in
      {
        devShells.default = with pkgs; mkShell {
          buildInputs = [
            rust-bin.stable."1.89.0".default
            rust-analyzer
          ];

          packages = with pkgs; [
            bacon
            dockerfile-language-server
            tailwindcss
            gemini-cli
          ];

          # environment variable for running dev server.
          SECRET="AVERYSECRETSECRET";
        };
      }
    );
}
