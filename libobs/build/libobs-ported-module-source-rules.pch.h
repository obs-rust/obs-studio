/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-module-source-rules. obs-module-source-rules.c needs only the
 * source type and flag definitions (obs-source.h), the dstr and bmem helpers
 * and the declarations of its entry points (obs-internal.h), so the C oracle
 * and the C tests can compile it on their own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "obs-internal.h"
