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
            rust-bin.stable."1.85.0".default
            rust-analyzer
          ];

          packages = with pkgs; [
            tailwindcss_4
            bacon
            dockerfile-language-server-nodejs
            marksman
          ];

          # environment variable for running dev server.
          SECRET="AVERYSECRETSECRET";
        };
      }
    );
}
