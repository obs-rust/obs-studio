/* Test-only oracle: the original libobs/graphics/effect.c.h, unmodified,
 * with gs_effect_destroy and gs_effect_actually_destroy renamed to
 * oracle_gs_effect_destroy and oracle_gs_effect_actually_destroy so they
 * can link next to the Rust shims, and the two calls the destroy makes,
 * bfree() and gs_shader_destroy(), renamed to the recorder below. The
 * header-inline frees of graphics/effect.h it runs are compiled here as
 * written; oracle_effect_free instantiates effect_free() on its own.
 *
 * The recorder writes each call (which one, and the pointer) to one event
 * log, then frees the block with the real bfree() (the Rust bmem, which the
 * Rust shim allocates and frees with too); a shader is only recorded. The
 * Rust shim's parity Calls call the same two recorder functions, so both
 * sides write the same log. The bench turns recording off, so the recorder
 * only frees. */

#define gs_effect_destroy oracle_gs_effect_destroy
#define gs_effect_actually_destroy oracle_gs_effect_actually_destroy
#define bfree oracle_destroy_bfree
#define gs_shader_destroy oracle_destroy_shader_destroy

#include "graphics/effect.c.h"

void oracle_effect_free(gs_effect_t *effect);

void oracle_effect_free(gs_effect_t *effect)
{
	effect_free(effect);
}

#undef bfree
#undef gs_shader_destroy

void bfree(void *ptr);

#define DESTROY_EVENTS 4096

struct destroy_event {
	int kind;
	const void *ptr;
};

static struct destroy_event destroy_events[DESTROY_EVENTS];
static size_t destroy_count;
static size_t destroy_dropped;
static bool destroy_recording = true;

void oracle_destroy_set_recording(bool recording);
void oracle_destroy_reset(void);
size_t oracle_destroy_count(void);
size_t oracle_destroy_dropped(void);
int oracle_destroy_event(size_t i, const void **ptr);

static void destroy_record(int kind, const void *ptr)
{
	if (!destroy_recording)
		return;
	if (destroy_count < DESTROY_EVENTS) {
		destroy_events[destroy_count].kind = kind;
		destroy_events[destroy_count].ptr = ptr;
		destroy_count++;
	} else {
		destroy_dropped++;
	}
}

void oracle_destroy_bfree(void *ptr)
{
	destroy_record('f', ptr);
	bfree(ptr);
}

void oracle_destroy_shader_destroy(gs_shader_t *shader)
{
	destroy_record('s', shader);
}

void oracle_destroy_set_recording(bool recording)
{
	destroy_recording = recording;
}

void oracle_destroy_reset(void)
{
	destroy_count = 0;
	destroy_dropped = 0;
}

size_t oracle_destroy_count(void)
{
	return destroy_count;
}

size_t oracle_destroy_dropped(void)
{
	return destroy_dropped;
}

/* The i-th event's kind ('f' a bfree, 's' a shader destroy) and pointer;
 * 0 past the end. */
int oracle_destroy_event(size_t i, const void **ptr)
{
	if (i >= destroy_count)
		return 0;
	*ptr = destroy_events[i].ptr;
	return destroy_events[i].kind;
}
