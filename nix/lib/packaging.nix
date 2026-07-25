{ lib
, pkgs

, ... }:
rec {
  createSinglePluginPackage = data: selectedVersion: updateId: fixup:
  let
    updateData = data.updates.${builtins.toString updateId};
    fileName = updateData.file_name or "${data.name}-${selectedVersion}.jar";
  in fixup (pkgs.callPackage ({
    version ? selectedVersion,
    name ? "jetbrains-plugin-${data.xml_id}-${version}",
    sha256 ? updateData.sha256,
    downloadUrl ? updateData.download_url,
    unpack ? lib.strings.hasSuffix ".zip" fileName,
    pluginStdenv ? pkgs.stdenvNoCC,
    since ? updateData.since,
    until ? updateData.until,
    compatibilityOverrides ? updateData.compatibility_overrides,
    channel ? updateData.channel,
    dependencies ? updateData.dependencies or [],
  }: let
    passthru = {
      rawData = data;
      inherit channel;
      inherit updateId;
      inherit updateData;
      compatibility = {
        inherit since until compatibilityOverrides dependencies;
      };
    };

    rawPluginFile = pkgs.fetchurl {
      url = downloadUrl;
      name = fileName;
      inherit sha256 passthru;
    };
  in if !unpack
    # If we are not unpacking either way, no need to build a proxy
    # derivation - just return the fetched file.
    #
    # If anyone wants to fixup the derivation later, they can just
    # override with `unpack = true` and then apply their fixups.
    then rawPluginFile
    else pluginStdenv.mkDerivation {
      name = name;
      version = version;

      src = rawPluginFile;

      dontUnpack = !unpack;

      nativeBuildInputs = [
        # For unpacking
        pkgs.unzip
        pkgs.glibcLocalesUtf8
      ];

      inherit passthru;

      installPhase = ''
        runHook preInstall
        mkdir -p $out && cp -r . $out
        runHook postInstall
      '';
    }) {});

  createAllPluginPackages = data: fixup: let
    channels = lib.attrsets.mapAttrs (_: channelData:
      (lib.attrsets.mapAttrs (version: updateId:
        createSinglePluginPackage data version updateId fixup
      ) channelData) // {
        type = "versionset";
      }
    ) data.channels;

    versions = (lib.attrsets.mapAttrs (updateId: _:
      createSinglePluginPackage data (builtins.toString updateId) updateId fixup
    ) data.updates) // {
      type = "versionset";
    };
  in (channels.stable or { type = "versionset"; }) // channels // {
    all = versions;
    name = data.xml_id;
  };
}