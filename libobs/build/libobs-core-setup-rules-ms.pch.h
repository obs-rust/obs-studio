/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.core-setup-rules-ms: the audio buffering in milliseconds.
 * obs-core-setup-rules-ms.c needs only its declaration and constants
 * (obs-core-setup-rules.h, with stdbool.h, stddef.h and stdint.h), not the
 * core, so the C oracle and the C test can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs-core-setup-rules.h"
