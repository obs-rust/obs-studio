/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.core-setup-rules-task: the task queue's route and the video level
 * checks. obs-core-setup-rules-task.c needs only its declarations and
 * constants (obs-core-setup-rules.h, with stdbool.h, stddef.h and stdint.h)
 * and math.h for isfinite(), not the core, so the C oracle and the C test
 * can compile it on its own. The implementation file includes this header
 * and nothing else. */

#pragma once

#include <math.h>

#include "libobs/obs-core-setup-rules.h"
