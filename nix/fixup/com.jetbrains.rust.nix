{ stdenv
, autoPatchelfHook
, lib
, ...}:
pkg: (pkg.override { pluginStdenv = stdenv; }).overrideAttrs {
  nativeBuildInputs = lib.optional stdenv.hostPlatform.isLinux autoPatchelfHook;
  buildInputs = [ (lib.getLib stdenv.cc.cc) ];

  buildPhase = ''
    runHook preBuild
    chmod +x -R bin
    runHook postBuild
  '';
}
