/* Direct characterization of the original output properties callback contract. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <obs-module.h>

static int token;
static void *seen_data;
static long long seen_value;
static unsigned defaults_calls, properties_calls, apply_calls;
static const char *name(void *data) { (void)data; return "properties trace"; }
static void *create(obs_data_t *settings, obs_output_t *output)
{ (void)settings; (void)output; return &token; }
static void destroy(void *data) { assert_ptr_equal(data, &token); }
static bool start(void *data) { (void)data; return false; }
static void trace_stop(void *data, uint64_t ts) { (void)data; (void)ts; }
static void packet(void *data, struct encoder_packet *p) { (void)data; (void)p; }
static void defaults(obs_data_t *settings)
{ defaults_calls++; obs_data_set_default_int(settings, "value", 17); }
static bool modified(obs_properties_t *props, obs_property_t *prop, obs_data_t *settings)
{ (void)props; (void)prop; apply_calls++; seen_value = obs_data_get_int(settings, "value"); return false; }
static obs_properties_t *properties(void *data)
{
	properties_calls++;
	seen_data = data;
	obs_properties_t *props = obs_properties_create();
	obs_property_t *p = obs_properties_add_int(props, "value", "Value", 0, 100, 1);
	obs_property_set_modified_callback(p, modified);
	return props;
}
static void trace(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_output_info info = {.id = "properties_trace", .flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED,
		.get_name = name, .create = create, .destroy = destroy, .start = start, .stop = trace_stop,
		.encoded_packet = packet, .get_defaults = defaults, .get_properties = properties};
	obs_register_output(&info);
	obs_properties_t *props = obs_get_output_properties(info.id);
	assert_non_null(props);
	assert_null(seen_data);
	assert_int_equal(defaults_calls, 1);
	assert_int_equal(properties_calls, 1);
	assert_int_equal(apply_calls, 1);
	assert_int_equal(seen_value, 17);
	obs_properties_destroy(props);
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "value", 43);
	obs_output_t *output = obs_output_create(info.id, "trace", settings, NULL);
	assert_non_null(output);
	unsigned before = defaults_calls;
	props = obs_output_properties(output);
	assert_non_null(props);
	assert_ptr_equal(seen_data, &token);
	assert_int_equal(defaults_calls, before);
	assert_int_equal(properties_calls, 2);
	assert_int_equal(apply_calls, 2);
	assert_int_equal(seen_value, 43);
	obs_properties_destroy(props);
	obs_output_release(output);
	obs_data_release(settings);
	obs_shutdown();
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(trace)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
