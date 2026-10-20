/* Precompiled header (BUILD.md) for the libobs unity groups libobs.nix, libobs.nix-main, libobs.null-audio. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* obs-core-modules-loader-nix.c */
#include "obs-core-modules.h"

#include <obs-internal.h>
#include <obs.h>
#include <util/dstr.h>
#include <util/platform.h>

/* obs-nix-platform.c */
#include "obs-nix-platform.h"

#include <assert.h>

/* obs-nix-x11.c */
#include "obs-internal.h"
#include "obs-nix-x11.h"
#include "obs-nix-x11-keymap.h"

#include <xcb/xcb.h>
#if defined(XCB_XINPUT_FOUND)
#include <xcb/xinput.h>
#endif
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/Xlib-xcb.h>
#include <X11/XF86keysym.h>
#include <X11/Sunkeysym.h>

/* obs-nix.c */
#include "obs-nix.h"

#include "util/config-file.h"
#include "obs-nix-os-release.h"

#ifdef ENABLE_WAYLAND
#include "obs-nix-wayland.h"
#endif

#if defined(__FreeBSD__)
#define _GNU_SOURCE
#endif
#include <stdlib.h>
#include <stdio.h>
#if defined(__FreeBSD__) || defined(__OpenBSD__)
#include <sys/sysctl.h>
#endif
#if !defined(__OpenBSD__)
#include <sys/sysinfo.h>
#endif
#include <sys/utsname.h>
#include <inttypes.h>
/* access() in log_system_info; came with util/platform-nix-inhibit.c,
 * which is in its own group now. */
#include <unistd.h>
