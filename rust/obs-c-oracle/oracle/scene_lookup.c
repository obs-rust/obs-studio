/* Original extracted C and production field adapters, over real private layouts. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "obs-internal.h"
#include "obs-scene.h"

/* Record exact lock/unlock order, while still exercising the real mutexes. */
static pthread_mutex_t *lookup_mutexes[8];
static unsigned lookup_trace[32];
static size_t lookup_trace_len;
static int lookup_mutex_lock(pthread_mutex_t *mutex)
{
	for (unsigned i = 0; i < 8; i++)
		if (lookup_mutexes[i] == mutex && lookup_trace_len < 32)
			lookup_trace[lookup_trace_len++] = i + 1;
	return pthread_mutex_lock(mutex);
}
static int lookup_mutex_unlock(pthread_mutex_t *mutex)
{
	for (unsigned i = 0; i < 8; i++)
		if (lookup_mutexes[i] == mutex && lookup_trace_len < 32)
			lookup_trace[lookup_trace_len++] = i + 17;
	return pthread_mutex_unlock(mutex);
}
/* Preserve command-line pthread renames while the extracted C uses probes. */
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock lookup_mutex_lock
#define pthread_mutex_unlock lookup_mutex_unlock
#define obs_scene_find_source oracle_obs_scene_find_source
#define obs_scene_find_source_recursive oracle_obs_scene_find_source_recursive
#define obs_scene_find_sceneitem_by_id oracle_obs_scene_find_sceneitem_by_id
#define obs_scene_get_group oracle_obs_scene_get_group
obs_sceneitem_t *oracle_obs_scene_find_source(obs_scene_t *, const char *);
obs_sceneitem_t *oracle_obs_scene_find_source_recursive(obs_scene_t *, const char *);
obs_sceneitem_t *oracle_obs_scene_find_sceneitem_by_id(obs_scene_t *, int64_t);
obs_sceneitem_t *oracle_obs_scene_get_group(obs_scene_t *, const char *);
#include "scene_lookup.inc"
#undef obs_scene_find_source
#undef obs_scene_find_source_recursive
#undef obs_scene_find_sceneitem_by_id
#undef obs_scene_get_group
#include "scene_lookup_fields.inc"
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")

typedef obs_sceneitem_t *(*name_lookup)(obs_scene_t *, const char *);
typedef obs_sceneitem_t *(*id_lookup)(obs_scene_t *, int64_t);
unsigned oracle_scene_lookup_probe(uint32_t seed, int64_t id, name_lookup direct, name_lookup recursive,
				   id_lookup by_id, name_lookup group);
unsigned oracle_scene_lookup_probe(uint32_t seed, int64_t id, name_lookup direct, name_lookup recursive,
				   id_lookup by_id, name_lookup group)
{
	struct obs_scene scenes[4] = {0};
	struct obs_scene_item items[12] = {0};
	struct obs_source sources[12] = {0};
	char *names[] = {"", "a", "b", "group", "deep", "\xff", "missing"};
	for (unsigned i = 0; i < 4; i++) {
		if (pthread_mutex_init(&scenes[i].video_mutex, NULL) ||
		    pthread_mutex_init(&scenes[i].audio_mutex, NULL))
			return 128;
		lookup_mutexes[2 * i] = &scenes[i].video_mutex;
		lookup_mutexes[2 * i + 1] = &scenes[i].audio_mutex;
	}
	for (unsigned i = 0; i < 12; i++) {
		items[i].source = &sources[i];
		sources[i].context.name = names[(seed >> (2 * (i % 12))) % 6];
		items[i].id = (i & 1) ? id : (int64_t)((seed >> i) % 4);
		items[i].is_group = ((seed >> i) & 1) != 0;
		sources[i].context.data = &scenes[i < 4 ? 1 + i % 2 : 3];
		items[i].removed = (i & 1) != 0;
	}
	/* Rotate each ordered list, keeping trees finite. */
	for (unsigned s = 0; s < 3; s++) {
		unsigned count = s == 2 ? 3 : 4;
		unsigned start = s * 4;
		unsigned offset = (seed >> (s * 4)) % count;
		scenes[s].first_item = &items[start + offset];
		for (unsigned j = 0; j + 1 < count; j++)
			items[start + (offset + j) % count].next = &items[start + (offset + j + 1) % count];
	}
	scenes[3].first_item = &items[11];
	sources[11].context.name = names[4];
	if (seed & (1U << 31))
		scenes[0].first_item = NULL;
	if (seed & (1U << 30))
		sources[0].context.data = NULL;
	unsigned char before[sizeof(items) + sizeof(sources)];
	memcpy(before, items, sizeof(items));
	memcpy(before + sizeof(items), sources, sizeof(sources));
	name_lookup actual[] = {direct, recursive, group};
	name_lookup expected[] = {oracle_obs_scene_find_source, oracle_obs_scene_find_source_recursive,
				  oracle_obs_scene_get_group};
	unsigned errors = 0;
	unsigned trace[32];
	for (unsigned f = 0; f < 3; f++) {
		for (unsigned n = 0; n < 7; n++) {
			lookup_trace_len = 0;
			obs_sceneitem_t *want = expected[f](&scenes[0], names[n]);
			size_t len = lookup_trace_len;
			memcpy(trace, lookup_trace, len * sizeof(*trace));
			lookup_trace_len = 0;
			if (actual[f](&scenes[0], names[n]) != want)
				errors |= 1U << f;
			if (len != lookup_trace_len || memcmp(trace, lookup_trace, len * sizeof(*trace)))
				errors |= 16;
		}
		lookup_trace_len = 0;
		if (actual[f](NULL, NULL) || lookup_trace_len)
			errors |= 32;
	}
	int64_t ids[] = {id, 0, 1, -1, INT64_MIN, INT64_MAX};
	for (unsigned i = 0; i < 6; i++) {
		lookup_trace_len = 0;
		obs_sceneitem_t *want = oracle_obs_scene_find_sceneitem_by_id(&scenes[0], ids[i]);
		size_t len = lookup_trace_len;
		memcpy(trace, lookup_trace, len * sizeof(*trace));
		lookup_trace_len = 0;
		if (by_id(&scenes[0], ids[i]) != want)
			errors |= 8;
		if (len != lookup_trace_len || memcmp(trace, lookup_trace, len * sizeof(*trace)))
			errors |= 16;
	}
	if (by_id(NULL, id) || group(&scenes[0], NULL))
		errors |= 32;
	if (memcmp(before, items, sizeof(items)) || memcmp(before + sizeof(items), sources, sizeof(sources)))
		errors |= 64;
	/* Only group lookup permits null stored names. */
	sources[0].context.name = NULL;
	if (group(&scenes[0], "a") != oracle_obs_scene_get_group(&scenes[0], "a"))
		errors |= 4;
	for (unsigned i = 0; i < 4; i++) {
		pthread_mutex_destroy(&scenes[i].audio_mutex);
		pthread_mutex_destroy(&scenes[i].video_mutex);
	}
	return errors;
}
