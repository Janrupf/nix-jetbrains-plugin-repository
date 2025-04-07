{ lib
, ...
}: rec {
  parse = build: builtins.addErrorContext "while parsing Jetbrains version ${versionToString build}" (
  let
    components = let
      minusSplit = lib.strings.splitString "-" build;
    in if builtins.length minusSplit > 1
      then {
        product = builtins.elemAt minusSplit 0;
        spec = lib.strings.splitString "." (lib.strings.concatStringsSep "-" (lib.lists.drop 1 minusSplit));
      }
      else { product = null; spec = lib.strings.splitString "." build; };
    
    spec = map (part: if part == "*"
      then { type = "wildcard"; }
      else { type = "number"; value = lib.strings.toIntBase10 part; }
    ) components.spec;
  in if build ? type && build.type == "jetbrains-version" then build
    else if build == null then null
    else {
      inherit (components) product;
      inherit spec;
      type = "jetbrains-version";
    });

  inRange = version: since: until: builtins.addErrorContext
    "while checking if Jetbrains version ${versionToString version} ^ in range ${versionToString since} .. ${versionToString until}"
    ((since == null || (versionCompare version since) >= 0) &&
      (until == null || (versionCompare version until) < 1));

  versionToString = version: if version ? type && version.type == "jetbrains-version"
    then let
      productPrefix = if version.product != null then "${version.product}-" else "";
      specString = lib.strings.concatStringsSep "." (map (part: if part.type == "wildcard"
        then "*"
        else builtins.toString part.value
      ) version.spec);
    in "${productPrefix}${specString}"
    else if version == null then "<<null>>"
    else if builtins.typeOf version == "string" then version
    else "<<invalid version ${builtins.typeOf version}>>";

  versionCompare = a: b: builtins.addErrorContext
    "while comparing Jetbrains version ${versionToString a} with ${versionToString b}"
  (let
    parsedA = parse a;
    parsedB = parse b;

    lenA = builtins.length parsedA.spec;
    lenB = builtins.length parsedB.spec;
    compareLen = if lenA > lenB then lenA else lenB;

    compareIndices = lib.lists.range 0 (compareLen - 1);

    zeroComp = { type = "number"; value = 0; };
    decision = lib.lists.foldl (state: idx: let
        elemA = if idx < lenA then builtins.elemAt parsedA.spec idx else zeroComp;
        elemB = if idx < lenB then builtins.elemAt parsedB.spec idx else zeroComp;
      in if state != null
        then state # Decision made already due to earlier component
        else if elemA.type == "wildcard" || elemB.type == "wildcard" then 0
        else if elemA.value > elemB.value then 1
        else if elemA.value < elemB.value then -1
        else null
    ) null compareIndices;
  in if decision == null then 0 else decision);
}
