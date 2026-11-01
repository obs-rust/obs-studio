/* Precompiled header (BUILD.md) for the libobs unity groups libobs.graphics and
 * libobs.ported-effect-lifecycle. C. Every implementation file in them includes this header and nothing else:
 * the includes below are theirs, in group and file order. libobs/graphics/ needs only util/ and graphics/
 * headers, so a core header (obs-internal.h, the output, encoder and audio headers) no longer rebuilds the
 * graphics module (#3260). */

#pragma once

/* graphics/effect-parser.c */
#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include "util/base.h"
#include "util/bmem.h"
#include "util/dstr.h"
#include "util/darray.h"
#include "util/platform.h"
#include "util/cf-parser.h"
#include "graphics/effect-param-types.h"
#include "graphics/effect-parser.h"
#include "graphics/effect.h"

/* graphics/effect.c */
#include "graphics/graphics-state-stacks.h"
#include "graphics/graphics-internal.h"
#include "graphics/vec2.h"
#include "graphics/vec3.h"
#include "graphics/vec4.h"

/* graphics/graphics.c */
#include "graphics/graphics-immediate.h"
#include "graphics/graphics-sprite.h"
#include "graphics/quat.h"
#include "graphics/axisang.h"

/* graphics/obs-graphics-adapter.c */
#include "graphics/image-file.h"

/* graphics/shader-parser.c */
#include "graphics/shader-parser.h"
