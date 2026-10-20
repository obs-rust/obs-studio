/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-nix-portal-shortcut-ids (ENABLE_WAYLAND + GIO): the portal
 * shortcut id and trigger codec, without <gio/gio.h>, so the C tests and
 * the C oracle compile it on their own. Its implementation file includes
 * this header and nothing else. */

#pragma once

#include "obs-nix-portal-shortcuts.h"

#include <string.h>
