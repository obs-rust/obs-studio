# Native NixOS environment for sanitize.py and a host (Docker-free) Tier 2
# run of rust/tools/linux-validate/validate.sh: the nixpkgs OBS build inputs
# plus cmocka, valgrind, a headless X server, and what the cmocka tests start
# (PipeWire with pipewire-pulse, WirePlumber, pactl, D-Bus, VLC).
# Usage: nix-shell rust/tools/sanitize/shell.nix
{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  inputsFrom = [ pkgs.obs-studio ];
  packages = with pkgs; [
    cmocka valgrind xorg.xorgserver xvfb-run dbus mesa ninja curl git rsync
    pipewire wireplumber pulseaudio vlc
    simde uthash nlohmann_json websocketpp asio qrcodegencpp
  ];
  hardeningDisable = [ "all" ];
  shellHook = ''
    export QT_PLUGIN_PATH=${pkgs.qt6.qtbase}/${pkgs.qt6.qtbase.qtPluginPrefix}:${pkgs.qt6.qtsvg}/${pkgs.qt6.qtbase.qtPluginPrefix}:${pkgs.qt6.qtwayland}/${pkgs.qt6.qtbase.qtPluginPrefix}
    export LIBGL_DRIVERS_PATH=${pkgs.mesa}/lib/dri
    export __EGL_VENDOR_LIBRARY_FILENAMES=${pkgs.mesa}/share/glvnd/egl_vendor.d/50_mesa.json
    export LD_LIBRARY_PATH=${pkgs.libglvnd}/lib:${pkgs.mesa}/lib''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
  '';
}
