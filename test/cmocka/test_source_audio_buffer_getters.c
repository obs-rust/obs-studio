/* Characterization of obs_source_get_audio_timestamp() and
 * obs_source_get_audio_mix() (#5589), the two getters scenes, transitions
 * and obs-websocket call every audio tick to read a source's audio buffer.
 *
 * What it pins, on a test-owned struct obs_source (no core needed):
 *
 *   - obs_source_get_audio_timestamp(NULL) returns 0 and logs one LOG_DEBUG
 *     line, "obs_source_get_audio_timestamp: Null 'source' parameter";
 *   - otherwise the answer is audio_ts alone: with every other byte set to
 *     0x00, 0xff or 0x5a the result follows audio_ts, and the call writes
 *     nothing outside the mutex;
 *   - audio_ts is read under audio_buf_mutex: while another thread holds
 *     the mutex the call waits, and it returns the value written before the
 *     unlock;
 *   - obs_source_get_audio_mix(NULL, audio) logs
 *     "obs_source_get_audio_mix: Null 'source' parameter" and leaves audio
 *     untouched; a NULL audio logs "audio: Null 'audio' parameter" (the
 *     parameter name stands where the function name usually goes) and
 *     writes nothing;
 *   - otherwise every output[mix].data[ch] is the matching
 *     audio_output_buf[mix][ch] pointer (pointers copied, not samples), for
 *     every mix and channel, and the source is not written. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/platform.h"
#include "util/threading.h"

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

/* The source bytes outside audio_buf_mutex: locking may touch the mutex's
 * own bookkeeping, nothing else may change. */
static void assert_unchanged_outside_mutex(const struct obs_source *before, const struct obs_source *after)
{
	const size_t start = offsetof(struct obs_source, audio_buf_mutex);
	const size_t end = start + sizeof(before->audio_buf_mutex);
	assert_memory_equal(before, after, start);
	assert_memory_equal((const unsigned char *)before + end, (const unsigned char *)after + end,
			    sizeof(*before) - end);
}

static void null_source_timestamp_is_zero_and_names_the_caller(void **state)
{
	(void)state;
	capture_start();
	uint64_t ts = obs_source_get_audio_timestamp(NULL);
	capture_stop();

	assert_int_equal(ts, 0);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_get_audio_timestamp: Null 'source' parameter");
}

static void timestamp_reads_only_audio_ts(void **state)
{
	(void)state;
	const unsigned char fills[] = {0x00, 0xff, 0x5a};
	const uint64_t values[] = {0, 1, 48000, UINT64_C(0x0123456789abcdef), UINT64_MAX};
	struct obs_source *source = bmalloc(sizeof(*source));
	struct obs_source *snapshot = bmalloc(sizeof(*source));

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		for (size_t v = 0; v < sizeof(values) / sizeof(values[0]); v++) {
			/* Every other byte set to the fill, so a getter reading
			 * another field (last_audio_ts, next_audio_ts_min, the
			 * timing fields) can't pass by finding the value there. */
			memset(source, fills[f], sizeof(*source));
			assert_int_equal(pthread_mutex_init(&source->audio_buf_mutex, NULL), 0);
			source->audio_ts = values[v];
			memcpy(snapshot, source, sizeof(*source));
			for (unsigned repeat = 0; repeat < 2; repeat++) {
				assert_true(obs_source_get_audio_timestamp(source) == values[v]);
				assert_unchanged_outside_mutex(snapshot, source);
			}
			/* The mutex is left unlocked. */
			assert_int_equal(pthread_mutex_trylock(&source->audio_buf_mutex), 0);
			pthread_mutex_unlock(&source->audio_buf_mutex);
			pthread_mutex_destroy(&source->audio_buf_mutex);
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);

	bfree(snapshot);
	bfree(source);
}

struct reader {
	struct obs_source *source;
	volatile long started;
	volatile long done;
	uint64_t ts;
};

static void *read_timestamp(void *param)
{
	struct reader *reader = param;
	os_atomic_set_long(&reader->started, 1);
	reader->ts = obs_source_get_audio_timestamp(reader->source);
	os_atomic_set_long(&reader->done, 1);
	return NULL;
}

static void timestamp_waits_for_the_audio_buffer_mutex(void **state)
{
	(void)state;
	struct obs_source *source = bzalloc(sizeof(*source));
	assert_int_equal(pthread_mutex_init(&source->audio_buf_mutex, NULL), 0);
	source->audio_ts = 1000;

	struct reader reader = {.source = source};
	pthread_t thread;

	pthread_mutex_lock(&source->audio_buf_mutex);
	assert_int_equal(pthread_create(&thread, NULL, read_timestamp, &reader), 0);
	while (!os_atomic_load_long(&reader.started))
		;
	/* A reader that skipped the mutex would be done long before this. */
	os_sleep_ms(100);
	assert_int_equal(os_atomic_load_long(&reader.done), 0);

	/* What the audio thread writes before it unlocks is what the reader
	 * returns. */
	source->audio_ts = 2000;
	pthread_mutex_unlock(&source->audio_buf_mutex);
	assert_int_equal(pthread_join(thread, NULL), 0);

	assert_int_equal(os_atomic_load_long(&reader.done), 1);
	assert_true(reader.ts == 2000);

	pthread_mutex_destroy(&source->audio_buf_mutex);
	bfree(source);
}

static void fill_mix(struct obs_source_audio_mix *audio, unsigned char fill)
{
	memset(audio, fill, sizeof(*audio));
}

static void null_source_mix_names_the_caller_and_writes_nothing(void **state)
{
	(void)state;
	struct obs_source_audio_mix audio;
	struct obs_source_audio_mix snapshot;
	fill_mix(&audio, 0x5a);
	memcpy(&snapshot, &audio, sizeof(audio));

	capture_start();
	obs_source_get_audio_mix(NULL, &audio);
	capture_stop();

	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_get_audio_mix: Null 'source' parameter");
	assert_memory_equal(&snapshot, &audio, sizeof(audio));

	/* Both NULL: the source is checked first, one line only. */
	capture_start();
	obs_source_get_audio_mix(NULL, NULL);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_string_equal(log_message, "obs_source_get_audio_mix: Null 'source' parameter");
}

static void null_audio_names_the_parameter_and_writes_nothing(void **state)
{
	(void)state;
	struct obs_source *source = bmalloc(sizeof(*source));
	struct obs_source *snapshot = bmalloc(sizeof(*source));
	memset(source, 0x5a, sizeof(*source));
	memcpy(snapshot, source, sizeof(*source));

	capture_start();
	obs_source_get_audio_mix(source, NULL);
	capture_stop();

	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	/* obs_ptr_valid(audio, "audio") passes "audio" where the function
	 * name usually goes, so the line names the parameter twice. */
	assert_string_equal(log_message, "audio: Null 'audio' parameter");
	assert_memory_equal(snapshot, source, sizeof(*source));

	bfree(snapshot);
	bfree(source);
}

static void mix_copies_every_buffer_pointer(void **state)
{
	(void)state;
	const unsigned char fills[] = {0x00, 0xff, 0x5a};
	/* Distinct addresses to stand for the buffers; only their values are
	 * compared, nothing is read through them. */
	static float samples[MAX_AUDIO_MIXES][MAX_AUDIO_CHANNELS];
	struct obs_source *source = bmalloc(sizeof(*source));
	struct obs_source *snapshot = bmalloc(sizeof(*source));
	struct obs_source_audio_mix audio;

	capture_start();
	for (size_t f = 0; f < sizeof(fills); f++) {
		memset(source, fills[f], sizeof(*source));
		for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
				source->audio_output_buf[mix][ch] = (mix + ch) % 3 == 0 ? NULL : &samples[mix][ch];
		memcpy(snapshot, source, sizeof(*source));

		fill_mix(&audio, (unsigned char)~fills[f]);
		obs_source_get_audio_mix(source, &audio);

		for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++)
			for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
				assert_ptr_equal(audio.output[mix].data[ch], source->audio_output_buf[mix][ch]);
		assert_memory_equal(snapshot, source, sizeof(*source));
	}
	capture_stop();
	assert_int_equal(log_count, 0);

	bfree(snapshot);
	bfree(source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source_timestamp_is_zero_and_names_the_caller),
		cmocka_unit_test(timestamp_reads_only_audio_ts),
		cmocka_unit_test(timestamp_waits_for_the_audio_buffer_mutex),
		cmocka_unit_test(null_source_mix_names_the_caller_and_writes_nothing),
		cmocka_unit_test(null_audio_names_the_parameter_and_writes_nothing),
		cmocka_unit_test(mix_copies_every_buffer_pointer),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
