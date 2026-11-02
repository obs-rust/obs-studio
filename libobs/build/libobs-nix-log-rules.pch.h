/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.nix-log-rules. obs-nix-log-rules.c needs only its declarations and
 * the C library, not the core or the display headers, so the C oracle and the
 * C tests can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

/* obs-nix-log-rules.c */
#include "libobs/obs-nix-system-info.h"

#include <string.h>
