# JetBrains Plugin Repository for Nix

A Nix flake that provides access to JetBrains IDE plugins with automatic compatibility checking and version selection.

## Features

- Access to all plugins from the official JetBrains marketplace
- Automatic compatibility checking between plugins and IDEs
- Support for different plugin channels (stable, nightly, etc.)
- Daily updates via GitHub Actions
- Easy integration with NixOS and Home Manager

## Quick Start

### Add the flake to your inputs

```nix
# In your flake.nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    
    nix-jetbrains-plugins = {
      url = "github:Janrupf/nix-jetbrains-plugin-repository";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
}
```

### Apply the overlay

```nix
# In your NixOS configuration
{
  nixpkgs.overlays = [
    inputs.nix-jetbrains-plugins.overlays.default
  ];
}
```

### Install JetBrains IDEs with plugins

```nix
# In your packages list
environment.systemPackages = [
  (pkgs.jetbrains-plugins.lib.buildIdeWithPlugins pkgs.jetbrains.idea-community (with pkgs.jetbrains-plugins; [
    # Latest compatible version of Github Copilot from stable channel
    com.github.copilot
    
    # Latest compatible version of Rust plugin from nightly channel
    com.jetbrains.rust.nightly
    
    # Specific version of a plugin (no compatibility check)
    com.example.my-plugin."1.0.0"
  ]))
];
```

## Finding Plugin IDs

To find a plugin's ID:
1. Go to https://plugins.jetbrains.com/
2. Search for and select your desired plugin
3. Scroll down to find the plugin ID above the "Report Plugin" button

## Plugin Structure

After applying the overlay, plugins are available in `pkgs.jetbrains-plugins`. Each plugin is structured as follows:

```nix
jetbrains-plugins.com.example.my-plugin = {
  # Direct version access - this is only available if the 
  # stable channel exists.
  "1.0.0" = { ... };  # Specific version
  "1.1.0" = { ... };  # Specific version
  
  # Channel-specific versions
  stable = {
    "1.0.0" = { ... };
    "1.1.0" = { ... };
    type = "versionset";
  };
  
  nightly = {
    "1.2.0" = { ... };
    type = "versionset";
  };
  
  # All versions across all channels
  # WARNING: This contain the raw update id instead of
  # the expected version!
  all = {
    "158822" = { ... };
    "232943" = { ... };
    "571264" = { ... };
    type = "versionset";
  };
  
  type = "versionset";
};
```

## Advanced Usage

### Home Manager Integration

```nix
home.packages = [
  (pkgs.jetbrains-plugins.lib.buildIdeWithPlugins pkgs.jetbrains.webstorm (with pkgs.jetbrains-plugins; [
    com.github.copilot
    io.flutter
  ]))
];
```

### Manual Plugin Selection

You can manually select specific plugin versions:

```nix
(pkgs.jetbrains-plugins.lib.buildIdeWithPlugins pkgs.jetbrains.pycharm-professional [
  pkgs.jetbrains-plugins.com.intellij.kubernetes."223.8836.35"
  pkgs.jetbrains-plugins.org.toml.lang."0.2.155.5308"
])
```

### Important Notes

- Plugin dependencies are not automatically resolved. You need to add dependencies manually.
- The repository is updated daily via GitHub Actions.
- Compatibility is checked based on the IDE's build number and plugin metadata.
