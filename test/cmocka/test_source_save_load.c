/* Characterize the C save/load entry points before their Rust port. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <util/bmem.h>

static char events[128];
static size_t event_count;

struct save_load_probe {
	obs_source_t *source;
};

static void record_event(char kind, obs_source_t *source)
{
	assert_true(event_count + 2 < sizeof(events));
	events[event_count++] = kind;
	events[event_count++] = obs_source_get_name(source)[0];
	events[event_count] = '\0';
}

static const char *probe_name(void *unused)
{
	(void)unused;
	return "save/load probe";
}

static void *probe_create(obs_data_t *settings, obs_source_t *source)
{
	if (obs_data_get_bool(settings, "fail_create"))
		return NULL;
	struct save_load_probe *probe = bzalloc(sizeof(*probe));
	probe->source = source;
	return probe;
}

static void probe_destroy(void *data)
{
	bfree(data);
}

static void probe_save(void *data, obs_data_t *settings)
{
	struct save_load_probe *probe = data;
	obs_data_t *actual = obs_source_get_settings(probe->source);
	assert_ptr_equal(settings, actual);
	obs_data_release(actual);
	record_event('C', probe->source);
	obs_data_set_int(settings, "saved", 42);
}

static void probe_load(void *data, obs_data_t *settings)
{
	struct save_load_probe *probe = data;
	obs_data_t *actual = obs_source_get_settings(probe->source);
	assert_ptr_equal(settings, actual);
	obs_data_release(actual);
	record_event('C', probe->source);
	obs_data_set_int(settings, "loaded", 73);
}

static void observe_signal(void *data, calldata_t *cd)
{
	obs_source_t *source = calldata_ptr(cd, "source");
	assert_non_null(source);
	record_event(*(const char *)data, source);
}

static struct obs_source_info probe_info = {
	.id = "source_save_load_probe",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = probe_name,
	.create = probe_create,
	.destroy = probe_destroy,
	.save = probe_save,
	.load = probe_load,
};

static const char global_event = 'G';
static const char local_event = 'S';

static obs_source_t *make_source(const char *id, const char *name, bool private, bool fail_create)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "fail_create", fail_create);
	obs_source_t *source = private ? obs_source_create_private(id, name, settings)
				       : obs_source_create(id, name, settings, NULL);
	obs_data_release(settings);
	assert_non_null(source);
	signal_handler_t *signals = obs_source_get_signal_handler(source);
	signal_handler_connect(signals, "save", observe_signal, (void *)&local_event);
	signal_handler_connect(signals, "load", observe_signal, (void *)&local_event);
	return source;
}

static void reset_events(void)
{
	event_count = 0;
	events[0] = '\0';
}

static void callback_order_and_settings(void **unused)
{
	(void)unused;
	obs_source_t *source = make_source(probe_info.id, "parent", false, false);
	reset_events();
	obs_source_save(source);
	assert_string_equal(events, "GpSpCp");
	obs_data_t *settings = obs_source_get_settings(source);
	assert_int_equal(obs_data_get_int(settings, "saved"), 42);
	reset_events();
	obs_source_load(source);
	assert_string_equal(events, "CpGpSp");
	assert_int_equal(obs_data_get_int(settings, "loaded"), 73);
	obs_data_release(settings);
	obs_source_release(source);
	obs_wait_for_destroy_queue();
}

static void optional_callbacks_still_signal(void **unused)
{
	(void)unused;
	obs_source_t *source = make_source("source_save_load_no_callbacks", "optional", false, false);
	reset_events();
	obs_source_save(source);
	obs_source_load(source);
	obs_source_load2(source);
	assert_string_equal(events, "GoSoGoSoGoSo");
	obs_source_release(source);
	obs_wait_for_destroy_queue();
}

static void invalid_plugin_data_is_silent(void **unused)
{
	(void)unused;
	obs_source_t *source = make_source(probe_info.id, "invalid", false, true);
	obs_source_t *filter = make_source("source_save_load_filter", "filter", false, false);
	obs_source_filter_add(source, filter);
	reset_events();
	obs_source_save(NULL);
	obs_source_load(NULL);
	obs_source_load2(NULL);
	obs_source_save(source);
	obs_source_load(source);
	obs_source_load2(source);
	/* Characterized, not endorsed: invalid parent data also skips valid filters. */
	assert_string_equal(events, "");
	obs_source_release(filter);
	obs_source_release(source);
	obs_wait_for_destroy_queue();
}

static void private_source_only_signals_locally(void **unused)
{
	(void)unused;
	obs_source_t *source = make_source(probe_info.id, "private", true, false);
	reset_events();
	obs_source_save(source);
	obs_source_load2(source);
	assert_string_equal(events, "SpCpCpSp");
	obs_source_release(source);
	obs_wait_for_destroy_queue();
}

static void load2_loads_parent_then_reverse_filters(void **unused)
{
	(void)unused;
	obs_source_t *source = make_source(probe_info.id, "parent", false, false);
	obs_source_t *first = make_source("source_save_load_filter", "alpha", false, false);
	obs_source_t *invalid = make_source("source_save_load_filter", "invalid", false, true);
	obs_source_t *last = make_source("source_save_load_filter", "beta", false, false);
	obs_source_filter_add(source, first);
	obs_source_filter_add(source, invalid);
	obs_source_filter_add(source, last);
	/* add prepends: the stored order is beta, invalid, alpha. */
	assert_int_equal(obs_source_filter_get_index(source, last), 0);
	assert_int_equal(obs_source_filter_get_index(source, first), 2);
	reset_events();
	obs_source_load(source);
	assert_string_equal(events, "CpGpSp");
	reset_events();
	obs_source_load2(source);
	/* Filters are private: their loads emit only the local signal. */
	assert_string_equal(events, "CpGpSpCaSaCbSb");
	reset_events();
	obs_source_save(source);
	assert_string_equal(events, "GpSpCp");
	obs_source_release(last);
	obs_source_release(invalid);
	obs_source_release(first);
	obs_source_release(source);
	obs_wait_for_destroy_queue();
}

static int setup(void **unused)
{
	(void)unused;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	obs_register_source(&probe_info);
	struct obs_source_info optional = probe_info;
	optional.id = "source_save_load_no_callbacks";
	optional.save = NULL;
	optional.load = NULL;
	obs_register_source(&optional);
	struct obs_source_info filter = probe_info;
	filter.id = "source_save_load_filter";
	filter.type = OBS_SOURCE_TYPE_FILTER;
	obs_register_source(&filter);
	signal_handler_connect(obs_get_signal_handler(), "source_save", observe_signal, (void *)&global_event);
	signal_handler_connect(obs_get_signal_handler(), "source_load", observe_signal, (void *)&global_event);
	return 0;
}

static int teardown(void **unused)
{
	(void)unused;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(callback_order_and_settings),
		cmocka_unit_test(optional_callbacks_still_signal),
		cmocka_unit_test(invalid_plugin_data_is_silent),
		cmocka_unit_test(private_source_only_signals_locally),
		cmocka_unit_test(load2_loads_parent_then_reverse_filters),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
