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

  cargoHash = "sha256-erSanGBTBHhcZYh0XswrzoQufINWRutOJbTa/h08yZ0=";
}
