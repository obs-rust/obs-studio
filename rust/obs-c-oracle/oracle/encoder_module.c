/* Original extracted C, real-header fixtures, and real private adapters. */
#include "obs-internal.h"
#include <stdlib.h>
#include <string.h>

static struct obs_core encoder_module_core;
#define obs (&encoder_module_core)
#define obs_encoder_get_module oracle_obs_encoder_get_module
#define obs_encoder_load_state oracle_obs_encoder_load_state
obs_module_t *oracle_obs_encoder_get_module(const char *id);
enum obs_module_load_state oracle_obs_encoder_load_state(const char *id);
#include "libobs/obs-encoder-module.c.h"
#undef obs_encoder_get_module
#undef obs_encoder_load_state
#include "encoder_module_fields.inc"
#include "libobs/obs-encoder-module-roots.c.h"
#undef obs

typedef obs_module_t *(*encoder_module_getter)(const char *);
typedef enum obs_module_load_state (*encoder_module_state)(const char *);
void oracle_encoder_module_run(encoder_module_getter get, encoder_module_state state, size_t count, size_t split,
			       const size_t *counts, char **ids, const int *states, const char *query, size_t *out);
void oracle_encoder_module_run(encoder_module_getter get, encoder_module_state state, size_t count, size_t split,
			       const size_t *counts, char **ids, const int *states, const char *query, size_t *out)
{
	struct obs_module *modules = calloc(count ? count : 1, sizeof(*modules));
	struct obs_module *before = calloc(count ? count : 1, sizeof(*before));
	if (!modules || !before)
		abort();
	char *other_ids[] = {"source-only", "output-only", "service-only"};
	size_t offset = 0;
	for (size_t i = 0; i < count; i++) {
		modules[i].sources.array = &other_ids[0];
		modules[i].sources.num = modules[i].sources.capacity = 1;
		modules[i].outputs.array = &other_ids[1];
		modules[i].outputs.num = modules[i].outputs.capacity = 1;
		modules[i].services.array = &other_ids[2];
		modules[i].services.num = modules[i].services.capacity = 1;
		modules[i].encoders.array = counts[i] ? ids + offset : NULL;
		modules[i].encoders.num = counts[i];
		modules[i].encoders.capacity = counts[i];
		modules[i].load_state = (enum obs_module_load_state)states[i];
		modules[i].next = i + 1 < count && i + 1 != split ? &modules[i + 1] : NULL;
		offset += counts[i];
	}
	memcpy(before, modules, count * sizeof(*modules));
	encoder_module_core.first_module = split ? modules : NULL;
	encoder_module_core.first_disabled_module = split < count ? &modules[split] : NULL;
	/* Both NULL queries with populated encoder IDs are C UB: do not execute them. */
	for (size_t repeat = 0; repeat < 3; repeat++) {
		obs_module_t *found = query || !offset ? get(query) : NULL;
		out[repeat * 3] = found ? count + 1 : 0;
		for (size_t i = 0; i < count; i++) {
			if (found == &modules[i])
				out[repeat * 3] = i + 1;
		}
		out[repeat * 3 + 1] = query || !offset ? (size_t)state(query) : OBS_MODULE_MISSING;
		out[repeat * 3 + 2] = memcmp(before, modules, count * sizeof(*modules)) == 0;
	}
	encoder_module_core.first_module = NULL;
	encoder_module_core.first_disabled_module = NULL;
	free(before);
	free(modules);
}

void oracle_encoder_module_layout(size_t *out);
void oracle_encoder_module_layout(size_t *out)
{
	out[0] = sizeof(enum obs_module_load_state);
	out[1] = _Alignof(enum obs_module_load_state);
	const enum obs_module_load_state states[] = {OBS_MODULE_INVALID,
						     OBS_MODULE_ENABLED,
						     OBS_MODULE_MISSING,
						     OBS_MODULE_DISABLED,
						     OBS_MODULE_DISABLED_SAFE,
						     OBS_MODULE_FAILED_TO_OPEN,
						     OBS_MODULE_FAILED_TO_INITIALIZE};
	for (size_t i = 0; i < 7; i++)
		out[i + 2] = (size_t)states[i];
}

/* No strcmp is reached: even a non-string query pointer must remain unread. */
bool oracle_encoder_module_unread_query(encoder_module_getter get, encoder_module_state state);
bool oracle_encoder_module_unread_query(encoder_module_getter get, encoder_module_state state)
{
	struct obs_module enabled = {0};
	struct obs_module disabled = {0};
	const char *query = (const char *)(uintptr_t)1;
	bool result = get(query) == NULL && state(query) == OBS_MODULE_MISSING && get(NULL) == NULL &&
		      state(NULL) == OBS_MODULE_MISSING;
	encoder_module_core.first_module = &enabled;
	encoder_module_core.first_disabled_module = &disabled;
	result = result && get(query) == NULL && state(query) == OBS_MODULE_MISSING && get(NULL) == NULL &&
		 state(NULL) == OBS_MODULE_MISSING;
	encoder_module_core.first_module = NULL;
	encoder_module_core.first_disabled_module = NULL;
	return result;
}
