{ lib
, pkgs
, runCommandLocal

, glibcLocalesUtf8

, ... }:
rec {
  # Derived from https://github.com/NixOS/nixpkgs/blob/master/pkgs/build-support/fetchzip/default.nix
  unpackPlugin = file: fileName: runCommandLocal "extract-intellij-plugin" {
    input = file;
    nativeBuildInputs = [ pkgs.unzip glibcLocalesUtf8 ];
  } ''
    unpackDir="$TMPDIR/unpack"
    mkdir "$unpackDir"

    renamedInput="$TMPDIR/${lib.strings.escapeShellArg fileName}"
    cp "$input" "$renamedInput"

    cd "$unpackDir"

    unpackFile "$renamedInput"
    chmod -R +w "$unpackDir"

    fn=$(cd "$unpackDir" && ls -A)
    if [ -f "$unpackDir/$fn" ]; then
      mkdir $out
    fi
    mv "$unpackDir/$fn" "$out"

    chmod 755 "$out"
  '';

  maybeUnpackPlugin = doUnpack: file: fileName: if doUnpack then unpackPlugin file fileName else file;

  createSinglePluginPackage = data: selectedVersion: updateId: fixup:
  let
    updateData = data.updates.${builtins.toString updateId};
    fileName = updateData.file_name or "${data.name}-${selectedVersion}.jar";
  in fixup (pkgs.callPackage ({
    name ? "jetbrains-plugin-${data.xml_id}",
    version ? selectedVersion,
    sha256 ? updateData.sha256,
    downloadUrl ? updateData.download_url,
    unpack ? lib.strings.hasSuffix ".zip" fileName,
    fetchAsExecutable ? lib.strings.hasSuffix ".jar" fileName,
    pluginStdenv ? pkgs.stdenvNoCC,
    since ? updateData.since,
    until ? updateData.until,
    compatibilityOverrides ? updateData.compatibility_overrides,
    channel ? updateData.channel,
  }: pluginStdenv.mkDerivation {
    name = name;
    version = version;

    # Download the plugin file
    src = maybeUnpackPlugin unpack (pkgs.fetchurl {
      url = downloadUrl;
      executable = fetchAsExecutable;
      inherit sha256;
    }) fileName;

    passthru = {
      rawData = data;
      inherit channel;
      inherit updateId;
      inherit updateData;
      compatibility = {
        inherit since until compatibilityOverrides;
      };
    };

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