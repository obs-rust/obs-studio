/* Characterization of obs_source_set_texcoords_centered() (#5599), the setter
 * whose getter obs_source_get_texcoords_centered() is already Rust.
 *
 * The setter is hidden in libobs (obs-internal.h, no EXPORT), so this test
 * compiles its extracted C file into itself: it pins the C source only, as
 * test_deinterlace_texture_allocation does. The getter it reads back through
 * is the real libobs export (Rust in hybrid, the original C in the
 * reference). render_item() in obs-scene.c.h, the only caller, reaches the
 * swapped setter through the scene rendering tests of hybrid libobs.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - obs_source_set_texcoords_centered(source, x) stores x into
 *     texcoords_centered and writes no other byte, whatever every other byte
 *     holds and whatever the field held before, and logs nothing;
 *   - obs_source_get_texcoords_centered() then answers x and writes nothing.
 *
 * There is no NULL check in the C: NULL is outside the contract and is not
 * tested. */
#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "libobs/build/libobs.pch.h"
#include "libobs/obs-source-texcoords-centered-setter.c.h"

static unsigned log_count;
static void count_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	log_count++;
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(count_log, NULL);
	log_count = 0;
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

static const unsigned char fills[] = {0x00, 0xff, 0x5a};

static void stores_only_the_flag(void **state)
{
	(void)state;
	static struct obs_source source;
	static unsigned char expected[sizeof(source)];
	const size_t at = offsetof(struct obs_source, texcoords_centered);

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		/* Every other byte holds the fill, and the flag starts at each
		 * value too, so a setter writing any neighbour (temp_removed and
		 * enabled among them) or skipping the store shows up in the
		 * bytes. */
		for (unsigned initial = 0; initial < 2; initial++) {
			const bool sequence[] = {true, true, false, false, true, false};
			memset(&source, fills[f], sizeof(source));
			source.texcoords_centered = initial != 0;
			memcpy(expected, &source, sizeof(source));
			for (size_t i = 0; i < sizeof(sequence) / sizeof(sequence[0]); i++) {
				obs_source_set_texcoords_centered(&source, sequence[i]);
				memcpy(expected + at, &sequence[i], sizeof(bool));
				assert_memory_equal(expected, &source, sizeof(source));
				assert_int_equal(source.texcoords_centered, sequence[i]);
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void getter_answers_what_was_stored(void **state)
{
	(void)state;
	static struct obs_source source;
	static unsigned char snapshot[sizeof(source)];

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (unsigned value = 0; value < 2; value++) {
			/* The neighbouring flags hold the opposite answer, so a getter
			 * or setter on the wrong field fails. */
			memset(&source, fills[f], sizeof(source));
			source.temp_removed = value == 0;
			source.enabled = value == 0;
			obs_source_set_texcoords_centered(&source, value != 0);
			memcpy(snapshot, &source, sizeof(source));
			for (unsigned repeat = 0; repeat < 2; repeat++) {
				assert_int_equal(obs_source_get_texcoords_centered(&source), value != 0);
				assert_memory_equal(snapshot, &source, sizeof(source));
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(stores_only_the_flag),
		cmocka_unit_test(getter_answers_what_was_stored),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
