{ indexerLib
, pkgs
, ... }:
indexerLib.mkFixupMatcher {
  "com.github.copilot" = pkgs.callPackage ./com.github.copilot.nix {};
  "com.jetbrains.rust" = pkgs.callPackage ./com.jetbrains.rust.nix {};
}
