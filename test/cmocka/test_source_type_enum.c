/* Characterize the actual libobs registry exports before extraction (Refs #4772).
 * The core must be live. A successful obs_enum_source_types call requires an
 * output pointer; input_types2 accepts either output independently. Returned
 * strings are borrowed. No-core calls and NULL source outputs on success are
 * outside the C domain. Existing registration tests remain unchanged. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include <obs-module.h>

static const char sentinel[] = "untouched";
static const char input_id[] = "enum_input";
static const char filter_id[] = "enum_filter";
static const char transition_id[] = "enum_transition";
static const char scene_id[] = "enum_scene";

static void expect_source_end(size_t index)
{
	const char *id = sentinel;
	assert_false(obs_enum_source_types(index, &id));
	assert_ptr_equal(id, sentinel);
	/* Failure returns before dereferencing the otherwise-required output. */
	assert_false(obs_enum_source_types(index, NULL));
}

static void expect_input_end(size_t index)
{
	for (unsigned mask = 0; mask < 4; mask++) {
		const char *id = sentinel, *unversioned = sentinel;
		assert_false(obs_enum_input_types2(index, mask & 1 ? &id : NULL, mask & 2 ? &unversioned : NULL));
		assert_ptr_equal(id, sentinel);
		assert_ptr_equal(unversioned, sentinel);
	}
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
	const char *id = sentinel;
	if (!probe->ran && obs_initialized() && !obs_enum_source_types(0, &id)) {
		probe->ran = true;
		assert_ptr_equal(id, sentinel);
		expect_source_end(0);
		expect_source_end(SIZE_MAX);
		expect_input_end(0);
		expect_input_end(SIZE_MAX);
	}
	if (probe->previous)
		probe->previous(level, format, args, probe->previous_param);
}

static void empty_registries(void **state)
{
	(void)state;
	/* As in test_source_type_info: startup logs expose a live core before the
	 * built-in source registrations. Restore the logger before asserting. */
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
	return "Enumeration fixture";
}

static uint32_t get_size(void *data)
{
	(void)data;
	return 16;
}

static bool audio_render(void *data, uint64_t *timestamp, struct obs_source_audio_mix *mix, uint32_t mixers,
			 size_t channels, size_t sample_rate)
{
	(void)data;
	(void)timestamp;
	(void)mix;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static void register_type(const char *id, enum obs_source_type type, uint32_t version)
{
	struct obs_source_info info = {0};
	info.id = id;
	info.type = type;
	info.version = version;
	info.output_flags = OBS_SOURCE_VIDEO;
	info.get_name = get_name;
	info.get_width = get_size;
	info.get_height = get_size;
	info.audio_render = audio_render;
	if (type == OBS_SOURCE_TYPE_TRANSITION) {
		info.get_width = NULL;
		info.get_height = NULL;
	}
	obs_register_source(&info);
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	/* No instances or owned type_data. OBS frees all registry/ID copies;
	 * caller IDs have static lifetime. */
	obs_shutdown();
	return 0;
}

static size_t count_types(bool inputs)
{
	size_t count = 0;
	const char *id;
	while (inputs ? obs_enum_input_types2(count, &id, NULL) : obs_enum_source_types(count, &id)) {
		assert_non_null(id);
		assert_true(count < 1024);
		count++;
	}
	return count;
}

static void populated_registries(void **state)
{
	(void)state;
	size_t sources = count_types(false), inputs = count_types(true);
	register_type(input_id, OBS_SOURCE_TYPE_INPUT, 0);
	register_type(filter_id, OBS_SOURCE_TYPE_FILTER, 0);
	register_type(input_id, OBS_SOURCE_TYPE_INPUT, 3);
	register_type(transition_id, OBS_SOURCE_TYPE_TRANSITION, 0);
	register_type(scene_id, OBS_SOURCE_TYPE_SCENE, 0);
	assert_int_equal(count_types(false), sources + 5);
	assert_int_equal(count_types(true), inputs + 2);

	const char *expected[] = {input_id, filter_id, "enum_input_v3", transition_id, scene_id};
	const char *borrowed[5];
	for (size_t i = 0; i < 5; i++) {
		assert_true(obs_enum_source_types(sources + i, &borrowed[i]));
		assert_string_equal(borrowed[i], expected[i]);
		const char *again = sentinel;
		assert_true(obs_enum_source_types(sources + i, &again));
		assert_ptr_equal(again, borrowed[i]);
	}
	for (size_t i = 0; i < 2; i++) {
		const char *id = sentinel, *unversioned = sentinel, *legacy = sentinel;
		assert_true(obs_enum_input_types2(inputs + i, &id, &unversioned));
		assert_ptr_equal(id, borrowed[i * 2]);
		assert_ptr_equal(unversioned, input_id);
		/* Registration owns the versioned ID copy, even at version zero. */
		assert_ptr_not_equal(id, input_id);
		assert_true(obs_enum_input_types(inputs + i, &legacy));
		assert_ptr_equal(id, legacy);
		for (unsigned mask = 0; mask < 4; mask++) {
			const char *only_id = sentinel, *only_unversioned = sentinel;
			assert_true(obs_enum_input_types2(inputs + i, mask & 1 ? &only_id : NULL,
							  mask & 2 ? &only_unversioned : NULL));
			assert_ptr_equal(only_id, mask & 1 ? id : sentinel);
			assert_ptr_equal(only_unversioned, mask & 2 ? unversioned : sentinel);
		}
		/* Characterized, not endorsed: aliasing outputs writes the unversioned
		 * pointer last. The two outputs are not an all-or-nothing pair. */
		const char *aliased = sentinel;
		assert_true(obs_enum_input_types2(inputs + i, &aliased, &aliased));
		assert_ptr_equal(aliased, unversioned);
	}

	const size_t source_ends[] = {sources + 5, sources + 6, UINT32_MAX, SIZE_MAX};
	const size_t input_ends[] = {inputs + 2, inputs + 3, UINT32_MAX, SIZE_MAX};
	for (size_t i = 0; i < 4; i++) {
		expect_source_end(source_ends[i]);
		expect_input_end(input_ends[i]);
	}
	/* Failure does not invalidate an earlier borrowed result. */
	assert_string_equal(borrowed[0], input_id);
	assert_string_equal(borrowed[2], "enum_input_v3");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(empty_registries),
		cmocka_unit_test_setup_teardown(populated_registries, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
