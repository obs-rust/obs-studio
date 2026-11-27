/* Tier 3: the unchanged extracted obs_source_duplicate()
 * (libobs/obs-source-duplicate-entry.c.h) over the real private layout, and
 * the production field adapters the Rust shim calls
 * (obs-source-duplicate-entry-fields.c.h, defined once, here).
 *
 * The libobs calls it makes (the scene and group lookups, the reference,
 * obs_scene_duplicate() and obs_scene_get_source(), obs_data_create(),
 * obs_data_apply() and obs_data_release(), both creates, the sync offset and
 * volume getters and obs_source_duplicate_filters()) are recording stand-ins
 * for both sides. Each call is recorded in order with the handles and
 * values it was given; the getters answer the source's own fields. The
 * decision steps (obs-source-duplicate.h) are the real ones of the build.
 * The real calls run through the real core in test_source_duplicate. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_duplicate oracle_obs_source_duplicate
#include "libobs/build/libobs.pch.h"

#define SD_EVENTS 24
/* One event: kind and four values. */
#define SD_EVENT_WIDTH 5

/* Handle ids. */
enum {
	SD_NULL = -1,
	SD_OTHER = -2,
	SD_SOURCE = 0,
	SD_CREATED = 1,
	SD_COPY_SOURCE = 2,
	SD_SCENE = 10,
	SD_GROUP = 11,
	SD_SCENE_COPY = 12,
	SD_SETTINGS = 20,
	SD_CONTEXT_SETTINGS = 21,
	SD_SOURCE_PRIVATE = 22,
	SD_CREATED_PRIVATE = 23,
	SD_NAME = 30,
	SD_ID = 31,
};

/* Event kinds. */
enum {
	SD_EV_SCENE = 1,
	SD_EV_GROUP,
	SD_EV_GET_REF,
	SD_EV_SCENE_DUPLICATE,
	SD_EV_SCENE_SOURCE,
	SD_EV_DATA_CREATE,
	SD_EV_DATA_APPLY,
	SD_EV_CREATE,
	SD_EV_CREATE_PRIVATE,
	SD_EV_DATA_RELEASE,
	SD_EV_SYNC_OFFSET,
	SD_EV_GET_VOLUME,
	SD_EV_DUPLICATE_FILTERS,
	SD_EV_LOG,
};

/* Answers of the stand-ins: bits of `answers`. */
enum {
	SD_A_SCENE = 1,
	SD_A_GROUP = 2,
	SD_A_REF = 4,
	SD_A_SCENE_COPY = 8,
	SD_A_CREATED = 16,
};

struct sd_fixture {
	struct obs_source sources[3];
	struct obs_source before[3];
	char markers[8];
	char name[8];
	char id[8];
	unsigned answers;
	long events[SD_EVENTS][SD_EVENT_WIDTH];
	size_t event_count;
	bool overflow;
	char message[128];
};

/* Per thread: the Rust test harness may run cases on several threads. */
static _Thread_local struct sd_fixture *sd_current;

static long sd_id(const void *p)
{
	struct sd_fixture *f = sd_current;
	if (!p)
		return SD_NULL;
	for (long i = 0; i < 3; i++) {
		if (p == &f->sources[i])
			return i;
	}
	static const long marker_ids[] = {
		SD_SCENE,          SD_GROUP,           SD_SCENE_COPY, SD_SETTINGS, SD_CONTEXT_SETTINGS,
		SD_SOURCE_PRIVATE, SD_CREATED_PRIVATE, SD_OTHER};
	for (size_t i = 0; i < 7; i++) {
		if (p == &f->markers[i])
			return marker_ids[i];
	}
	if (p == f->name)
		return SD_NAME;
	if (p == f->id)
		return SD_ID;
	return SD_OTHER;
}

static void *sd_marker(long id)
{
	switch (id) {
	case SD_SCENE:
		return &sd_current->markers[0];
	case SD_GROUP:
		return &sd_current->markers[1];
	case SD_SCENE_COPY:
		return &sd_current->markers[2];
	case SD_SETTINGS:
		return &sd_current->markers[3];
	case SD_CONTEXT_SETTINGS:
		return &sd_current->markers[4];
	case SD_SOURCE_PRIVATE:
		return &sd_current->markers[5];
	case SD_CREATED_PRIVATE:
		return &sd_current->markers[6];
	default:
		return NULL;
	}
}

static void sd_event(long kind, long a, long b, long c, long d)
{
	struct sd_fixture *f = sd_current;
	if (f->event_count >= SD_EVENTS) {
		f->overflow = true;
		return;
	}
	long *e = f->events[f->event_count++];
	e[0] = kind;
	e[1] = a;
	e[2] = b;
	e[3] = c;
	e[4] = d;
}

static bool sd_answer(unsigned bit)
{
	return (sd_current->answers & bit) != 0;
}

static obs_scene_t *sd_scene_from_source(const obs_source_t *source)
{
	sd_event(SD_EV_SCENE, sd_id(source), 0, 0, 0);
	return sd_answer(SD_A_SCENE) ? sd_marker(SD_SCENE) : NULL;
}

static obs_scene_t *sd_group_from_source(const obs_source_t *source)
{
	sd_event(SD_EV_GROUP, sd_id(source), 0, 0, 0);
	return sd_answer(SD_A_GROUP) ? sd_marker(SD_GROUP) : NULL;
}

static obs_source_t *sd_get_ref(obs_source_t *source)
{
	sd_event(SD_EV_GET_REF, sd_id(source), 0, 0, 0);
	return sd_answer(SD_A_REF) ? source : NULL;
}

static obs_scene_t *sd_scene_duplicate(obs_scene_t *scene, const char *name, enum obs_scene_duplicate_type type)
{
	sd_event(SD_EV_SCENE_DUPLICATE, sd_id(scene), sd_id(name), (long)type, 0);
	return sd_answer(SD_A_SCENE_COPY) ? sd_marker(SD_SCENE_COPY) : NULL;
}

static obs_source_t *sd_scene_get_source(const obs_scene_t *scene)
{
	sd_event(SD_EV_SCENE_SOURCE, sd_id(scene), 0, 0, 0);
	return scene == sd_marker(SD_SCENE_COPY) ? &sd_current->sources[SD_COPY_SOURCE] : NULL;
}

static obs_data_t *sd_data_create(void)
{
	sd_event(SD_EV_DATA_CREATE, 0, 0, 0, 0);
	return sd_marker(SD_SETTINGS);
}

static void sd_data_apply(obs_data_t *target, obs_data_t *apply_data)
{
	sd_event(SD_EV_DATA_APPLY, sd_id(target), sd_id(apply_data), 0, 0);
}

static void sd_data_release(obs_data_t *data)
{
	sd_event(SD_EV_DATA_RELEASE, sd_id(data), 0, 0, 0);
}

static obs_source_t *sd_create(const char *id, const char *name, obs_data_t *settings, obs_data_t *hotkey_data)
{
	sd_event(SD_EV_CREATE, sd_id(id), sd_id(name), sd_id(settings), sd_id(hotkey_data));
	return sd_answer(SD_A_CREATED) ? &sd_current->sources[SD_CREATED] : NULL;
}

static obs_source_t *sd_create_private(const char *id, const char *name, obs_data_t *settings)
{
	sd_event(SD_EV_CREATE_PRIVATE, sd_id(id), sd_id(name), sd_id(settings), 0);
	return sd_answer(SD_A_CREATED) ? &sd_current->sources[SD_CREATED] : NULL;
}

static int64_t sd_get_sync_offset(const obs_source_t *source)
{
	sd_event(SD_EV_SYNC_OFFSET, sd_id(source), 0, 0, 0);
	return source->sync_offset;
}

static float sd_get_volume(const obs_source_t *source)
{
	sd_event(SD_EV_GET_VOLUME, sd_id(source), 0, 0, 0);
	return source->user_volume;
}

static void sd_duplicate_filters(obs_source_t *dst, obs_source_t *src, bool private)
{
	sd_event(SD_EV_DUPLICATE_FILTERS, sd_id(dst), sd_id(src), private, 0);
}

#define obs_scene_from_source sd_scene_from_source
#define obs_group_from_source sd_group_from_source
#define obs_source_get_ref sd_get_ref
#define obs_scene_duplicate sd_scene_duplicate
#define obs_scene_get_source sd_scene_get_source
#define obs_data_create sd_data_create
#define obs_data_apply sd_data_apply
#define obs_data_release sd_data_release
#define obs_source_create sd_create
#define obs_source_create_private sd_create_private
#define obs_source_get_sync_offset sd_get_sync_offset
#define obs_source_get_volume sd_get_volume
#define obs_source_duplicate_filters sd_duplicate_filters
#include "source-duplicate-entry.inc"
#undef obs_source_duplicate
#include "source_duplicate_entry_fields.inc"
#undef obs_source_duplicate_filters
#undef obs_source_get_volume
#undef obs_source_get_sync_offset
#undef obs_source_create_private
#undef obs_source_create
#undef obs_data_release
#undef obs_data_apply
#undef obs_data_create
#undef obs_scene_get_source
#undef obs_scene_duplicate
#undef obs_source_get_ref
#undef obs_group_from_source
#undef obs_scene_from_source

/* The fields obs_source_duplicate() copies, as the out array holds them. */
#define SD_FIELDS 7

static void sd_fields(const struct obs_source *s, long *out)
{
	uint32_t user_volume;
	uint32_t volume;
	memcpy(&user_volume, &s->user_volume, sizeof(user_volume));
	memcpy(&volume, &s->volume, sizeof(volume));
	out[0] = (long)s->audio_mixers;
	out[1] = (long)(s->sync_offset & 0xffffffff);
	out[2] = (long)((s->sync_offset >> 32) & 0xffffffff);
	out[3] = (long)user_volume;
	out[4] = (long)os_atomic_load_bool(&s->user_muted) | ((long)s->muted << 1);
	out[5] = (long)volume;
	out[6] = (long)s->flags;
}

/* The source's copied fields: each its own value, none the noise. */
static void sd_fill_source(struct obs_source *s, uint8_t noise, unsigned bools)
{
	s->audio_mixers = 0x2du ^ noise;
	s->sync_offset = -123456789 - (int64_t)noise;
	s->user_volume = 0.25f + (float)noise;
	s->user_muted = (bools & 1) != 0;
	s->volume = 0.75f + (float)noise;
	s->muted = (bools & 2) != 0;
	s->flags = 0x5u ^ ((uint32_t)noise << 8);
}

/* Whether before and after differ only inside the copied fields (all bytes
 * compared when copied is false). */
static bool sd_unchanged(const struct obs_source *before, const struct obs_source *after, bool copied)
{
	if (!copied)
		return memcmp(before, after, sizeof(*before)) == 0;
	/* The copied fields taken from after, then every byte compared. */
	struct obs_source *expected = malloc(sizeof(*expected));
	if (!expected)
		return false;
	memcpy(expected, before, sizeof(*expected));
	memcpy(&expected->audio_mixers, &after->audio_mixers, sizeof(after->audio_mixers));
	memcpy(&expected->sync_offset, &after->sync_offset, sizeof(after->sync_offset));
	memcpy(&expected->user_volume, &after->user_volume, sizeof(after->user_volume));
	expected->user_muted = after->user_muted;
	memcpy(&expected->volume, &after->volume, sizeof(after->volume));
	memcpy(&expected->muted, &after->muted, sizeof(after->muted));
	memcpy(&expected->flags, &after->flags, sizeof(after->flags));
	bool same = memcmp(expected, after, sizeof(*after)) == 0;
	free(expected);
	return same;
}

struct sd_log {
	unsigned count;
};
#if defined(__GNUC__) || defined(__clang__)
static void sd_capture(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void sd_capture(int level, const char *format, va_list args, void *param)
{
	struct sd_log *captured = param;
	captured->count++;
	vsnprintf(sd_current->message, sizeof(sd_current->message), format, args);
	sd_event(SD_EV_LOG, level, 0, 0, 0);
}

static void sd_copy(char *dst, size_t size, const char *src)
{
	size_t len = strlen(src);
	if (len >= size)
		len = size - 1;
	memcpy(dst, src, len);
	dst[len] = '\0';
}

typedef obs_source_t *(*sd_duplicate)(obs_source_t *, const char *, bool);
typedef void (*sd_get_log)(log_handler_t *, void **);
typedef void (*sd_set_log)(log_handler_t, void *);

/* The out array, by index. */
enum {
	SD_OUT_RESULT = 0,
	SD_OUT_SOURCE_UNCHANGED,
	SD_OUT_CREATED_UNCHANGED,
	SD_OUT_COPY_UNCHANGED,
	SD_OUT_LOGS,
	SD_OUT_EVENTS,
	SD_OUT_OVERFLOW,
	SD_OUT_FIELDS,
	SD_OUT_HEAD = SD_OUT_FIELDS + SD_FIELDS,
	SD_OUT_COUNT = SD_OUT_HEAD + SD_EVENTS * SD_EVENT_WIDTH,
};

/* Runs obs_source_duplicate() once through `duplicate` (the Rust export or
 * a mutant; NULL for the original) on a fresh noise-filled fixture: the
 * source (NULL when present is false), the source a create returns and the
 * source of a scene copy. `type` and `output_flags` are the source's
 * info.type and info.output_flags, `answers` the SD_A_* bits of what the
 * stand-ins return, `named` whether new_name is a string (else NULL), and
 * `bools` the source's user_muted (bit 0) and muted (bit 1) and the new
 * source's (bits 2 and 3). Writes SD_OUT_COUNT values to out and the last
 * log line to message; false when the fixture could not be built. */
bool oracle_sd_run(sd_duplicate duplicate, sd_get_log get_log, sd_set_log set_log, bool present, uint8_t noise,
		   int type, uint32_t output_flags, unsigned answers, bool named, bool create_private, unsigned bools,
		   long *out, char *message, size_t message_size);
bool oracle_sd_run(sd_duplicate duplicate, sd_get_log get_log, sd_set_log set_log, bool present, uint8_t noise,
		   int type, uint32_t output_flags, unsigned answers, bool named, bool create_private, unsigned bools,
		   long *out, char *message, size_t message_size)
{
	if (message_size == 0)
		return false;
	struct sd_fixture *f = malloc(sizeof(*f));
	if (!f)
		return false;
	memset(f, noise, sizeof(*f));
	sd_current = f;
	struct obs_source *src = &f->sources[SD_SOURCE];
	struct obs_source *created = &f->sources[SD_CREATED];
	src->info.type = (enum obs_source_type)type;
	src->info.output_flags = output_flags;
	src->info.id = f->id;
	src->context.settings = sd_marker(SD_CONTEXT_SETTINGS);
	src->private_settings = sd_marker(SD_SOURCE_PRIVATE);
	sd_fill_source(src, noise, bools);
	created->private_settings = sd_marker(SD_CREATED_PRIVATE);
	created->user_muted = (bools & 4) != 0;
	created->muted = (bools & 8) != 0;
	f->sources[SD_COPY_SOURCE].user_muted = false;
	f->sources[SD_COPY_SOURCE].muted = false;
	f->answers = answers;
	f->event_count = 0;
	f->overflow = false;
	f->message[0] = '\0';
	memcpy(f->before, f->sources, sizeof(f->sources));

	obs_source_t *source = present ? src : NULL;
	const char *new_name = named ? f->name : NULL;
	struct sd_log captured = {0};
	log_handler_t previous;
	void *param;

	get_log(&previous, &param);
	set_log(sd_capture, &captured);
	obs_source_t *result = duplicate ? duplicate(source, new_name, create_private)
					 : oracle_obs_source_duplicate(source, new_name, create_private);
	set_log(previous, param);

	memset(out, 0, sizeof(long) * SD_OUT_COUNT);
	out[SD_OUT_RESULT] = sd_id(result);
	out[SD_OUT_SOURCE_UNCHANGED] = sd_unchanged(&f->before[SD_SOURCE], src, false);
	out[SD_OUT_CREATED_UNCHANGED] = sd_unchanged(&f->before[SD_CREATED], created, true);
	out[SD_OUT_COPY_UNCHANGED] = sd_unchanged(&f->before[SD_COPY_SOURCE], &f->sources[SD_COPY_SOURCE], false);
	out[SD_OUT_LOGS] = (long)captured.count;
	out[SD_OUT_EVENTS] = (long)f->event_count;
	out[SD_OUT_OVERFLOW] = f->overflow;
	sd_fields(created, &out[SD_OUT_FIELDS]);
	for (size_t i = 0; i < f->event_count; i++)
		memcpy(&out[SD_OUT_HEAD + i * SD_EVENT_WIDTH], f->events[i], sizeof(f->events[i]));
	sd_copy(message, message_size, f->message);

	sd_current = NULL;
	free(f);
	return true;
}

/* The copied fields of the fixture's source (`source`) or of a new source
 * nothing was copied into, for `noise` and `bools` (bits 0 and 1 as the
 * source's or the new source's), so the Rust side can predict what a run
 * leaves. Writes SD_FIELDS values. */
void oracle_sd_source_fields(uint8_t noise, unsigned bools, bool source, long *out);
void oracle_sd_source_fields(uint8_t noise, unsigned bools, bool source, long *out)
{
	struct obs_source *s = malloc(sizeof(*s));
	if (!s)
		return;
	memset(s, noise, sizeof(*s));
	if (source) {
		sd_fill_source(s, noise, bools);
	} else {
		s->user_muted = (bools & 1) != 0;
		s->muted = (bools & 2) != 0;
	}
	sd_fields(s, out);
	free(s);
}

/* The sizes the Rust side mirrors: 0 the out array, 1 the event width,
 * 2 the field count. */
size_t oracle_sd_layout(size_t which);
size_t oracle_sd_layout(size_t which)
{
	return which == 0 ? SD_OUT_COUNT : which == 1 ? SD_EVENT_WIDTH : which == 2 ? SD_FIELDS : 0;
}
