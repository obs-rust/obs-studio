/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.nix-wayland-keys: the Wayland keysym tables and keymap rules, built
 * with libobs.nix-wayland but without the Wayland and xkbcommon library
 * headers (only the keysym names), so the C tests and the C oracle compile it
 * on their own. C. Its implementation file includes this header and nothing
 * else. */

#pragma once

#include "obs-nix-wayland-keys.h"

#include <xkbcommon/xkbcommon-keysyms.h>
