/* Precompiled header (BUILD.md) for the libobs unity groups libobs.core, libobs.core-output, libobs.core-source, libobs.util, libobs.graphics. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* obs-hotkey.c */
#include <inttypes.h>

#include "obs-internal.h"

/* obs-missing-files.c */
#include "util/threading.h"
#include "util/dstr.h"
#include "obs-missing-files.h"
#include "obs.h"

/* obs-module.c */
#include "obs-core-modules-loader.h"
#include "obs-core-modules.h"
#include "obs-defs.h"
#include "obs-module.h"
#include "obs-host/module-paths.h"
#include "obs-host/module-symbols.h"
#include "util/platform.h"

/* obs.c */

#include "graphics/matrix4.h"
#include "callback/calldata.h"

#include "obs-host/com.h"
#include "obs-host/graphics-thread.h"
#include "obs-host/shared-texture.h"

/* obs-audio-controls.c */
#include <math.h>

#include "util/sse-intrin.h"

#include "util/bmem.h"
#include "media-io/audio-math.h"

#include "obs-audio-controls.h"

/* obs-audio.c */
#include "util/util_uint64.h"

/* obs-output.c */
#include "util/array-serializer.h"
#include "graphics/math-extra.h"
#include "obs-av1.h"

#include <caption/caption.h>
#include <caption/mpeg.h>

/* obs-video.c */
#include <time.h>
#include <stdlib.h>

#include "graphics/vec4.h"
#include "media-io/format-conversion.h"
#include "media-io/video-frame.h"
#include "obs-host/graphics-thread-hooks.h"

/* obs-canvas.c */
#include "obs-scene.h"

/* obs-display.c */
#include "obs-host/display-clear-workaround.h"
#include "obs-host/display-srgb-clear.h"

/* obs-scene-item-audio-actions.c */

#include "obs-scene-audio-actions.h"

/* obs-scene-bounds.c */

#include "obs-scene-bounds.h"

/* obs-scene-coords.c */

#include "obs-scene-coords.h"

/* obs-scene-group-transform.c */

#include "obs-scene-group-transform.h"

/* obs-scene.c */
#include "graphics/math-defs.h"
#include "obs-scene-blend.h"

/* obs-source.c */

#include "media-io/audio-io.h"
#include "obs-host/default-monitoring-device.h"
#include "graphics/matrix3.h"
#include "graphics/vec3.h"

/* callback/calldata.c */
#include <string.h>

#include "util/base.h"

/* callback/decl.c */
#include "util/cf-parser.h"
#include "callback/decl.h"

/* callback/proc.c */
#include "util/darray.h"

#include "callback/proc.h"

/* callback/signal.c */

#include "callback/signal.h"

/* util/cf-lexer.c */
#include <ctype.h>
#include <stdio.h>
#include "util/cf-lexer.h"

/* util/source-profiler.c */
#include "util/source-profiler.h"

#include "util/uthash.h"

/* graphics/effect-parser.c */
#include <assert.h>
#include <limits.h>
#include "graphics/effect-parser.h"
#include "graphics/effect.h"

/* graphics/effect.c */
#include "graphics/graphics-internal.h"
#include "graphics/vec2.h"

/* graphics/graphics.c */

#include "graphics/quat.h"
#include "graphics/axisang.h"

/* graphics/shader-parser.c */
#include "graphics/shader-parser.h"
