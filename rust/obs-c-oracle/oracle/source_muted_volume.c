/* Tier 3: the unchanged extracted source_muted() and get_source_volume()
 * (libobs/obs-source-muted-volume.c.h) against the Rust exports, over the
 * real source layout.
 *
 * The production adapters the Rust exports call
 * (libobs/obs-source-muted-volume-fields.c.h) are compiled here, once for
 * every test binary. Both they and the original load user_muted through a
 * counting os_atomic_load_bool(), so the parity test sees that the load is
 * atomic on both sides and skipped where the C's || skips it. The count is
 * per thread. */
#define _NO_CRT_STDIO_INLINE
#include <stddef.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static THREAD_LOCAL unsigned smv_loads;

static inline bool smv_load_bool(const volatile bool *ptr)
{
	smv_loads++;
	return os_atomic_load_bool(ptr);
}

#define os_atomic_load_bool smv_load_bool

#define source_muted oracle_source_muted
#define get_source_volume oracle_get_source_volume
bool oracle_source_muted(obs_source_t *source, uint64_t os_time);
float oracle_get_source_volume(obs_source_t *source, uint64_t os_time);
#include "source-muted-volume.inc"
#undef get_source_volume
#undef source_muted

#include "source-muted-volume-fields.inc"

#undef os_atomic_load_bool

/* The fields the two steps read and write; index 0 is push-to-mute, 1
 * push-to-talk. Mirrored by the parity test. */
struct oracle_smv_fields {
	bool enabled;
	bool user_muted;
	bool muted;
	bool key_enabled[2];
	bool user_pressed[2];
	bool pressed[2];
	uint64_t delay[2];
	uint64_t stop[2];
	float volume;
};

/* A zeroed source of the real layout, for the caller to free. */
struct obs_source *oracle_smv_source_new(void);
struct obs_source *oracle_smv_source_new(void)
{
	return bzalloc(sizeof(struct obs_source));
}

void oracle_smv_source_free(struct obs_source *source);
void oracle_smv_source_free(struct obs_source *source)
{
	bfree(source);
}

/* Zeroes the whole source, then sets the fields. */
void oracle_smv_set(struct obs_source *source, const struct oracle_smv_fields *f);
void oracle_smv_set(struct obs_source *source, const struct oracle_smv_fields *f)
{
	memset(source, 0, sizeof(*source));
	source->enabled = f->enabled;
	source->user_muted = f->user_muted;
	source->muted = f->muted;
	source->push_to_mute_enabled = f->key_enabled[0];
	source->push_to_talk_enabled = f->key_enabled[1];
	source->user_push_to_mute_pressed = f->user_pressed[0];
	source->user_push_to_talk_pressed = f->user_pressed[1];
	source->push_to_mute_pressed = f->pressed[0];
	source->push_to_talk_pressed = f->pressed[1];
	source->push_to_mute_delay = f->delay[0];
	source->push_to_talk_delay = f->delay[1];
	source->push_to_mute_stop_time = f->stop[0];
	source->push_to_talk_stop_time = f->stop[1];
	source->volume = f->volume;
}

void oracle_smv_get(const struct obs_source *source, struct oracle_smv_fields *f);
void oracle_smv_get(const struct obs_source *source, struct oracle_smv_fields *f)
{
	f->enabled = source->enabled;
	f->user_muted = source->user_muted;
	f->muted = source->muted;
	f->key_enabled[0] = source->push_to_mute_enabled;
	f->key_enabled[1] = source->push_to_talk_enabled;
	f->user_pressed[0] = source->user_push_to_mute_pressed;
	f->user_pressed[1] = source->user_push_to_talk_pressed;
	f->pressed[0] = source->push_to_mute_pressed;
	f->pressed[1] = source->push_to_talk_pressed;
	f->delay[0] = source->push_to_mute_delay;
	f->delay[1] = source->push_to_talk_delay;
	f->stop[0] = source->push_to_mute_stop_time;
	f->stop[1] = source->push_to_talk_stop_time;
	f->volume = source->volume;
}

/* The whole source, for a byte compare. */
size_t oracle_smv_source_size(void);
size_t oracle_smv_source_size(void)
{
	return sizeof(struct obs_source);
}

/* The user_muted loads on this thread since the last read; clears them. */
unsigned oracle_smv_loads(void);
unsigned oracle_smv_loads(void)
{
	unsigned loads = smv_loads;
	smv_loads = 0;
	return loads;
}
