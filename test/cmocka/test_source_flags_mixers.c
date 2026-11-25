/* Characterization of obs_source_set_flags() and obs_source_set_audio_mixers()
 * (#5610), the two public setters the advanced audio properties, source
 * loading and obs-websocket use.
 *
 * What it pins, on test-owned struct obs_source values with a real signal
 * handler (no core needed):
 *
 *   - NULL: each setter logs one LOG_DEBUG line,
 *     "<function>: Null 'source' parameter", and does nothing else.
 *   - set_flags: an unchanged value stores nothing and emits nothing. A
 *     change stores first, then emits update_flags once with (source, the
 *     new flags) on a fixed 128-byte calldata.
 *   - set_audio_mixers: a source without OBS_SOURCE_AUDIO that does not own
 *     its info id is left alone, whatever the value; owning the info id is
 *     enough. An unchanged value stores nothing and emits nothing. On a
 *     change, the audio_mixers handlers run before the store (a handler sees
 *     the old field and the old obs_source_get_audio_mixers() answer) and can
 *     rewrite "mixers" in the calldata; the stored value is what the calldata
 *     holds afterwards, truncated to its low 32 bits (0 when a handler left
 *     a "mixers" of another size). Handlers run in connection order, each
 *     seeing the previous one's rewrite.
 *   - Neither setter writes any other byte of the source, or logs for a
 *     valid source. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

#define EVENTS 8

/* What a handler saw: the calldata's source and value, the field and the
 * public getter at the time, and the calldata's shape. */
struct event {
	void *source;
	long long value;
	uint32_t field;
	uint32_t getter;
	bool fixed;
	size_t capacity;
};

enum rewrite { REWRITE_NONE, REWRITE_INT, REWRITE_BOOL };

struct fixture {
	struct obs_source source;
	struct obs_source before;
	size_t calls;
	struct event events[EVENTS];
	enum rewrite rewrite;
	long long rewrite_value;
};

static void record(struct fixture *f, calldata_t *data, const char *name, uint32_t field)
{
	assert_true(f->calls < EVENTS);
	struct event *e = &f->events[f->calls++];
	e->source = calldata_ptr(data, "source");
	e->value = calldata_int(data, name);
	e->field = field;
	e->getter = obs_source_get_audio_mixers(&f->source);
	e->fixed = data->fixed;
	e->capacity = data->capacity;
}

static void on_update_flags(void *param, calldata_t *data)
{
	struct fixture *f = param;
	record(f, data, "flags", f->source.flags);
}

static void on_audio_mixers(void *param, calldata_t *data)
{
	struct fixture *f = param;
	record(f, data, "mixers", f->source.audio_mixers);
	if (f->rewrite == REWRITE_INT)
		calldata_set_int(data, "mixers", f->rewrite_value);
	else if (f->rewrite == REWRITE_BOOL)
		calldata_set_bool(data, "mixers", true);
}

static void fixture_init(struct fixture *f, unsigned char fill, uint32_t output_flags, bool owns_info_id)
{
	memset(f, fill, sizeof(*f));
	f->source.info.output_flags = output_flags;
	f->source.owns_info_id = owns_info_id;
	f->source.context.signals = signal_handler_create();
	assert_non_null(f->source.context.signals);
	assert_true(signal_handler_add(f->source.context.signals, "void update_flags(ptr source, int flags)"));
	assert_true(signal_handler_add(f->source.context.signals, "void audio_mixers(ptr source, in out int mixers)"));
	signal_handler_connect(f->source.context.signals, "update_flags", on_update_flags, f);
	signal_handler_connect(f->source.context.signals, "audio_mixers", on_audio_mixers, f);
	f->calls = 0;
	f->rewrite = REWRITE_NONE;
	f->rewrite_value = 0;
	memcpy(&f->before, &f->source, sizeof(f->source));
}

/* Every byte of the source is as before, apart from the expected values of
 * the two fields. */
static void assert_only(struct fixture *f, uint32_t flags, uint32_t audio_mixers)
{
	f->before.flags = flags;
	f->before.audio_mixers = audio_mixers;
	assert_memory_equal(&f->before, &f->source, sizeof(f->source));
}

static void fixture_free(struct fixture *f)
{
	signal_handler_destroy(f->source.context.signals);
}

static const unsigned char fills[] = {0x00, 0xff, 0x5a};

static void null_source_logs_once(void **state)
{
	(void)state;
	capture_start();
	obs_source_set_flags(NULL, 1);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_set_flags: Null 'source' parameter");

	capture_start();
	obs_source_set_audio_mixers(NULL, 1);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_set_audio_mixers: Null 'source' parameter");
}

static void set_flags_stores_then_signals_a_change(void **state)
{
	(void)state;
	static struct fixture f;
	static const uint32_t values[] = {
		0, 0, OBS_SOURCE_FLAG_FORCE_MONO, OBS_SOURCE_FLAG_FORCE_MONO, 0xffffffffu, 0x5a5a5a5au, 0};

	capture_start();
	for (size_t fill = 0; fill < sizeof(fills); fill++) {
		fixture_init(&f, fills[fill], OBS_SOURCE_AUDIO, false);
		uint32_t flags = f.source.flags;
		const uint32_t mixers = f.source.audio_mixers;
		for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
			f.calls = 0;
			obs_source_set_flags(&f.source, values[i]);
			if (values[i] == flags) {
				assert_int_equal(f.calls, 0);
			} else {
				assert_int_equal(f.calls, 1);
				const struct event *e = &f.events[0];
				assert_ptr_equal(e->source, &f.source);
				assert_int_equal(e->value, (long long)values[i]);
				/* Stored before the signal. */
				assert_int_equal(e->field, values[i]);
				assert_true(e->fixed);
				assert_int_equal(e->capacity, 128);
				flags = values[i];
			}
			assert_only(&f, flags, mixers);
		}
		fixture_free(&f);
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void set_audio_mixers_needs_audio_or_an_owned_id(void **state)
{
	(void)state;
	static struct fixture f;
	static const uint32_t no_audio[] = {0, OBS_SOURCE_VIDEO, OBS_SOURCE_ASYNC_VIDEO, ~(uint32_t)OBS_SOURCE_AUDIO};

	capture_start();
	for (size_t fill = 0; fill < sizeof(fills); fill++) {
		for (size_t i = 0; i < sizeof(no_audio) / sizeof(no_audio[0]); i++) {
			fixture_init(&f, fills[fill], no_audio[i], false);
			const uint32_t flags = f.source.flags;
			const uint32_t mixers = f.source.audio_mixers;
			obs_source_set_audio_mixers(&f.source, mixers ^ 1);
			obs_source_set_audio_mixers(&f.source, 0);
			obs_source_set_audio_mixers(&f.source, 0x3f);
			assert_int_equal(f.calls, 0);
			assert_only(&f, flags, mixers);
			fixture_free(&f);

			/* Owning the info id is enough. */
			fixture_init(&f, fills[fill], no_audio[i], true);
			obs_source_set_audio_mixers(&f.source, f.source.audio_mixers ^ 1);
			assert_int_equal(f.calls, 1);
			assert_only(&f, flags, mixers ^ 1);
			fixture_free(&f);
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void set_audio_mixers_signals_before_the_store(void **state)
{
	(void)state;
	static struct fixture f;
	static const uint32_t values[] = {0x3f, 0x3f, 0x5, 0, 0, 0xffffffffu, 1};

	capture_start();
	for (size_t fill = 0; fill < sizeof(fills); fill++) {
		fixture_init(&f, fills[fill], OBS_SOURCE_AUDIO | OBS_SOURCE_VIDEO, false);
		const uint32_t flags = f.source.flags;
		uint32_t mixers = f.source.audio_mixers;
		for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
			f.calls = 0;
			obs_source_set_audio_mixers(&f.source, values[i]);
			if (values[i] == mixers) {
				assert_int_equal(f.calls, 0);
			} else {
				assert_int_equal(f.calls, 1);
				const struct event *e = &f.events[0];
				assert_ptr_equal(e->source, &f.source);
				assert_int_equal(e->value, (long long)values[i]);
				/* The handler still sees the old value. */
				assert_int_equal(e->field, mixers);
				assert_int_equal(e->getter, mixers);
				assert_true(e->fixed);
				assert_int_equal(e->capacity, 128);
				mixers = values[i];
			}
			assert_only(&f, flags, mixers);
		}
		fixture_free(&f);
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void second_handler(void *param, calldata_t *data)
{
	struct fixture *f = param;
	record(f, data, "mixers", f->source.audio_mixers);
	calldata_set_int(data, "mixers", calldata_int(data, "mixers") | 0x100);
}

static void set_audio_mixers_stores_what_the_handlers_leave(void **state)
{
	(void)state;
	static struct fixture f;
	static const long long rewrites[] = {0,  0x3f,           0xffffffffll, 0x100000005ll,
					     -1, -0x100000000ll, INT64_MIN,    INT64_MAX};

	capture_start();
	for (size_t fill = 0; fill < sizeof(fills); fill++) {
		for (size_t i = 0; i < sizeof(rewrites) / sizeof(rewrites[0]); i++) {
			fixture_init(&f, fills[fill], OBS_SOURCE_AUDIO, false);
			const uint32_t flags = f.source.flags;
			f.rewrite = REWRITE_INT;
			f.rewrite_value = rewrites[i];
			obs_source_set_audio_mixers(&f.source, f.source.audio_mixers ^ 2);
			assert_int_equal(f.calls, 1);
			assert_only(&f, flags, (uint32_t)(uint64_t)rewrites[i]);
			fixture_free(&f);
		}

		/* A "mixers" left with another size reads as 0. */
		fixture_init(&f, fills[fill], OBS_SOURCE_AUDIO, false);
		const uint32_t flags = f.source.flags;
		f.rewrite = REWRITE_BOOL;
		obs_source_set_audio_mixers(&f.source, f.source.audio_mixers ^ 2);
		assert_int_equal(f.calls, 1);
		assert_only(&f, flags, 0);
		fixture_free(&f);

		/* Handlers run in connection order; the second sees the first's
		 * rewrite, and the store sees the second's. */
		fixture_init(&f, fills[fill], OBS_SOURCE_AUDIO, false);
		signal_handler_connect(f.source.context.signals, "audio_mixers", second_handler, &f);
		f.rewrite = REWRITE_INT;
		f.rewrite_value = 0x7;
		const uint32_t old = f.source.audio_mixers;
		obs_source_set_audio_mixers(&f.source, old ^ 2);
		assert_int_equal(f.calls, 2);
		assert_int_equal(f.events[0].value, (long long)(old ^ 2));
		assert_int_equal(f.events[1].value, 0x7);
		assert_int_equal(f.events[1].field, old);
		assert_only(&f, flags, 0x107);
		fixture_free(&f);
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_logs_once),
		cmocka_unit_test(set_flags_stores_then_signals_a_change),
		cmocka_unit_test(set_audio_mixers_needs_audio_or_an_owned_id),
		cmocka_unit_test(set_audio_mixers_signals_before_the_store),
		cmocka_unit_test(set_audio_mixers_stores_what_the_handlers_leave),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
