/* Precompiled header (BUILD.md) for the plugins/obs-transitions unity groups
 * obs-transitions.core. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* obs-transitions.c */
#include <obs-module.h>

/* transition-fade-to-color.c */

#include "fade-to-color-mix.h"

/* transition-luma-wipe.c */
#include <graphics/image-file.h>
#include <util/dstr.h>

/* transition-slide.c */
#include <graphics/vec2.h>
#include "easings.h"

/* transition-stinger.c */
#include "util/platform.h"
/* transition-stinger-monitoring.h: the pre-33.0 audio monitoring migration
 * the Stinger needs, shared with its test (#2638). */
#include "transition-stinger-monitoring.h"
