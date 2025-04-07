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

  createSinglePluginPackage = data: selectedVersion: fixup:
  let
    versionData = data.versions.${selectedVersion};
    fileName = versionData.file_name or "${data.name}-${selectedVersion}.jar";
  in fixup (pkgs.callPackage ({
    name ? "jetbrains-plugin-${data.xml_id}",
    version ? selectedVersion,
    sha256 ? versionData.sha256,
    downloadUrl ? versionData.download_url,
    unpack ? lib.strings.hasSuffix ".zip" fileName,
    fetchAsExecutable ? lib.strings.hasSuffix ".jar" fileName,
    pluginStdenv ? pkgs.stdenvNoCC,
    since ? versionData.since,
    until ? versionData.until,
    compatibilityOverrides ? versionData.compatibility_overrides,
    channel ? versionData.channel,
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
    versions = (lib.attrsets.mapAttrs (version: _:
      createSinglePluginPackage data version fixup
    ) data.versions) // {
      type = "versionset";
    };

    channels = let
      groupedByChannel = lib.lists.groupBy
        (v: v.data.channel)
        (lib.attrsets.mapAttrsToList (version: data: {
          inherit version;
          inherit data;
        }) data.versions);
    in (lib.attrsets.mapAttrs (_: versionList:
      lib.attrsets.listToAttrs (map (v: {
        name = v.version;
        value = versions.${v.version};
      }) versionList)
    ) groupedByChannel) // {
      type = "versionset";
    };
  in (channels.stable or { type = "versionset"; }) // channels // {
    all = versions;
  };
}