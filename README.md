# Jetbrains Plugin Repository for Nix

## Using this flake

Add the following to your inputs:

```nix
nix-jetbrains-plugins = {
  url = "github:Janrupf/nix-jetbrains-plugin-repository";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Then add the overlay to your system configuration:

```nix
nixpkgs.overlays = [
  nix-jetbrains-plugins.overlays.default
];
```

Afterwards you can build Jetbrains IDE packages with the wanted plugins pre-installed:
```nix
environment.systemPackages = [
  (pkgs.jetbrains-plugins.lib.buildIdeWithPlugins pkgs.jetbrains.idea-community (with pkgs.jetbrains-plugins; [
    # Install the latest compatible version of Github Copilot's stable channel
    com.github.copilot

    # Install the latest compatible version of the Rust plugin from the nightly channel 
    com.jetbrains.rust.nightly

    # Install version 1.0.0 of the example plugin
    # Note that in this case no compatibility checks are done, the
    # plugin is assumed to be compatible
    com.example.my-plugin."1.0.0"

    # To find the plugin ID, go to https://plugins.jetbrains.com/ and select a plugin.
    # Scroll down and then find the plugin ID directly above the "Report Plugin" button.
  ]))
];
```

Currently plugin dependencies are not resolved! If you install plugins, make sure that
you also add their dependencies. On IDE startup you will usually receive a warning if
you are missing dependencies.

## Plugin versions and channels

The repository contains metadata for all plugins and all versions that are currently available
on the Jetbrains marketplace. This information is updated daily by a Github cron Workflow,
that synchronizes the metadata from the Jetbrains marketplace.

Plugins can generally be found (after applying the overlay) in `pkgs.jetbrains-plugins`.
An entry inside `jetbrains-plugins` is not a derivation itself, but rather a set of
versions and channels.

An imaginary plugin may look like this:
```nix
jetbrains-plugins.com.example.my-plugin = {
  # Versions from the stable channel
  "1.0.0" = { ... };
  "1.1.0" = { ... };

  # This channel almost always exists
  stable = {
    "1.0.0" = { ... };
    "1.1.0" = { ... };

    type = "versionset";
  };

  # Other channels appear here too, but their names
  # are different depending on the plugin - nightly and eap
  # are somewhat common names, but plugin authors are free 
  # to choose whatever name they desire. Check the plugin
  # website on plugins.jetbrains.com to determine the available
  # channels.
  nightly = {
    "1.2.0" = { ... };

    type = "versionset";
  };

  # This artifical channel is added by the nix packaging scripts
  # and contains all versions across all channels.
  all = {
    "1.0.0" = { ... };
    "1.1.0" = { ... };
    "1.2.0" = { ... };

    type = "versionset";
  };

  # Indicates that this is a mapping of versions
  type = "versionset";
};
```

The `buildIdeWithPlugins` function accepts an array of versionsets and packages. Packages are
passed through directly and not checked for compatibility, while versionsets are 
expected to contain jetbrains plugins with compatibility metadata. 
