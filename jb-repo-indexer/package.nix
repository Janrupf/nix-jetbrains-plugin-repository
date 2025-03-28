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
  cargoHash = "sha256-q15Itz2lc3c55J441GGGCtpxmgK222HTFRPiAbf0v3M=";
}
