/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-output-caption. obs-output-caption.c needs only its
 * declarations (obs-output-caption.h, with stdbool.h and stdint.h), memcmp()
 * and strcmp(), and libcaption's eia608_parity_varify() (a header-only
 * inline), not the core, so the C oracle and the C test can compile it on
 * its own. The implementation file includes this header and nothing else. */

#pragma once

#include <string.h>

#include <caption/eia608.h>

#include "libobs/obs-output-caption.h"
