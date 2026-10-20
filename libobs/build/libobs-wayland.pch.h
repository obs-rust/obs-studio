/* Precompiled header (BUILD.md) for the libobs unity group libobs.nix-wayland. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* obs-nix-wayland.c */
#include "obs-internal.h"
#include "obs-nix-hotkey-state.h"
#include "obs-nix-platform.h"
#include "obs-nix-wayland.h"
#ifdef HAVE_GLOBAL_SHORTCUTS_PORTAL
#include "obs-nix-portal-shortcuts.h"
#endif

#include <unistd.h>
#include <sys/mman.h>
#include <string.h>

#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
