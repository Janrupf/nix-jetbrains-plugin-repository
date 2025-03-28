{ lib
, pkgs
, ... }:
let
  packaging = pkgs.callPackage ./packaging.nix {};
in rec {
  mergeAttrsListRecursive = list:
    let
      # `binaryMerge start end` merges the elements at indices `index` of `list` such that `start <= index < end`
      # Type: Int -> Int -> Attrs
      binaryMerge = start: end:
        # assert start < end; # Invariant
        if end - start >= 2 then
          # If there's at least 2 elements, split the range in two, recurse on each part and merge the result
          # The invariant is satisfied because each half will have at least 1 element
          lib.attrsets.recursiveUpdate (binaryMerge start (start + (end - start) / 2))
            (binaryMerge (start + (end - start) / 2) end)
        else
          # Otherwise there will be exactly 1 element due to the invariant, in which case we just return it directly
          builtins.elemAt list start;
    in
    if list == [ ] then
      # Calling binaryMerge as below would not satisfy its invariant
      { }
    else
      binaryMerge 0 (builtins.length list);

  loadPlugin = metadata: let
    plugin = builtins.fromJSON (builtins.readFile metadata);
  in
    plugin;

  # Load the data from the dataRoot directory
  loadData = dataRoot: let
    indexFile = /${dataRoot}/index.json;
    index = builtins.fromJSON (builtins.readFile indexFile);
  in
    lib.attrsets.mapAttrs (_: hash: let
      # Split the hash into aa/bb/cc[...]
      hashFirst = builtins.substring 0 2 hash;
      hashSecond = builtins.substring 2 2 hash;
      hashRest = builtins.substring 4 ((builtins.stringLength hash) - 4) hash;

      pluginPath = /${dataRoot}/${hashFirst}/${hashSecond}/${hashRest}/metadata.json;
    in packaging.createAllPluginPackages (loadPlugin pluginPath)) index;

  # Expand attributes like "a.b.c" = value to { a = { b = { c = value; }; }; }
  expandAttrNames = set: let
    mapToKeyValuePair = key: value: let
      path = lib.strings.splitString "." key;
    in lib.attrsets.setAttrByPath path value;
  in
    mergeAttrsListRecursive (lib.attrsets.mapAttrsToList mapToKeyValuePair set);
}
