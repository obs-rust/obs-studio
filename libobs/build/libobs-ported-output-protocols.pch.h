/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-output-protocols. obs-output-protocols.c needs only the
 * declaration of obs_output_protocol_list_count() (obs-internal.h) and
 * strlen, strchr and strncmp, so the C oracle and the C test can compile it
 * on their own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include <string.h>

#include "obs-internal.h"
