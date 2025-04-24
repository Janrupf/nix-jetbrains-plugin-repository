{ lib
, pkgs
, rustPlatform
, pkg-config
, openssl
, ...
}: let
  cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
in rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    openssl
  ];

  src = ./.;

  useFetchCargoVendor = true;
  cargoHash = "sha256-HQoEEBcNoCqH+5U8U7BbRKxP5rBMfyTq1fnPBPXwXh0=";
}
