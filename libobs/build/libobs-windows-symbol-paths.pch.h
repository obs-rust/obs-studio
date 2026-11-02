/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.windows-symbol-paths. obs-windows-symbol-paths.c needs only its
 * declarations and util/dstr.h, not Windows or the core headers, so a C test
 * can compile it on every OS. The implementation file includes this header and
 * nothing else. */

#pragma once

/* obs-windows-symbol-paths.c */
#include "libobs/obs-windows-symbol-paths.h"

#include <string.h>
