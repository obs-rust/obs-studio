/* Precompiled header (BUILD.md) for the libobs unity group libobs.ported-platform. C.
 * Every implementation file in it includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* util/platform-conversions.c, util/platform-file.c, util/platform-filename.c, util/platform-mkdirs.c, util/platform-time.c */
#define _FILE_OFFSET_BITS 64

#include <time.h>
#include <errno.h>
#include <stdlib.h>
#include <inttypes.h>
#include <locale.h>
#include "util/c99defs.h"
#include "util/platform.h"
#include "util/bmem.h"
#include "util/utf8.h"
#include "util/dstr.h"
#include "obs.h"
#include "util/threading.h"
