/* Characterize the six legacy index-to-ID catalogs before extraction (Refs #5170).
 * A live core and a writable output on success are required. IDs are borrowed;
 * failed lookups return before touching the output, which may then be NULL.
 * Only test-owned registrations are asserted: CMake and Cargo-native reference
 * libraries may register different built-in/module types (#5182). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include <obs-module.h>

struct catalog {
	bool (*enumerate)(size_t, const char **);
	const char *sentinel;
	const char *ids[2];
};

static const struct catalog catalogs[] = {
	{obs_enum_input_types, "input untouched", {"catalog_input_z", "catalog_input_a"}},
	{obs_enum_filter_types, "filter untouched", {"catalog_filter_z", "catalog_filter_a"}},
	{obs_enum_transition_types, "transition untouched", {"catalog_transition_z", "catalog_transition_a"}},
	{obs_enum_output_types, "output untouched", {"catalog_output_z", "catalog_output_a"}},
	{obs_enum_encoder_types, "encoder untouched", {"catalog_encoder_z", "catalog_encoder_a"}},
	{obs_enum_service_types, "service untouched", {"catalog_service_z", "catalog_service_a"}},
};

static void expect_end(const struct catalog *catalog, size_t index)
{
	const char *id = catalog->sentinel;
	assert_false(catalog->enumerate(index, &id));
	assert_ptr_equal(id, catalog->sentinel);
	assert_false(catalog->enumerate(index, NULL));
}

struct startup_probe {
	log_handler_t previous;
	void *previous_param;
	bool ran;
};

#if defined(__GNUC__) || defined(__clang__)
static void probe_empty(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void probe_empty(int level, const char *format, va_list args, void *param)
{
	struct startup_probe *probe = param;
	const char *id;
	/* Startup logging exposes the initialized core before built-in types.
	 * The existing source enumeration characterization uses this same seam. */
	if (!probe->ran && obs_initialized() && !obs_enum_source_types(0, &id)) {
		probe->ran = true;
		for (size_t i = 0; i < 6; i++) {
			expect_end(&catalogs[i], 0);
			expect_end(&catalogs[i], SIZE_MAX);
		}
	}
	if (probe->previous)
		probe->previous(level, format, args, probe->previous_param);
}

static void empty_catalogs(void **state)
{
	(void)state;
	struct startup_probe probe = {0};
	base_get_log_handler(&probe.previous, &probe.previous_param);
	base_set_log_handler(probe_empty, &probe);
	bool started = obs_startup("en-US", NULL, NULL);
	base_set_log_handler(probe.previous, probe.previous_param);
	if (started)
		obs_shutdown();
	assert_true(started);
	assert_true(probe.ran);
}

static const char *get_name(void *data)
{
	(void)data;
	return "Type catalog fixture";
}

static void unused_destroy(void *data)
{
	(void)data;
	assert_true(false);
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	assert_true(false);
	return NULL;
}

static bool output_start(void *data)
{
	(void)data;
	assert_true(false);
	return false;
}

static void output_stop(void *data, uint64_t timestamp)
{
	(void)data;
	(void)timestamp;
	assert_true(false);
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	assert_true(false);
	return NULL;
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received;
	assert_true(false);
	return false;
}

static void *service_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	(void)service;
	assert_true(false);
	return NULL;
}

static const char *service_protocol(void *data)
{
	(void)data;
	assert_true(false);
	return NULL;
}

static void register_type(size_t catalog, const char *id)
{
	if (catalog < 3) {
		const enum obs_source_type types[] = {OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_TYPE_FILTER,
						      OBS_SOURCE_TYPE_TRANSITION};
		struct obs_source_info info = {0};
		info.id = id;
		info.type = types[catalog];
		info.get_name = get_name;
		obs_register_source(&info);
	} else if (catalog == 3) {
		struct obs_output_info info = {0};
		info.id = id;
		info.get_name = get_name;
		info.create = output_create;
		info.destroy = unused_destroy;
		info.start = output_start;
		info.stop = output_stop;
		obs_register_output(&info);
	} else if (catalog == 4) {
		struct obs_encoder_info info = {0};
		info.id = id;
		info.type = OBS_ENCODER_VIDEO;
		info.codec = "catalog-codec";
		info.get_name = get_name;
		info.create = encoder_create;
		info.destroy = unused_destroy;
		info.encode = encoder_encode;
		obs_register_encoder(&info);
	} else {
		struct obs_service_info info = {0};
		info.id = id;
		info.get_name = get_name;
		info.create = service_create;
		info.destroy = unused_destroy;
		info.get_protocol = service_protocol;
		obs_register_service(&info);
	}
}

static size_t count_types(const struct catalog *catalog)
{
	size_t count = 0;
	const char *id;
	while (catalog->enumerate(count, &id)) {
		assert_true(count < 1024);
		count++;
	}
	return count;
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	/* No instances or owned type_data. Real shutdown frees the registries
	 * and versioned source IDs; the caller's IDs have static lifetime. */
	obs_shutdown();
	return 0;
}

static void populated_catalogs(void **state)
{
	(void)state;
	size_t base[6], added[6] = {0};
	const char *borrowed[6][2];
	for (size_t c = 0; c < 6; c++)
		base[c] = count_types(&catalogs[c]);

	/* Interleave catalogs and reverse lexical ID order. Each independent
	 * registry appends exactly its own registrations without sorting. */
	for (size_t entry = 0; entry < 2; entry++) {
		for (size_t c = 0; c < 6; c++) {
			register_type(c, catalogs[c].ids[entry]);
			added[c]++;
			for (size_t other = 0; other < 6; other++)
				assert_int_equal(count_types(&catalogs[other]), base[other] + added[other]);
			const char *id = catalogs[c].sentinel;
			assert_true(catalogs[c].enumerate(base[c] + entry, &id));
			assert_string_equal(id, catalogs[c].ids[entry]);
			borrowed[c][entry] = id;
		}
	}
	for (size_t c = 0; c < 6; c++) {
		for (size_t entry = 0; entry < 2; entry++) {
			const char *id = catalogs[c].sentinel;
			assert_true(catalogs[c].enumerate(base[c] + entry, &id));
			assert_ptr_equal(id, borrowed[c][entry]);
			assert_string_equal(id, catalogs[c].ids[entry]);
		}
		expect_end(&catalogs[c], base[c] + 2);
		expect_end(&catalogs[c], base[c] + 3);
		expect_end(&catalogs[c], SIZE_MAX);
		assert_string_equal(borrowed[c][0], catalogs[c].ids[0]);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_catalogs),
		cmocka_unit_test_setup_teardown(populated_catalogs, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
