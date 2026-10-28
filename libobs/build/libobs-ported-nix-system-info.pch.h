/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-nix-system-info. obs-nix-system-info.c needs only its
 * declarations and the C library, not the core or the display headers, so the
 * C oracle and the C test can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

/* obs-nix-system-info.c */
#include "libobs/obs-nix-system-info.h"

#include <stdlib.h>
#include <string.h>
