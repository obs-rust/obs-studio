/* Precompiled header (BUILD.md) for the plugins/image-source unity groups
 * image-source.core. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* color-source.c */
#include <obs-module.h>

/* image-file-version.c */
#include "image-file-version.h"
#include "plugins/image-source/image-source-host.h"

#include <string.h>
#include <sys/stat.h>

#include <util/platform.h>

/* image-source.c */
#include <graphics/image-file.h>
#include <util/threading.h>
#include <util/dstr.h>

/* obs-slideshow-mk2.c */
#include <util/deque.h>
#include <util/darray.h>
#include <util/task.h>

#include <inttypes.h>

#include "plugins/image-source/slideshow-order.h"
#include "slideshow-size.h"

/* slideshow-size.c */

#include <stdio.h>
