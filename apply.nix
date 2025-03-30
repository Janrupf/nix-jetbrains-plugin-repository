# Main entry point for the final output data
# needs to be called with callPackage
{ pkgs
, ... }:
let
  indexerLib = pkgs.callPackage ./nix/lib {};
  defaultFixup = pkgs.callPackage ./nix/fixup { inherit indexerLib; };
in {
  lib = indexerLib;
  plugins = indexerLib.loadData {
    dataRoot = ./data;
    fixup = defaultFixup;
  };
}