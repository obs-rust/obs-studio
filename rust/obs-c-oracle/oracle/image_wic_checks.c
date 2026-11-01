/* Test-only oracle: the original
 * libobs/graphics/graphics-ffmpeg-host-wic-checks.c.h, unmodified, with
 * gs_image_host_fallback_refuses and gs_image_wic_kind renamed to oracle_*
 * so they can link next to the Rust shims. The rename of gs_image_wic_kind
 * also renames the enum's tag, which is all it touches. astrcmpi is the C
 * dstr oracle's (oracle/dstr.c). For a name shorter than 4 bytes the C
 * compares from before the start of the name: the parity test never passes
 * one. */
#include <stdbool.h>
#include <stddef.h>

#define astrcmpi oracle_astrcmpi
#define gs_image_host_fallback_refuses oracle_gs_image_host_fallback_refuses
#define gs_image_wic_kind oracle_gs_image_wic_kind

#include "graphics/graphics-ffmpeg-host-wic-checks.c.h"
