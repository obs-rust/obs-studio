/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-context-names. obs-context-names.c needs only the dstr and
 * bmem helpers and the declarations of its entry points and of the
 * obs_context_name_exists_t callback (obs-internal.h), so the C oracle and
 * the C tests can compile it on their own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "obs-internal.h"
