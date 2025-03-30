{ indexerLib
, pkgs
, ... }:
indexerLib.mkFixupMatcher {
  "com.github.copilot" = pkgs.callPackage ./com.github.copilot.nix {};
}
