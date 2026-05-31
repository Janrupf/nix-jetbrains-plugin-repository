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

  cargoHash = "sha256-UBpZ1kn3qlGx62Bem+2knGV7t6b6ewFwcWwYMSaM9d4=";
}
