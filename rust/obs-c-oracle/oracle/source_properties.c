/* Compare the unchanged extracted C constructors through the real private layout. */
#include "obs-internal.h"
#include <stddef.h>
#include <string.h>

static struct obs_source_info sp_info;
static bool sp_null;
static int sp_token, sp_instance;
struct sp_trace {
	unsigned legacy, modern, defaults, defaults2, applied;
	bool data_ok, type_ok, result, property;
	long long value;
};
static struct sp_trace sp_trace;

/* Registry stand-in for both implementations; never linked into production. */
struct obs_source_info *get_source_info(const char *id)
{
	return id && strcmp(id, "valid") == 0 ? &sp_info : NULL;
}

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
	obs_data_set_default_int(settings, "value", 17);
}
static void sp_defaults2(void *type_data, obs_data_t *settings)
{
	sp_trace.defaults2++;
	sp_trace.type_ok = sp_trace.type_ok && type_data == &sp_token;
	obs_data_set_default_int(settings, "value", 29);
}
static bool sp_modified(obs_properties_t *props, obs_property_t *property, obs_data_t *settings)
{
	(void)props;
	(void)property;
	sp_trace.applied++;
	sp_trace.value = obs_data_get_int(settings, "value");
	return false;
}
static obs_properties_t *sp_properties(void)
{
	if (sp_null)
		return NULL;
	obs_properties_t *props = obs_properties_create();
	obs_property_t *property = obs_properties_add_int(props, "value", "Value", 0, 100, 1);
	obs_property_set_modified_callback(property, sp_modified);
	return props;
}
static void *sp_expected_data;
static obs_properties_t *sp_legacy(void *data)
{
	sp_trace.legacy++;
	sp_trace.data_ok = data == sp_expected_data;
	return sp_properties();
}
static obs_properties_t *sp_modern(void *data, void *type_data)
{
	sp_trace.modern++;
	sp_trace.data_ok = data == sp_expected_data;
	sp_trace.type_ok = sp_trace.type_ok && type_data == &sp_token;
	return sp_properties();
}

typedef obs_properties_t *(*sp_type_fn)(const char *);
typedef obs_properties_t *(*sp_instance_fn)(const obs_source_t *);
bool oracle_source_properties_check(sp_type_fn type_fn, sp_instance_fn instance_fn, unsigned flags, long long value);
bool oracle_source_properties_check(sp_type_fn type_fn, sp_instance_fn instance_fn, unsigned flags, long long value)
{
	sp_info = (struct obs_source_info){0};
	sp_info.get_properties = flags & 1 ? sp_legacy : NULL;
	sp_info.get_properties2 = flags & 2 ? sp_modern : NULL;
	sp_info.get_defaults = flags & 4 ? sp_defaults : NULL;
	sp_info.get_defaults2 = flags & 8 ? sp_defaults2 : NULL;
	sp_info.type_data = &sp_token;
	sp_null = (flags & 16) != 0;
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "value", value);
	struct obs_source source = {0};
	source.info = sp_info;
	source.context.data = flags & 32 ? NULL : &sp_instance;
	source.context.settings = settings;
	bool equal = true;
	for (unsigned query = 0; query < 5; query++) {
		struct sp_trace traces[2];
		for (unsigned implementation = 0; implementation < 2; implementation++) {
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
		}
		equal = equal && memcmp(&traces[0], &traces[1], sizeof(traces[0])) == 0;
	}
	obs_data_release(settings);
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
