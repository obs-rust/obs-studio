/* Original texture-file-data2.c.h, compiled as
 * oracle_gs_create_texture_file_data2 over a thread-local recording data3
 * (oracle_tfd2_data3). The Rust shim's create_texture_file_data2_with gets
 * the same recorder, so Tier 3 compares every argument data3 sees, how
 * often it runs, what it writes and what data2 returns. The real-name
 * gs_create_texture_file_data3 stand-in stays in texture_render_device.c. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#define gs_create_texture_file_data2 oracle_gs_create_texture_file_data2
#define gs_create_texture_file_data3 oracle_tfd2_data3
#include "graphics/graphics.h"
#include "util/threading.h"

/* What the last data3 call saw, as plain integers for Rust. */
struct oracle_tfd2_call {
	uintptr_t file;
	uint32_t alpha_mode;
	uintptr_t format;
	uintptr_t cx_out;
	uintptr_t cy_out;
	uintptr_t space;
};

void oracle_tfd2_configure(uint8_t *result, bool write, uint32_t format, uint32_t cx, uint32_t cy, uint32_t space);
uint32_t oracle_tfd2_calls(struct oracle_tfd2_call *last);
size_t oracle_tfd2_layout(size_t *out, size_t cap);

static THREAD_LOCAL uint8_t *tfd2_result;
static THREAD_LOCAL bool tfd2_write;
static THREAD_LOCAL uint32_t tfd2_format, tfd2_cx, tfd2_cy, tfd2_space;
static THREAD_LOCAL uint32_t tfd2_calls;
static THREAD_LOCAL struct oracle_tfd2_call tfd2_last;

uint8_t *oracle_tfd2_data3(const char *file, enum gs_image_alpha_mode alpha_mode, enum gs_color_format *format,
			   uint32_t *cx_out, uint32_t *cy_out, enum gs_color_space *space)
{
	tfd2_calls++;
	tfd2_last.file = (uintptr_t)file;
	tfd2_last.alpha_mode = (uint32_t)alpha_mode;
	tfd2_last.format = (uintptr_t)format;
	tfd2_last.cx_out = (uintptr_t)cx_out;
	tfd2_last.cy_out = (uintptr_t)cy_out;
	tfd2_last.space = (uintptr_t)space;
	if (tfd2_write) {
		*format = (enum gs_color_format)tfd2_format;
		*cx_out = tfd2_cx;
		*cy_out = tfd2_cy;
		*space = (enum gs_color_space)tfd2_space;
	}
	return tfd2_result;
}

/* Sets what the next data3 calls return and write (only when write), and
 * clears the call record. */
void oracle_tfd2_configure(uint8_t *result, bool write, uint32_t format, uint32_t cx, uint32_t cy, uint32_t space)
{
	tfd2_result = result;
	tfd2_write = write;
	tfd2_format = format;
	tfd2_cx = cx;
	tfd2_cy = cy;
	tfd2_space = space;
	tfd2_calls = 0;
	tfd2_last = (struct oracle_tfd2_call){0};
}

uint32_t oracle_tfd2_calls(struct oracle_tfd2_call *last)
{
	*last = tfd2_last;
	return tfd2_calls;
}

/* sizeof and alignof of the ABI's enums and the call record, in order:
 * gs_image_alpha_mode, gs_color_format, gs_color_space, then the record's
 * size and alignment. */
size_t oracle_tfd2_layout(size_t *out, size_t cap)
{
	const size_t values[] = {
		sizeof(enum gs_image_alpha_mode), _Alignof(enum gs_image_alpha_mode), sizeof(enum gs_color_format),
		_Alignof(enum gs_color_format),   sizeof(enum gs_color_space),        _Alignof(enum gs_color_space),
		sizeof(struct oracle_tfd2_call),  _Alignof(struct oracle_tfd2_call),
	};
	size_t n = sizeof(values) / sizeof(values[0]);

	if (n > cap)
		n = cap;
	for (size_t i = 0; i < n; i++)
		out[i] = values[i];
	return n;
}

#include "texture-file-data2.inc"
