/* Precompiled header (BUILD.md) for the plugins/image-source unity groups
 * image-source.slideshow. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* obs-slideshow.c */
#include <obs-module.h>
#include <util/threading.h>
#include <util/platform.h>
#include <util/darray.h>
#include <util/dstr.h>
#include <util/task.h>
#include "plugins/image-source/image-source-host.h"
