/* Original extracted scalar bodies, with recording vector-boundary callbacks. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "graphics/graphics.h"
#include "graphics/vec3.h"
#include "util/threading.h"

void oracle_gs_vertex2f(float x, float y);
void oracle_gs_normal3f(float x, float y, float z);
void oracle_immediate_scalars_reset(void);
size_t oracle_immediate_scalars_count(void);
uint32_t oracle_immediate_scalars_word(size_t index);

/* At most 64 calls per sequence, five words per call; thread-local so parity
 * tests can run alongside the rest of the Graphics category. */
static THREAD_LOCAL uint32_t immediate_scalars_trace[64 * 5];
static THREAD_LOCAL size_t immediate_scalars_count;

static void immediate_scalars_record(uint32_t kind, const struct vec3 *value)
{
	if (immediate_scalars_count + 5 > 64 * 5)
		abort();
	immediate_scalars_trace[immediate_scalars_count++] = kind;
	memcpy(&immediate_scalars_trace[immediate_scalars_count], value->ptr, sizeof(float) * 4);
	immediate_scalars_count += 4;
}

void gs_vertex3v(const struct vec3 *value)
{
	immediate_scalars_record(1, value);
}

void gs_normal3v(const struct vec3 *value)
{
	immediate_scalars_record(2, value);
}

#define gs_vertex2f oracle_gs_vertex2f
#define gs_normal3f oracle_gs_normal3f
#include "graphics-immediate-scalars.inc"
#undef gs_normal3f
#undef gs_vertex2f

void oracle_immediate_scalars_reset(void)
{
	immediate_scalars_count = 0;
}

size_t oracle_immediate_scalars_count(void)
{
	return immediate_scalars_count;
}

uint32_t oracle_immediate_scalars_word(size_t index)
{
	return immediate_scalars_trace[index];
}
