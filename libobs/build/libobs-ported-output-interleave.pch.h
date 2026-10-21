/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-output-interleave. obs-output-interleave.c reads the output
 * and its encoders only through the hidden accessors and wrappers in
 * obs-internal.h (obs_output_interleave_fields() and the others), and needs
 * nothing else from the core, so the C oracle and the C test can compile it
 * on its own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include "obs-internal.h"
