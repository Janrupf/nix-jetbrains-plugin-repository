{ lib
, pkgs
, runCommandLocal

# Dependencies
, unzip
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

  createSinglePluginPackage = data: selectedVersion:
  let
    versionData = data.versions.${selectedVersion};
    fileName = versionData.file_name or "${data.name}-${selectedVersion}.jar";
  in pkgs.callPackage ({
    name ? "jetbrains-plugin-${data.xml_id}",
    version ? selectedVersion,
    sha256 ? versionData.sha256,
    downloadUrl ? versionData.download_url,
    unpack ? lib.strings.hasSuffix ".zip" fileName,
    fetchAsExecutable ? lib.strings.hasSuffix ".jar" fileName,
    stdenvNoCC
  }: stdenvNoCC.mkDerivation {
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
    };

    installPhase = ''
      runHook preInstall
      mkdir -p $out && cp -r . $out
      runHook postInstall
    '';
  }) {};

  createAllPluginPackages = data: let
    versions = lib.attrsets.mapAttrs (version: _:
      createSinglePluginPackage data version
    ) data.versions;

    channels = lib.attrsets.mapAttrs (channel: version: versions.${version}) data.latest;
    latest = if channels ? stable
      then channels.stable
      else let
        err = throw "No stable version found, please select a version explicitly via versions.<version> or channels.<channel>";
      in {
        # Hack so that a proper error message is shown when someone tries to use the
        # package without explicitly selecting a version
        type = "derivation";
        drvPath = err;
        name = err;
        outputs = err;
        meta = err;
        system = err;
      };
  in latest // {
    inherit channels;
    inherit versions;
  };
}