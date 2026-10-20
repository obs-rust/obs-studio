/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-output-delay. obs-output-delay.c reads the output only
 * through obs_output_delay_fields() and obs_output_delay_signal()
 * (obs-internal.h), and needs nothing else from the core, so the C oracle
 * and the C test can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "obs-internal.h"
