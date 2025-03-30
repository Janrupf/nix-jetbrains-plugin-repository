{ stdenv
, lib
, glibc
, gcc-unwrapped
, buildFHSEnv
, writeShellScript
, runtimeShell
, ...}:
let
  copilotFhsEnvs = buildFHSEnv {
    name = "copilot-jetbrains-fhs";

    runScript = writeShellScript "copilot-run-fhs" ''
      exec "$@"
    '';
  };
in pkg: pkg.overrideAttrs {
  buildPhase = ''
    agent='copilot-agent/native/${lib.toLower stdenv.hostPlatform.uname.system}${
      {
        x86_64 = "-x64";
        aarch64 = "-arm64";
      }
      .${stdenv.hostPlatform.uname.processor} or ""
    }/copilot-language-server'

    mkdir -p "$out/libexec"

    agentUnwrapped="$out/libexec/copilot-language-server.unwrapped"
    mv "$agent" "$agentUnwrapped"

    cat <<EOF > "$agent"
      #!${runtimeShell}
      exec ${copilotFhsEnvs}/bin/copilot-jetbrains-fhs "$agentUnwrapped" "\$@"
    EOF

    chmod +x "$agent"
  '';
}
