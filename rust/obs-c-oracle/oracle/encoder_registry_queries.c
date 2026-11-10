/* Real-header fixture, original lookup and unchanged C query definitions. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <string.h>

struct obs_encoder_info *encoder_registry_original_find(const char *id);
enum obs_encoder_type oracle_obs_get_encoder_type(const char *id);
uint32_t oracle_obs_get_encoder_caps(const char *id);

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *encoder_registry_core;
static __declspec(thread) size_t encoder_registry_lookups;
static __declspec(thread) const char *encoder_registry_last_id;
#else
static _Thread_local struct obs_core *encoder_registry_core;
static _Thread_local size_t encoder_registry_lookups;
static _Thread_local const char *encoder_registry_last_id;
#endif

#define obs encoder_registry_core
#define find_encoder encoder_registry_original_find
#include "encoder_registry_find.inc"
#undef find_encoder
#undef obs

/* Instrument, then execute the original lookup; never a placeholder stub. */
struct obs_encoder_info *find_encoder(const char *id)
{
	++encoder_registry_lookups;
	encoder_registry_last_id = id;
	return encoder_registry_original_find(id);
}

#define obs_get_encoder_type oracle_obs_get_encoder_type
#define obs_get_encoder_caps oracle_obs_get_encoder_caps
#include "libobs/obs-encoder-registry-queries.c.h"
#undef obs_get_encoder_type
#undef obs_get_encoder_caps
#include "encoder_registry_query_fields.inc"

typedef enum obs_encoder_type (*registry_type_query)(const char *);
typedef uint32_t (*registry_caps_query)(const char *);

bool oracle_encoder_registry_check(const char *const *ids, const int *kinds, const uint32_t *caps, size_t count,
				   const char *needle, registry_type_query get_type, registry_caps_query get_caps);
bool oracle_encoder_registry_check(const char *const *ids, const int *kinds, const uint32_t *caps, size_t count,
				   const char *needle, registry_type_query get_type, registry_caps_query get_caps)
{
	struct obs_core core = {0};
	struct obs_core before_core;
	struct obs_encoder_info entries[8] = {0};
	struct obs_encoder_info before[8];
	if (count > 8)
		return false;
	for (size_t i = 0; i < count; ++i) {
		entries[i].id = ids[i];
		entries[i].type = (enum obs_encoder_type)kinds[i];
		entries[i].caps = caps[i];
		entries[i].codec = "untouched codec";
		entries[i].type_data = &entries[i];
	}
	core.encoder_types.array = entries;
	core.encoder_types.num = count;
	core.encoder_types.capacity = 8;
	memcpy(before, entries, sizeof(entries));
	memcpy(&before_core, &core, sizeof(core));
	encoder_registry_core = &core;
	encoder_registry_lookups = 0;
	enum obs_encoder_type type = get_type(needle);
	bool valid = encoder_registry_lookups == 1 && encoder_registry_last_id == needle;
	encoder_registry_lookups = 0;
	uint32_t bits = get_caps(needle);
	valid = valid && encoder_registry_lookups == 1 && encoder_registry_last_id == needle;
	valid = valid && type == oracle_obs_get_encoder_type(needle) && bits == oracle_obs_get_encoder_caps(needle);
	for (size_t i = 0; i < count; ++i) {
		struct obs_encoder_registry_query_fields fields = obs_encoder_registry_query_fields(&entries[i]);
		valid = valid && fields.kind == &entries[i].type && fields.caps == &entries[i].caps;
	}
	valid = valid && memcmp(entries, before, sizeof(entries)) == 0 &&
		memcmp(&core, &before_core, sizeof(core)) == 0;
	encoder_registry_core = NULL;
	return valid;
}

void oracle_encoder_registry_layout(size_t *layout);
void oracle_encoder_registry_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_encoder_registry_query_fields);
	layout[1] = _Alignof(struct obs_encoder_registry_query_fields);
	layout[2] = offsetof(struct obs_encoder_registry_query_fields, kind);
	layout[3] = offsetof(struct obs_encoder_registry_query_fields, caps);
	layout[4] = sizeof(enum obs_encoder_type);
	layout[5] = _Alignof(enum obs_encoder_type);
	layout[6] = OBS_ENCODER_AUDIO;
	layout[7] = OBS_ENCODER_VIDEO;
	layout[8] = OBS_ENCODER_CAP_DYN_BITRATE;
}
