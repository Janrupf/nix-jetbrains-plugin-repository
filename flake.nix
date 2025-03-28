{
  description = "Nix Jetbrains Plugin Repository development environment";

  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixos-unstable";
    };

    nix-rust-wrangler = {
      url = "github:Janrupf/nix-rust-wrangler";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils = {
      url = "github:numtide/flake-utils";
    };
  };

  outputs = {
    nixpkgs
  , flake-utils
  , rust-overlay
  , nix-rust-wrangler
  , ...
  }:
  let
    # We can re-use this across all nixpkgs instances
    rustOverlayInstance = (import rust-overlay);
  in (flake-utils.lib.eachDefaultSystem (system: let
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ rustOverlayInstance nix-rust-wrangler.overlays.default ];
    };

    indexer-lib = pkgs.callPackage ./nix/lib/default.nix {};

    nix-rust-wrangler-lib = nix-rust-wrangler.lib.${system};

    toolchainCollection = nix-rust-wrangler-lib.mkToolchainCollection [
      (nix-rust-wrangler-lib.deriveToolchainInstance (
        pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rust-analyzer" ];
        }
      ))
    ];
  in rec {
    devShells.default = pkgs.mkShell {
      NIX_RUST_WRANGLER_TOOLCHAIN_COLLECTION = toolchainCollection;

      buildInputs = [
        pkgs.openssl
        pkgs.pkg-config
        pkgs.stdenv.cc
        pkgs.valgrind
        pkgs.nix-rust-wrangler
      ];
    };

    plugins = indexer-lib.loadData ./data;

    packages.test-ide = pkgs.jetbrains.plugins.addPlugins pkgs.jetbrains.pycharm-professional [
      plugins."de.achimonline.github_markdown_emojis"
      plugins."ice.explosive.gdscript"
    ];
  }));
}
