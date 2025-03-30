{ ... }:
{
  mkFixupMatcher = matchAttrs: pluginPkg:
  let
    hasMatchingFixup = builtins.hasAttr pluginPkg.rawData.xml_id matchAttrs;
  in if hasMatchingFixup then
    # Apply the fixup if it matches the plugin
    matchAttrs.${pluginPkg.rawData.xml_id} pluginPkg
  else
    # Otherwise just return the original package
    pluginPkg;
}
