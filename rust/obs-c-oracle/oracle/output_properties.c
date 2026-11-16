/* The original C remains the oracle; the real adapter sees the private layout. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_output_defaults oracle_obs_output_defaults
#define obs_get_output_properties oracle_obs_get_output_properties
#define obs_output_properties oracle_obs_output_properties
#include "obs-output-properties.c.h"
#undef obs_output_defaults
#undef obs_get_output_properties
#undef obs_output_properties
#include "output_properties_fields.inc"

static struct obs_output_info properties_info;
static unsigned properties_trace[6];
static int properties_token;
static bool properties_null_result;
struct obs_core *oracle_output_type_exchange_core(struct obs_core *core);
static void properties_defaults(obs_data_t *settings)
{
	properties_trace[0]++;
	properties_trace[5] = properties_trace[5] * 10 + 1;
	obs_data_set_default_int(settings, "value", 17);
}
static bool properties_modified(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	(void)props;
	(void)p;
	properties_trace[3]++;
	properties_trace[5] = properties_trace[5] * 10 + 3;
	properties_trace[4] = (unsigned)obs_data_get_int(settings, "value");
	return false;
}
static obs_properties_t *properties_callback(void *data)
{
	properties_trace[1]++;
	properties_trace[5] = properties_trace[5] * 10 + 2;
	properties_trace[2] = data == &properties_token ? 1 : data == NULL ? 2 : 3;
	if (properties_null_result)
		return NULL;
	obs_properties_t *props = obs_properties_create();
	obs_property_t *p = obs_properties_add_int(props, "value", "Value", 0, 100, 1);
	obs_property_set_modified_callback(p, properties_modified);
	return props;
}
typedef obs_data_t *(*properties_defaults_fn)(const char *);
typedef obs_properties_t *(*properties_type_fn)(const char *);
typedef obs_properties_t *(*properties_instance_fn)(const obs_output_t *);
struct properties_result {
	unsigned trace[6];
	bool present;
	long long value;
	long leaks;
};
static struct properties_result properties_run(unsigned mask, unsigned op, unsigned id_kind, int value,
					       properties_defaults_fn defaults_fn, properties_type_fn type_fn,
					       properties_instance_fn instance_fn)
{
	struct properties_result result = {0};
	long before = bnum_allocs();
	properties_info = (struct obs_output_info){0};
	properties_info.id = "properties";
	properties_info.get_defaults = mask & 1 ? properties_defaults : NULL;
	properties_info.get_properties = mask & 2 ? properties_callback : NULL;
	properties_null_result = (mask & 4) != 0;
	struct obs_core core = {0};
	core.output_types.array = &properties_info;
	core.output_types.num = core.output_types.capacity = 1;
	struct obs_core *previous = oracle_output_type_exchange_core(&core);
	struct obs_output output = {0};
	output.info = properties_info;
	output.context.data = mask & 8 ? NULL : &properties_token;
	output.context.settings = obs_data_create();
	obs_data_set_int(output.context.settings, "value", value);
	memset(properties_trace, 0, sizeof(properties_trace));
	const char *id = id_kind == 0 ? "properties" : id_kind == 1 ? "missing" : NULL;
	if (op == 0) {
		obs_data_t *data = defaults_fn(id);
		result.present = data != NULL;
		if (data)
			result.value = obs_data_get_int(data, "value");
		obs_data_release(data);
	} else {
		obs_properties_t *props = op == 1 ? type_fn(id) : instance_fn(op == 3 ? NULL : &output);
		result.present = props != NULL;
		obs_properties_destroy(props);
	}
	memcpy(result.trace, properties_trace, sizeof(properties_trace));
	obs_data_release(output.context.settings);
	result.leaks = bnum_allocs() - before;
	oracle_output_type_exchange_core(previous);
	return result;
}
bool oracle_output_properties_compare(unsigned mask, unsigned op, unsigned id_kind, int value,
				      properties_defaults_fn defaults_fn, properties_type_fn type_fn,
				      properties_instance_fn instance_fn);
bool oracle_output_properties_compare(unsigned mask, unsigned op, unsigned id_kind, int value,
				      properties_defaults_fn defaults_fn, properties_type_fn type_fn,
				      properties_instance_fn instance_fn)
{
	struct properties_result c = properties_run(mask, op, id_kind, value, oracle_obs_output_defaults,
						    oracle_obs_get_output_properties, oracle_obs_output_properties);
	struct properties_result r = properties_run(mask, op, id_kind, value, defaults_fn, type_fn, instance_fn);
	return memcmp(c.trace, r.trace, sizeof(c.trace)) == 0 && c.present == r.present && c.value == r.value &&
	       c.leaks == 0 && r.leaks == 0;
}
void oracle_output_properties_layout(size_t *values);
void oracle_output_properties_layout(size_t *values)
{
	values[0] = sizeof(struct obs_output_properties_fields);
	values[1] = _Alignof(struct obs_output_properties_fields);
	values[2] = offsetof(struct obs_output_properties_fields, defaults);
	values[3] = offsetof(struct obs_output_properties_fields, properties);
	values[4] = offsetof(struct obs_output_properties_fields, data);
	values[5] = offsetof(struct obs_output_properties_fields, settings);
}
