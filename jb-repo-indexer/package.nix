{ lib
, pkgs
, rustPlatform
, pkg-config
, openssl
, ...
}: rustPlatform.buildRustPackage rec {
  pname = "jb-repo-indexer";
  version = "0.1.0";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    openssl
  ];

  src = ./.;

  useFetchCargoVendor = true;
  cargoHash = "sha256-kWGEO3ez2BJXJxfgMPorTwIo73sM/eG2R7t2w6bHfs0=";
}
