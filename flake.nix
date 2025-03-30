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
  in ((flake-utils.lib.eachDefaultSystem (system: let
    pkgs = import nixpkgs {
      inherit system;
      overlays = [
        rustOverlayInstance
        nix-rust-wrangler.overlays.default
        (final: prev: {
          jb-repo-indexer = final.callPackage ./jb-repo-indexer/package.nix {};
        })
      ];
    };

    nix-rust-wrangler-lib = nix-rust-wrangler.lib.${system};

    toolchainCollection = nix-rust-wrangler-lib.mkToolchainCollection [
      (nix-rust-wrangler-lib.deriveToolchainInstance (
        pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rust-analyzer" ];
        }
      ))
    ];

    applied = pkgs.callPackage ./apply.nix {};
  in rec {
    devShells.default = pkgs.mkShell {
      NIX_RUST_WRANGLER_TOOLCHAIN_COLLECTION = toolchainCollection;
      NIX_RUST_WRANGLER_INSIDE_NIX_DEVELOP = "true";

      nativeBuildInputs = [
        pkgs.valgrind
        pkgs.nix-rust-wrangler
      ] ++ pkgs.jb-repo-indexer.nativeBuildInputs;

      buildInputs = pkgs.jb-repo-indexer.buildInputs;
    };

    legacyPackages = pkgs;

    packages.jb-repo-indexer = pkgs.jb-repo-indexer;
    packages.default = packages.jb-repo-indexer;

    apps.jb-repo-indexer = flake-utils.lib.mkApp {
      drv = packages.jb-repo-indexer;
    };
    apps.default = apps.jb-repo-indexer;

    plugins = applied.plugins;
  })) // rec {
    overlays.jetbrains-plugins = import ./overlay.nix;
    overlays.default = overlays.jetbrains-plugins;
  });
}
