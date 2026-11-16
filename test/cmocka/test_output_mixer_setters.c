/* Direct C characterization; keep unchanged during the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct setter_log {
	unsigned count;
	int level;
	char text[256];
};
#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	struct setter_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->text, sizeof(log->text), format, args);
}
static void setters(void **state)
{
	(void)state;
	log_handler_t old;
	void *param;
	base_get_log_handler(&old, &param);
	const size_t values[] = {0, 1, 2, 3, 4, 5, 6, 63, 64, SIZE_MAX};
	for (unsigned which = 0; which < 2; which++) {
		void (*set)(obs_output_t *, size_t) = which ? obs_output_set_mixers : obs_output_set_mixer;
		const char *name = which ? "obs_output_set_mixers" : "obs_output_set_mixer";
		struct setter_log log = {0};
		base_set_log_handler(capture, &log);
		set(NULL, SIZE_MAX);
		base_set_log_handler(old, param);
		char expected[256];
		snprintf(expected, sizeof(expected), "%s: Null 'output' parameter", name);
		assert_int_equal(log.count, 1);
		assert_int_equal(log.level, LOG_DEBUG);
		assert_string_equal(log.text, expected);
		for (unsigned encoded = 0; encoded < 2; encoded++)
			for (unsigned active = 0; active < 2; active++)
				for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
					struct obs_output output = {0};
					output.context.name = "setter fixture";
					output.info.flags = encoded ? OBS_OUTPUT_ENCODED : 0;
					os_atomic_set_bool(&output.active, active != 0);
					output.mixer_mask = 0x29;
					unsigned char before[sizeof(output)];
					memcpy(before, &output, sizeof(output));
					memset(&log, 0, sizeof(log));
					base_set_log_handler(capture, &log);
					set(&output, values[i]);
					base_set_log_handler(old, param);
					size_t mask = !encoded && !active && (which || values[i] < MAX_AUDIO_MIXES)
							      ? (which ? values[i] : (size_t)1 << values[i])
							      : 0x29;
					assert_int_equal(output.mixer_mask, mask);
					output.mixer_mask = 0x29;
					assert_memory_equal(before, &output, sizeof(output));
					if (encoded) {
						snprintf(
							expected, sizeof(expected),
							"Output 'setter fixture': Tried to use %s on an encoded output",
							name);
					} else if (!which && values[i] >= MAX_AUDIO_MIXES) {
						snprintf(
							expected, sizeof(expected),
							"output 'setter fixture': Ignoring mixer index %zu, it must be below %d",
							values[i], MAX_AUDIO_MIXES);
					} else {
						assert_int_equal(log.count, 0);
						continue;
					}
					assert_int_equal(log.count, 1);
					assert_int_equal(log.level, LOG_WARNING);
					assert_string_equal(log.text, expected);
				}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(setters)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
