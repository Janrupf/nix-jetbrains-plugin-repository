final: prev: let
  applied = final.callPackage ./apply.nix {};
in {
  jetbrains-plugins = applied.plugins // {
    inherit (applied) lib;
  };
}
