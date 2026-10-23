/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-module-locale. obs-module-locale.c needs only the dstr and
 * bmem helpers (astrcmpi() included) and the declarations of its entry
 * points (obs-internal.h), so the C oracle and the C tests can compile it on
 * their own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include "obs-internal.h"
