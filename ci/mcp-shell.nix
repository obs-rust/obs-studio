# Native SDK for the repository MCP bootstrap on NixOS.
{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [ pkg-config uv python3 ];
  buildInputs = with pkgs; [
    libx11 libxcb libuuid ffmpeg-full jansson zlib simde
    libpulseaudio glib wayland libxkbcommon uthash
    libdrm libglvnd mesa x264 v4l-utils pipewire libva pciutils
  ];
  shellHook = ''
    export OBS_MCP_NIX_SHELL=1
  '';
}
