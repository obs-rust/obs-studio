/* Tier 3: the unchanged extracted obs_get_source_defaults() and
 * obs_source_settings() against the Rust exports, over the real registry
 * adapter and real obs_data objects. The Rust side reads the callbacks through
 * obs_source_properties_info_fields(), defined once in source_properties.c. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <string.h>
#define obs_get_source_defaults oracle_obs_get_source_defaults
#define obs_source_settings oracle_obs_source_settings
#define source_get_defaults oracle_source_defaults_get
#include "libobs/build/libobs.pch.h"
#include "source-defaults.inc"
#undef source_get_defaults
#undef obs_source_settings
#undef obs_get_source_defaults

/* Use the same per-thread real registry adapter as the other source oracles. */
struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core);

struct source_defaults_trace {
	unsigned legacy, modern;
	obs_data_t *seen;
	void *type_data;
	bool empty_at_call;
};

static struct source_defaults_trace source_defaults_seen;
static int source_defaults_token;

static bool source_defaults_empty(obs_data_t *settings)
{
	obs_data_item_t *item = obs_data_first(settings);
	if (!item)
		return true;
	obs_data_item_release(&item);
	return false;
}

static void source_defaults_legacy(obs_data_t *settings)
{
	source_defaults_seen.legacy++;
	source_defaults_seen.seen = settings;
	source_defaults_seen.empty_at_call = source_defaults_empty(settings);
	obs_data_set_default_int(settings, "which", 1);
}

static void source_defaults_modern(void *type_data, obs_data_t *settings)
{
	source_defaults_seen.modern++;
	source_defaults_seen.seen = settings;
	source_defaults_seen.type_data = type_data;
	source_defaults_seen.empty_at_call = source_defaults_empty(settings);
	obs_data_set_default_int(settings, "which", 2);
}

typedef obs_data_t *(*source_defaults_proc)(const char *);

struct source_defaults_result {
	struct source_defaults_trace trace;
	bool returned, same_object, empty, has_which;
	long long which;
};

static struct source_defaults_result source_defaults_run(source_defaults_proc proc, const char *id)
{
	struct source_defaults_result result = {0};
	memset(&source_defaults_seen, 0, sizeof(source_defaults_seen));
	obs_data_t *settings = proc(id);
	result.trace = source_defaults_seen;
	result.returned = settings != NULL;
	if (settings) {
		result.same_object = source_defaults_seen.seen == NULL || source_defaults_seen.seen == settings;
		result.empty = source_defaults_empty(settings);
		result.has_which = obs_data_has_default_value(settings, "which");
		result.which = obs_data_get_int(settings, "which");
		/* The caller owns the only reference: one release frees it, and a
		 * second object is independent of the first. */
		obs_data_t *other = proc(id);
		result.same_object = result.same_object && other != settings;
		obs_data_set_int(settings, "which", 7);
		result.same_object = result.same_object && obs_data_get_int(other, "which") == result.which;
		obs_data_release(other);
		obs_data_release(settings);
	}
	result.trace.seen = NULL;
	return result;
}

static bool source_defaults_equal(const struct source_defaults_result *a, const struct source_defaults_result *b)
{
	return a->trace.legacy == b->trace.legacy && a->trace.modern == b->trace.modern &&
	       a->trace.type_data == b->trace.type_data && a->trace.empty_at_call == b->trace.empty_at_call &&
	       a->returned == b->returned && a->same_object == b->same_object && a->empty == b->empty &&
	       a->has_which == b->has_which && a->which == b->which;
}

/* flags: 1 get_defaults, 2 get_defaults2, 4 NULL type data, 8 a second type
 * registered first. Each query (the registered ID, a near miss, an empty ID)
 * runs through both functions of both sides. */
bool oracle_source_defaults_check(source_defaults_proc rust_defaults, source_defaults_proc rust_settings,
				  unsigned flags);
bool oracle_source_defaults_check(source_defaults_proc rust_defaults, source_defaults_proc rust_settings,
				  unsigned flags)
{
	struct obs_source_info infos[2] = {0};
	infos[0].id = "source_defaults_other";
	/* A decoy with the other callback and its own type data: a lookup that
	 * picks it shows in the trace. */
	infos[0].get_defaults2 = source_defaults_modern;
	infos[0].type_data = &infos[0];
	infos[1].id = "source_defaults";
	infos[1].get_defaults = flags & 1 ? source_defaults_legacy : NULL;
	infos[1].get_defaults2 = flags & 2 ? source_defaults_modern : NULL;
	infos[1].type_data = flags & 4 ? NULL : &source_defaults_token;
	struct obs_core registry = {0};
	registry.source_types.array = flags & 8 ? infos : &infos[1];
	registry.source_types.num = registry.source_types.capacity = flags & 8 ? 2 : 1;
	struct obs_core *previous = oracle_source_registry_exchange_core(&registry);

	static const char *const ids[] = {"source_defaults", "source_defaults ", "", "source_default"};
	bool equal = true;
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
		struct source_defaults_result c = source_defaults_run(oracle_obs_get_source_defaults, ids[i]);
		struct source_defaults_result r = source_defaults_run(rust_defaults, ids[i]);
		struct source_defaults_result cs = source_defaults_run(oracle_obs_source_settings, ids[i]);
		struct source_defaults_result rs = source_defaults_run(rust_settings, ids[i]);
		equal = equal && source_defaults_equal(&c, &r) && source_defaults_equal(&cs, &rs) &&
			source_defaults_equal(&c, &cs);
	}
	oracle_source_registry_exchange_core(previous);
	return equal;
}
