{ lib
, pkgs
, ... }:
let
  versionLib = pkgs.callPackage ./version.nix {};
  packaging = pkgs.callPackage ./packaging.nix {};
  fixup = pkgs.callPackage ./fixup.nix {};
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
  loadData = { dataRoot, fixup }@args: let
    indexFile = /${dataRoot}/index.json;
    index = builtins.fromJSON (builtins.readFile indexFile);

    allPlugins = lib.attrsets.mapAttrs (_: hash: let
      # Split the hash into aa/bb/cc[...]
      hashFirst = builtins.substring 0 2 hash;
      hashSecond = builtins.substring 2 2 hash;
      hashRest = builtins.substring 4 ((builtins.stringLength hash) - 4) hash;

      pluginPath = /${dataRoot}/${hashFirst}/${hashSecond}/${hashRest}/metadata.json;
    in packaging.createAllPluginPackages (loadPlugin pluginPath) fixup) index;
  in
    (expandAttrNames allPlugins) // {
      raw = allPlugins;

      # This works differently than overriding in nixpkgs because we have multiple
      # versions of a single package. Each fixup function can decide whether it wants
      # to apply the previous fixup or not for a certain package.
      override = newFixup: overrideLoadData args (newFixup fixup);
    };

  # Call loadData with the original arguments but with a new fixup function
  overrideLoadData = args: newFixup: loadData (args // {
    fixup = newFixup;
  });

  # Expand attributes like "a.b.c" = value to { a = { b = { c = value; }; }; }
  expandAttrNames = set: let
    mapToKeyValuePair = key: value: let
      path = lib.strings.splitString "." key;
    in lib.attrsets.setAttrByPath path value;
  in
    mergeAttrsListRecursive (lib.attrsets.mapAttrsToList mapToKeyValuePair set);

  # Check wether a plugin is compatible with a given IDE build
  isCompatibleWith = build: plugin: let
    buildNumber = if build ? buildNumber then build.buildNumber else build;
    compatData = plugin.compatibility or (throw (lib.trace plugin.type "Missing compatibility information"));

    isInValidRange = versionLib.inRange buildNumber compatData.since compatData.until;
    isInWorkingCondition = lib.lists.foldl (state: compatOverride: state && (
      if versionLib.inRange buildNumber compatOverride.applies_since compatOverride.applies_until
        then versionLib.inRange buildNumber compatOverride.compatible_since compatOverride.compatible_until
        else true
    )) true compatData.compatibilityOverrides; 
  in isInValidRange && isInWorkingCondition;

  # Select the latest compatible version of the version set, or null, if
  # no compatible version is found
  selectLatestCompatibleVersion = ideBuildNumber: pluginVersionSet: (lib.attrsets.foldlAttrs
    ({ selectedVersion, selectedPkg }@selected: version: pkg:
      if (pkg ? type && lib.isDerivation pkg) && isCompatibleWith ideBuildNumber pkg
        then if selectedVersion == null 
          then { selectedVersion = version; selectedPkg = pkg; } # No compatible plugin selected yet, take anything we can
        else if lib.strings.versionAtLeast version selectedVersion 
          then { selectedVersion = version; selectedPkg = pkg; } # Higher version
        else selected # Older
      else selected # Not compatible anyway
    )
    { selectedVersion = null; selectedPkg = null; }
    pluginVersionSet
  ).selectedPkg;

  # Build an IDE package with the given plugins, potentially automatically
  # selecting the latest compatible versions.
  buildIdeWithPlugins = (pkgs.callPackage ({
    lib,
    jetbrains,
  }: ide: plugins: let
    idePkg = if builtins.typeOf ide == "set" && lib.isDerivation ide
      then ide
      else jetbrains.${ide};

    ideBuildNumber = idePkg.buildNumber;

    pluginPkgs = map (plugin: if plugin ? type && plugin.type == "versionset"
      then let
          selectedPlugin = selectLatestCompatibleVersion ideBuildNumber plugin;
        in if selectedPlugin == null
          then throw "no compatible plugin ${plugin.pname} found for IDE build ${ideBuildNumber}"
          else selectedPlugin
      else plugin) plugins;

  in jetbrains.plugins.addPlugins idePkg pluginPkgs) {});

  version = versionLib;
} // fixup
