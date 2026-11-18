/* Compare the unchanged extracted C constructors through the real private layout. */
#include "obs-internal.h"
#include <stddef.h>
#include <string.h>

static struct obs_source_info sp_info;
static bool sp_null;
static int sp_token, sp_instance;
static bool sp_replace;
static obs_source_t *sp_source;
static void *sp_expected_type;
static void *sp_expected_data;
struct sp_trace {
	unsigned legacy, modern, defaults, defaults2, applied;
	unsigned order;
	bool data_ok, type_ok, result, property;
	long long value;
};
static struct sp_trace sp_trace;

/* Use the same per-thread real registry adapter as the other source oracles. */
struct obs_core *oracle_source_registry_exchange_core(struct obs_core *core);

obs_properties_t *oracle_obs_get_source_properties(const char *id);
obs_properties_t *oracle_obs_source_properties(const obs_source_t *source);

#define obs_get_source_properties oracle_obs_get_source_properties
#define obs_source_properties oracle_obs_source_properties
#include "obs-source-properties.c.h"
#undef obs_get_source_properties
#undef obs_source_properties
#include "source_properties_fields.inc"

static void sp_defaults(obs_data_t *settings)
{
	sp_trace.defaults++;
	sp_trace.order = sp_trace.order * 10 + 1;
	obs_data_set_default_int(settings, "value", 17);
}
static void sp_defaults2(void *type_data, obs_data_t *settings)
{
	sp_trace.defaults2++;
	sp_trace.order = sp_trace.order * 10 + 2;
	sp_trace.type_ok = sp_trace.type_ok && type_data == sp_expected_type;
	obs_data_set_default_int(settings, "value", 29);
}
static bool sp_modified(obs_properties_t *props, obs_property_t *property, obs_data_t *settings)
{
	(void)props;
	(void)property;
	sp_trace.applied++;
	sp_trace.order = sp_trace.order * 10 + 5;
	sp_trace.value = obs_data_get_int(settings, "value");
	return false;
}
static obs_properties_t *sp_properties(void)
{
	/* The original reads the live settings field after the constructor returns. */
	if (sp_replace && sp_expected_data) {
		long long value = obs_data_get_int(sp_source->context.settings, "value");
		obs_data_release(sp_source->context.settings);
		sp_source->context.settings = obs_data_create();
		obs_data_set_int(sp_source->context.settings, "value", ~value);
	}
	if (sp_null)
		return NULL;
	obs_properties_t *props = obs_properties_create();
	obs_property_t *property = obs_properties_add_int(props, "value", "Value", 0, 100, 1);
	obs_property_set_modified_callback(property, sp_modified);
	return props;
}
static obs_properties_t *sp_legacy(void *data)
{
	sp_trace.legacy++;
	sp_trace.order = sp_trace.order * 10 + 3;
	sp_trace.data_ok = data == sp_expected_data;
	return sp_properties();
}
static obs_properties_t *sp_modern(void *data, void *type_data)
{
	sp_trace.modern++;
	sp_trace.order = sp_trace.order * 10 + 4;
	sp_trace.data_ok = data == sp_expected_data;
	sp_trace.type_ok = sp_trace.type_ok && type_data == sp_expected_type;
	return sp_properties();
}

typedef obs_properties_t *(*sp_type_fn)(const char *);
typedef obs_properties_t *(*sp_instance_fn)(const obs_source_t *);
bool oracle_source_properties_check(sp_type_fn type_fn, sp_instance_fn instance_fn, unsigned flags, long long value);
bool oracle_source_properties_check(sp_type_fn type_fn, sp_instance_fn instance_fn, unsigned flags, long long value)
{
	sp_info = (struct obs_source_info){0};
	sp_info.id = "valid";
	sp_info.get_properties = flags & 1 ? sp_legacy : NULL;
	sp_info.get_properties2 = flags & 2 ? sp_modern : NULL;
	sp_info.get_defaults = flags & 4 ? sp_defaults : NULL;
	sp_info.get_defaults2 = flags & 8 ? sp_defaults2 : NULL;
	sp_expected_type = flags & 128 ? NULL : &sp_token;
	sp_info.type_data = sp_expected_type;
	sp_null = (flags & 16) != 0;
	sp_replace = (flags & 64) != 0;
	struct obs_source source = {0};
	sp_source = &source;
	source.info = sp_info;
	source.context.data = flags & 32 ? NULL : &sp_instance;
	struct obs_core registry = {0};
	registry.source_types.array = &sp_info;
	registry.source_types.num = registry.source_types.capacity = 1;
	struct obs_core *previous = oracle_source_registry_exchange_core(&registry);
	bool equal = true;
	for (unsigned query = 0; query < 5; query++) {
		struct sp_trace traces[2];
		for (unsigned implementation = 0; implementation < 2; implementation++) {
			source.context.settings = obs_data_create();
			obs_data_set_int(source.context.settings, "value", value);
			/* Keep the original object alive across replacement so an early
			 * settings snapshot cannot pass merely through allocator reuse. */
			obs_data_t *original_settings = source.context.settings;
			obs_data_addref(original_settings);
			memset(&sp_trace, 0, sizeof(sp_trace));
			sp_trace.type_ok = true;
			sp_expected_data = query == 0 ? source.context.data : NULL;
			obs_properties_t *props;
			if (query == 0 || query == 3) {
				const obs_source_t *input = query == 3 ? NULL : &source;
				props = implementation ? instance_fn(input) : oracle_obs_source_properties(input);
			} else {
				const char *id = query == 1 ? "valid" : query == 2 ? "unknown" : NULL;
				props = implementation ? type_fn(id) : oracle_obs_get_source_properties(id);
			}
			sp_trace.result = props != NULL;
			sp_trace.property = props && obs_properties_get(props, "value");
			traces[implementation] = sp_trace;
			obs_properties_destroy(props);
			/* Still caller-owned after properties borrowed it. */
			long long expected = sp_replace && query == 0 && source.context.data &&
							     (sp_info.get_properties || sp_info.get_properties2)
						     ? ~value
						     : value;
			equal = equal && obs_data_get_int(source.context.settings, "value") == expected;
			obs_data_release(source.context.settings);
			obs_data_release(original_settings);
		}
		const struct sp_trace *a = &traces[0], *b = &traces[1];
		equal = equal && a->legacy == b->legacy && a->modern == b->modern && a->defaults == b->defaults &&
			a->defaults2 == b->defaults2 && a->applied == b->applied && a->order == b->order &&
			a->data_ok == b->data_ok && a->type_ok == b->type_ok && a->result == b->result &&
			a->property == b->property && a->value == b->value;
	}
	oracle_source_registry_exchange_core(previous);
	sp_source = NULL;
	return equal;
}

void oracle_source_properties_layout(size_t *layout);
void oracle_source_properties_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_properties_fields);
	layout[1] = _Alignof(struct obs_source_properties_fields);
	layout[2] = offsetof(struct obs_source_properties_fields, legacy);
	layout[3] = offsetof(struct obs_source_properties_fields, modern);
	layout[4] = offsetof(struct obs_source_properties_fields, defaults);
	layout[5] = offsetof(struct obs_source_properties_fields, defaults2);
	layout[6] = offsetof(struct obs_source_properties_fields, type_data);
	layout[7] = offsetof(struct obs_source_properties_fields, data);
	layout[8] = offsetof(struct obs_source_properties_fields, settings);
}
