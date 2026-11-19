/* Null backend characterization, before its Rust swap. The public calls below
 * use the real selected libobs (C-reference or hybrid), so registration must
 * mirror Linux/FreeBSD's ENABLE_PULSEAUDIO=OFF backend selection.
 *
 * The four private functions are hidden in libobs. Characterize the unchanged
 * whole C backend under distinct names as well; these source-only assertions
 * must be mirrored against the Rust implementation in the later swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "build/libobs-nix.pch.h"

#define obs_audio_monitoring_available null_c_available
#define obs_enum_audio_monitoring_devices null_c_enum_devices
#define audio_monitor_create null_c_create
#define audio_monitor_reset null_c_reset
#define audio_monitor_destroy null_c_destroy
#define devices_match null_c_devices_match
#include "libobs/audio-monitoring/null/null-audio-monitoring.c.h"
#undef obs_audio_monitoring_available
#undef obs_enum_audio_monitoring_devices
#undef audio_monitor_create
#undef audio_monitor_reset
#undef audio_monitor_destroy
#undef devices_match

static unsigned log_calls;
static long allocation_count;
static log_handler_t previous_log_handler;
static void *previous_log_data;

static void count_log(int level, const char *format, va_list args, void *data)
{
	(void)level;
	(void)format;
	(void)args;
	(void)data;
	log_calls++;
}

static int setup(void **state)
{
	(void)state;
	base_get_log_handler(&previous_log_handler, &previous_log_data);
	base_set_log_handler(count_log, NULL);
	log_calls = 0;
	allocation_count = bnum_allocs();
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(previous_log_handler, previous_log_data);
	assert_int_equal(log_calls, 0);
	assert_int_equal(bnum_allocs(), allocation_count);
	return 0;
}

static void availability_is_false_without_startup(void **state)
{
	(void)state;
	assert_true(obs_audio_monitoring_available()); /* deliberately wrong RED expectation */
	assert_false(null_c_available());
}

static bool unexpected_device(void *data, const char *name, const char *id)
{
	(void)data;
	(void)name;
	(void)id;
	/* No callback, even one that would stop enumeration, may be invoked. */
	fail();
	return false;
}

static void enumeration_never_calls_back_or_writes_data(void **state)
{
	(void)state;
	unsigned char data[] = {0xa5, 0x00, 0xff, 0x5a};
	const unsigned char saved[] = {0xa5, 0x00, 0xff, 0x5a};
	obs_enum_audio_monitoring_devices(unexpected_device, data);
	obs_enum_audio_monitoring_devices(unexpected_device, NULL);
	obs_enum_audio_monitoring_devices(NULL, data);
	obs_enum_audio_monitoring_devices(NULL, NULL);
	null_c_enum_devices(unexpected_device, data);
	null_c_enum_devices(unexpected_device, NULL);
	null_c_enum_devices(NULL, data);
	null_c_enum_devices(NULL, NULL);
	assert_memory_equal(data, saved, sizeof(data));
}

static void lifecycle_ignores_null_and_nonnull_handles(void **state)
{
	(void)state;
	/* These bytes are not an OBS object. The null backend never dereferences,
	 * initializes, takes ownership of, or frees the caller's pointer. */
	union {
		long double alignment;
		void *pointer_alignment;
		unsigned char bytes[64];
	} storage;
	memset(&storage, 0xa5, sizeof(storage));
	unsigned char saved[sizeof(storage)];
	memcpy(saved, &storage, sizeof(storage));
	assert_null(null_c_create(NULL));
	assert_null(null_c_create((obs_source_t *)&storage));
	null_c_reset(NULL);
	null_c_reset((struct audio_monitor *)&storage);
	null_c_destroy(NULL);
	null_c_destroy((struct audio_monitor *)&storage);
	/* Repeat to characterize the absence of ownership or state changes. */
	null_c_reset((struct audio_monitor *)&storage);
	null_c_destroy((struct audio_monitor *)&storage);
	assert_null(null_c_create((obs_source_t *)&storage));
	assert_memory_equal(&storage, saved, sizeof(storage));
}

static void device_matching_is_always_false(void **state)
{
	(void)state;
	char equal1[] = "same";
	char equal2[] = "same";
	char bytes[] = {(char)0xff, (char)0x80, 'x', 0};
	char unterminated[] = {'x', 'y'};
	const char *ids[] = {NULL, "", "default", equal1, equal2, "different", bytes, unterminated};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++)
		for (size_t j = 0; j < sizeof(ids) / sizeof(ids[0]); j++)
			assert_false(null_c_devices_match(ids[i], ids[j]));
	assert_string_equal(equal1, "same");
	assert_string_equal(equal2, "same");
	const char saved_bytes[] = {(char)0xff, (char)0x80, 'x', 0};
	assert_memory_equal(bytes, saved_bytes, sizeof(bytes));
	assert_memory_equal(unterminated, "xy", sizeof(unterminated));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(availability_is_false_without_startup, setup, teardown),
		cmocka_unit_test_setup_teardown(enumeration_never_calls_back_or_writes_data, setup, teardown),
		cmocka_unit_test_setup_teardown(lifecycle_ignores_null_and_nonnull_handles, setup, teardown),
		cmocka_unit_test_setup_teardown(device_matching_is_always_false, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
