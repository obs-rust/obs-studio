/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-nix-inhibit. util/platform-nix-inhibit.c alone, without X11,
 * so the C oracle can compile it. The implementation file includes this
 * header and nothing else. */

#pragma once

/* util/platform-nix-inhibit.c */
#include <errno.h>
#include <signal.h>
#include <spawn.h>
#include <stdbool.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include "obsconfig.h"

#include "util/base.h"
#include "util/bmem.h"
#include "util/platform.h"
#include "util/threading.h"
