/* Test-only oracle: the original libobs/graphics/graphics-ffmpeg-host-pq.c.h,
 * unmodified, with gs_image_pq10_to_scrgb renamed to oracle_* so it can link
 * next to the Rust shim, plus two wrappers that reach what the conversion
 * calls: its static pq_to_linear() and half.h's static inline
 * half_from_float(). Built with -ffp-contract=off (build.rs), so the
 * matrix rows are not fused. The conversion reads src_size bytes and writes
 * 2 * src_size: the parity test always passes a dst of exactly that size. */
#include <stdint.h>

#define gs_image_pq10_to_scrgb oracle_gs_image_pq10_to_scrgb

#include "graphics/graphics-ffmpeg-host-pq.c.h"

#undef gs_image_pq10_to_scrgb

float oracle_pq_to_linear(float u);
uint16_t oracle_half_from_float(float f);

float oracle_pq_to_linear(float u)
{
	return pq_to_linear(u);
}

/* Undefined in C for a float below 2^-45 in magnitude (a shift of 32 or
 * more): the parity test never passes one. */
uint16_t oracle_half_from_float(float f)
{
	return half_from_float(f).u;
}
