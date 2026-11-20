/* Original obs_canvas_has_valid_video_info() over the original C canvas
 * rules, beside the real-header ovi adapter of
 * obs-canvas-has-valid-video-info-fields.c.h. Under cargo test the adapter
 * is the only definition of its real name, so the Rust shim reads a real
 * struct obs_canvas through it. The probe runs the original and a candidate
 * on the same canvas and reports, instead of aborting, any difference in the
 * result, the canvas bytes or the allocation count, so mutants are
 * observable. */
/* The MSVC UCRT headers define snprintf and friends inline in every object
 * that includes them, which lets a COFF link pull this member into an
 * unrelated test; keep them declarations only. */
#define _NO_CRT_STDIO_INLINE
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

bool oracle_obs_canvas_rules_size_set(uint32_t size);
bool oracle_obs_canvas_rules_format_set(int format);
bool oracle_obs_canvas_has_valid_video_info(obs_canvas_t *canvas);

#define obs_canvas_rules_size_set oracle_obs_canvas_rules_size_set
#define obs_canvas_rules_format_set oracle_obs_canvas_rules_format_set
#define obs_canvas_has_valid_video_info oracle_obs_canvas_has_valid_video_info
#include "libobs/obs-canvas-has-valid-video-info.c.h"
#undef obs_canvas_has_valid_video_info
#undef obs_canvas_rules_format_set
#undef obs_canvas_rules_size_set
#include "canvas_has_valid_video_info_fields.inc"

typedef bool (*cvi_check_fn)(obs_canvas_t *canvas);

/* sizeof/alignof obs_video_info, the offsets of the five fields read, and
 * the size of enum video_format. */
void oracle_canvas_valid_video_info_layout(size_t *out);
void oracle_canvas_valid_video_info_layout(size_t *out)
{
	out[0] = sizeof(struct obs_video_info);
	out[1] = _Alignof(struct obs_video_info);
	out[2] = offsetof(struct obs_video_info, base_width);
	out[3] = offsetof(struct obs_video_info, base_height);
	out[4] = offsetof(struct obs_video_info, output_width);
	out[5] = offsetof(struct obs_video_info, output_height);
	out[6] = offsetof(struct obs_video_info, output_format);
	out[7] = sizeof(enum video_format);
}

/* The adapter's address against the real field. */
bool oracle_canvas_valid_video_info_adapter(void);
bool oracle_canvas_valid_video_info_adapter(void)
{
	static _Thread_local struct obs_canvas canvas;
	return obs_canvas_has_valid_video_info_ovi(&canvas) == &canvas.ovi;
}

/* 0: match. 1: the results differ, 2: the original's result is not the C
 * expression's, 4: the candidate changed the canvas, 8: it allocated. */
unsigned oracle_canvas_valid_video_info_probe(cvi_check_fn candidate, uint8_t fill, uint32_t base_width,
					      uint32_t base_height, uint32_t output_width, uint32_t output_height,
					      int output_format);
unsigned oracle_canvas_valid_video_info_probe(cvi_check_fn candidate, uint8_t fill, uint32_t base_width,
					      uint32_t base_height, uint32_t output_width, uint32_t output_height,
					      int output_format)
{
	static _Thread_local struct obs_canvas canvas, before;
	memset(&canvas, fill, sizeof(canvas));
	canvas.ovi.base_width = base_width;
	canvas.ovi.base_height = base_height;
	canvas.ovi.output_width = output_width;
	canvas.ovi.output_height = output_height;
	canvas.ovi.output_format = (enum video_format)output_format;
	memcpy(&before, &canvas, sizeof(canvas));
	bool expected = base_width && base_height && output_width && output_height && output_format != 0;
	bool original = oracle_obs_canvas_has_valid_video_info(&canvas);
	long allocations = bnum_allocs();
	bool actual = candidate(&canvas);
	unsigned diff = 0;
	if (actual != original)
		diff |= 1;
	if (original != expected)
		diff |= 2;
	if (memcmp(&canvas, &before, sizeof(canvas)) != 0)
		diff |= 4;
	if (bnum_allocs() != allocations)
		diff |= 8;
	return diff;
}
