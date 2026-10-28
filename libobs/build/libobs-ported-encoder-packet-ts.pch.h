/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-encoder-packet-ts. obs-encoder-packet-ts.c needs its
 * declarations (obs-encoder-packet-ts.h, with stddef.h and stdint.h) and the
 * definition of struct encoder_packet_time (obs-encoder.h, through obs.h),
 * not the core, so the C oracle and the C test can compile it on its own.
 * The implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs.h"
#include "libobs/obs-encoder-packet-ts.h"
