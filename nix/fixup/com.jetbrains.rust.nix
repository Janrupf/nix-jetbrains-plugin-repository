{ stdenv
, autoPatchelfHook
, lib
, ...}:
pkg: (pkg.override { pluginStdenv = stdenv; }).overrideAttrs (prev: {
  nativeBuildInputs = (lib.optional stdenv.hostPlatform.isLinux autoPatchelfHook) ++ prev.nativeBuildInputs;
  buildInputs = [ (lib.getLib stdenv.cc.cc) ];

  buildPhase = ''
    runHook preBuild
    chmod +x -R bin
    runHook postBuild
  '';
})
