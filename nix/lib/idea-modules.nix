{ pkgs
, lib
}: rec {
  isPlatformModuleName = moduleName:
    lib.strings.hasPrefix "com.intellij.modules.os." moduleName ||
    lib.strings.hasPrefix "com.intellij.modules.arch." moduleName;

  platformModulesForStdenvTarget = stdenv:
    let
      platformModuleName = "com.intellij.modules.os.${lib.toLower stdenv.targetPlatform.uname.system}";
      archModuleName = "com.intellij.modules.arch.${lib.toLower stdenv.targetPlatform.uname.processor}";
    in [
      platformModuleName
      archModuleName
    ];

  platformModulesCompatible = stdenv: dependencies:
  let
    currentPlatformModules = platformModulesForStdenvTarget stdenv;
  in
    lib.all (dependency: !(isPlatformModuleName dependency) || lib.elem dependency currentPlatformModules) dependencies;
}
