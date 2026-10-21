/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-output-caption-text. obs-output-caption-text.c needs only
 * the CAPTION_* bounds and the declaration of
 * obs_output_caption_text_page() (obs-internal.h), and memcpy, so the C
 * oracle can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <string.h>

#include "obs-internal.h"
