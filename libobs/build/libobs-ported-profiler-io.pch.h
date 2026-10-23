/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-profiler-io. util/profiler-io.c alone, without the core, so
 * the C oracle and C tests can compile it. The implementation file includes
 * this header and nothing else. */

#pragma once

/* util/profiler-io.c */
#include <inttypes.h>
#include <math.h>
#include <stdio.h>

#include "util/bmem.h"
#include "util/dstr.h"
#include "util/platform.h"
#include "util/profiler.h"
#include "util/profiler-snapshot.h"

#include <zlib.h>
