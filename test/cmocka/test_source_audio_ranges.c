/* The range of a source's push-to-mute and push-to-talk delays (#2051), of
 * its audio balance (#2051) and of its volume (#2108), through the public API
 * and through a loaded scene collection (obs_load_source() on obs_data):
 *
 *   - the delays are milliseconds. A scene collection stores them as signed
 *     integers, so a negative one loads as 0. The stop time, the moment the
 *     key was last held plus the delay, saturates instead of wrapping: at
 *     UINT64_MAX / 1000000 + 1 ms the multiply wraps, and at
 *     UINT64_MAX / 1000000 ms the sum does;
 *   - the balance is 0 to 1: the setter and the load clamp it, and NaN is
 *     the default, 0.5. The samples come out as at the clamped value;
 *   - the volume is a multiplier of 0 or more: NaN is the default, 1, a
 *     negative volume is 0, and +inf is 20 (+26 dB), the most the Advanced
 *     Audio Properties and obs-websocket set. Finite volumes above that are
 *     kept. The setter and the load map it, and so does the setter after the
 *     "volume" signal, whose volume is in/out. The value queued for the mix
 *     is the mapped one too; test_source_volume_mix checks the mixed audio.
 *
 * Values that were valid stay as they were: delays of 0 and 300 ms,
 * balances of 0, 0.25 and 1, and volumes of 0, -0, 0.5, 1, 20 and above, bit
 * for bit.
 *
 * The key is pressed through the hotkey router, as the frontend does, and
 * the stop time is read back after one packet: source_output_audio_data()
 * sets it from os_gettime_ns(), which the test brackets. An audio capture
 * callback reports whether libobs muted the packet and what it made of the
 * samples. The sources are kept out of the mix, so only the test's packets
 * reach them. */
#ifndef _USE_MATH_DEFINES
#define _USE_MATH_DEFINES
#endif

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs-internal.h>
#include <util/platform.h>
#include <util/threading.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#define SOURCE_ID "test_source_audio_ranges_input"
#define RATE 48000
#define FRAMES 64
#define SEC 1000000000ULL
#define MSEC 1000000ULL

static int obs_up;
static int source_count;

static const char *ar_name(void *unused)
{
	(void)unused;
	return "test source audio ranges";
}

static void *ar_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void ar_destroy(void *data)
{
	(void)data;
}

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (level > LOG_WARNING)
		return;
	vfprintf(stderr, format, args);
	fputc('\n', stderr);
}

struct capture {
	uint32_t packets;
	bool muted;
	float left[FRAMES];
	float right[FRAMES];
};

static void capture_cb(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	struct capture *cap = param;
	(void)source;

	cap->packets++;
	cap->muted = muted;
	if (audio->frames != FRAMES || !audio->data[0] || !audio->data[1])
		return;
	memcpy(cap->left, audio->data[0], sizeof(cap->left));
	memcpy(cap->right, audio->data[1], sizeof(cap->right));
}

static float left_in[FRAMES];
static float right_in[FRAMES];

static uint32_t bits_of(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

/* A fresh, public input (the audio hotkeys are only registered for those),
 * removed so it never joins the mix. */
static obs_source_t *new_source(void)
{
	char name[64];
	snprintf(name, sizeof(name), "audio ranges %d", source_count++);
	obs_source_t *source = obs_source_create(SOURCE_ID, name, NULL, NULL);
	assert_non_null(source);
	obs_source_remove(source);
	return source;
}

/* The same through a scene collection's source entry. */
static obs_source_t *load_source(obs_data_t *data)
{
	char name[64];
	snprintf(name, sizeof(name), "audio ranges %d", source_count++);
	obs_data_set_string(data, "id", SOURCE_ID);
	obs_data_set_string(data, "name", name);
	obs_source_t *source = obs_load_source(data);
	assert_non_null(source);
	obs_source_remove(source);
	return source;
}

static void release_source(obs_source_t *source)
{
	obs_source_release(source);
}

/* Sends one packet at timestamp; returns what the capture callback saw and
 * the os_gettime_ns() bracket around source_output_audio_data(). */
static struct capture send_packet(obs_source_t *source, uint64_t timestamp, uint64_t *before, uint64_t *after)
{
	struct capture cap = {0};
	obs_source_add_audio_capture_callback(source, capture_cb, &cap);

	struct obs_source_audio audio = {
		.data = {(uint8_t *)left_in, (uint8_t *)right_in},
		.frames = FRAMES,
		.speakers = SPEAKERS_STEREO,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.samples_per_sec = RATE,
		.timestamp = timestamp,
	};
	uint64_t start = os_gettime_ns();
	obs_source_output_audio(source, &audio);
	uint64_t end = os_gettime_ns();

	obs_source_remove_audio_capture_callback(source, capture_cb, &cap);
	assert_int_equal(cap.packets, 1);
	if (before)
		*before = start;
	if (after)
		*after = end;
	return cap;
}

/* far from os_gettime_ns(), so libobs never mistakes it for its own clock */
static uint64_t packet_time(void)
{
	return os_gettime_ns() + 3600 * SEC;
}

enum key { PUSH_TO_TALK, PUSH_TO_MUTE };

static void press(obs_source_t *source, enum key key, bool pressed)
{
	obs_hotkey_trigger_routed_callback(key == PUSH_TO_TALK ? source->push_to_talk_key : source->push_to_mute_key,
					   pressed);
}

static uint64_t stop_time(obs_source_t *source, enum key key)
{
	return key == PUSH_TO_TALK ? source->push_to_talk_stop_time : source->push_to_mute_stop_time;
}

/* Enables the key, holds it for one packet and releases it; returns the stop
 * time that packet set and the bracket around it. A second packet after the
 * release reports in *muted_after whether the key's delay still holds. */
static uint64_t hold_once(obs_source_t *source, enum key key, uint64_t *before, uint64_t *after, bool *muted_after)
{
	if (key == PUSH_TO_TALK)
		obs_source_enable_push_to_talk(source, true);
	else
		obs_source_enable_push_to_mute(source, true);

	uint64_t ts = packet_time();
	press(source, key, true);
	struct capture held = send_packet(source, ts, before, after);
	/* held: push-to-talk lets it through, push-to-mute mutes it */
	assert_int_equal(held.muted, key == PUSH_TO_MUTE);
	press(source, key, false);

	uint64_t stop = stop_time(source, key);
	/* make sure a wrapped stop time just after the press would have passed */
	os_sleep_ms(5);
	struct capture released = send_packet(source, ts + (uint64_t)FRAMES * SEC / RATE, NULL, NULL);
	*muted_after = released.muted;
	return stop;
}

static void check_stop(uint64_t stop, uint64_t before, uint64_t after, uint64_t delay_ns)
{
	if (stop < before + delay_ns || stop > after + delay_ns) {
		fprintf(stderr, "stop time %llu outside [%llu, %llu] + %llu\n", (unsigned long long)stop,
			(unsigned long long)before, (unsigned long long)after, (unsigned long long)delay_ns);
		fail();
	}
}

static void check_saturated(uint64_t stop, uint64_t after)
{
	if (stop != UINT64_MAX) {
		fprintf(stderr, "stop time %llu, %s the press at %llu; expected UINT64_MAX\n", (unsigned long long)stop,
			stop < after ? "before" : "after", (unsigned long long)after);
		fail();
	}
}

/* --- delays through the setter ------------------------------------------ */

static void delay_zero_closes_at_release(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (int key = PUSH_TO_TALK; key <= PUSH_TO_MUTE; key++) {
		obs_source_t *source = new_source();
		uint64_t before, after;
		bool muted;
		uint64_t stop = hold_once(source, key, &before, &after, &muted);
		check_stop(stop, before, after, 0);
		/* released and no delay: push-to-talk mutes again, push-to-mute
		 * stops muting */
		assert_int_equal(muted, key == PUSH_TO_TALK);
		release_source(source);
	}
}

static void delay_normal_holds_after_release(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (int key = PUSH_TO_TALK; key <= PUSH_TO_MUTE; key++) {
		obs_source_t *source = new_source();
		if (key == PUSH_TO_TALK)
			obs_source_set_push_to_talk_delay(source, 300);
		else
			obs_source_set_push_to_mute_delay(source, 300);
		uint64_t before, after;
		bool muted;
		uint64_t stop = hold_once(source, key, &before, &after, &muted);
		check_stop(stop, before, after, 300 * MSEC);
		/* within the delay the key still holds */
		assert_int_equal(muted, key == PUSH_TO_MUTE);
		release_source(source);
	}
}

/* delay * 1000000 wraps: the stop time saturates, and the key holds. */
static void delay_multiply_saturates(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	const uint64_t delay = UINT64_MAX / 1000000 + 1;
	for (int key = PUSH_TO_TALK; key <= PUSH_TO_MUTE; key++) {
		obs_source_t *source = new_source();
		if (key == PUSH_TO_TALK)
			obs_source_set_push_to_talk_delay(source, delay);
		else
			obs_source_set_push_to_mute_delay(source, delay);
		/* the API takes any uint64_t and gives it back */
		assert_true((key == PUSH_TO_TALK ? obs_source_get_push_to_talk_delay(source)
						 : obs_source_get_push_to_mute_delay(source)) == delay);
		uint64_t before, after;
		bool muted;
		uint64_t stop = hold_once(source, key, &before, &after, &muted);
		(void)before;
		check_saturated(stop, after);
		assert_int_equal(muted, key == PUSH_TO_MUTE);
		release_source(source);
	}
}

/* delay * 1000000 fits, the sum with the clock does not. */
static void delay_sum_saturates(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	const uint64_t delay = UINT64_MAX / 1000000;
	for (int key = PUSH_TO_TALK; key <= PUSH_TO_MUTE; key++) {
		obs_source_t *source = new_source();
		if (key == PUSH_TO_TALK)
			obs_source_set_push_to_talk_delay(source, delay);
		else
			obs_source_set_push_to_mute_delay(source, delay);
		uint64_t before, after;
		bool muted;
		uint64_t stop = hold_once(source, key, &before, &after, &muted);
		(void)before;
		check_saturated(stop, after);
		assert_int_equal(muted, key == PUSH_TO_MUTE);
		release_source(source);
	}
}

/* --- delays through a loaded scene collection --------------------------- */

static obs_source_t *load_with_delays(long long ptm_delay, long long ptt_delay)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_int(data, "push-to-mute-delay", ptm_delay);
	obs_data_set_int(data, "push-to-talk-delay", ptt_delay);
	obs_source_t *source = load_source(data);
	obs_data_release(data);
	return source;
}

static void loaded_negative_delay_is_zero(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	const long long negatives[] = {-1, -300, INT64_MIN};
	for (size_t i = 0; i < sizeof(negatives) / sizeof(negatives[0]); i++) {
		for (int key = PUSH_TO_TALK; key <= PUSH_TO_MUTE; key++) {
			obs_source_t *source = load_with_delays(negatives[i], negatives[i]);
			assert_true(obs_source_get_push_to_mute_delay(source) == 0);
			assert_true(obs_source_get_push_to_talk_delay(source) == 0);
			uint64_t before, after;
			bool muted;
			uint64_t stop = hold_once(source, key, &before, &after, &muted);
			check_stop(stop, before, after, 0);
			assert_int_equal(muted, key == PUSH_TO_TALK);
			release_source(source);
		}
	}
}

static void loaded_delays_keep_their_value(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = load_with_delays(0, 0);
	assert_true(obs_source_get_push_to_mute_delay(source) == 0);
	assert_true(obs_source_get_push_to_talk_delay(source) == 0);
	release_source(source);

	for (int key = PUSH_TO_TALK; key <= PUSH_TO_MUTE; key++) {
		source = load_with_delays(300, 300);
		assert_true(obs_source_get_push_to_mute_delay(source) == 300);
		assert_true(obs_source_get_push_to_talk_delay(source) == 300);
		uint64_t before, after;
		bool muted;
		uint64_t stop = hold_once(source, key, &before, &after, &muted);
		check_stop(stop, before, after, 300 * MSEC);
		assert_int_equal(muted, key == PUSH_TO_MUTE);
		release_source(source);
	}

	/* the largest delay a scene collection can hold; the stop time saturates */
	source = load_with_delays(INT64_MAX, INT64_MAX);
	assert_true(obs_source_get_push_to_talk_delay(source) == (uint64_t)INT64_MAX);
	uint64_t before, after;
	bool muted;
	uint64_t stop = hold_once(source, PUSH_TO_TALK, &before, &after, &muted);
	(void)before;
	check_saturated(stop, after);
	assert_false(muted);
	release_source(source);
}

/* --- balance ------------------------------------------------------------ */

struct balance_case {
	float in;
	float out;
};

static const struct balance_case out_of_range[] = {
	{-1.0f, 0.0f}, {2.0f, 1.0f}, {NAN, 0.5f}, {INFINITY, 1.0f}, {-INFINITY, 0.0f},
};

static const float valid_balances[] = {0.0f, 0.25f, 0.5f, 1.0f};

static void check_balance(obs_source_t *source, float expected, const char *what, float in)
{
	float got = obs_source_get_balance_value(source);
	if (bits_of(got) != bits_of(expected)) {
		fprintf(stderr, "%s %a: balance %a, expected %a\n", what, (double)in, (double)got, (double)expected);
		fail();
	}
}

/* What a packet comes out as at a valid balance. */
static struct capture samples_at(float balance)
{
	obs_source_t *source = new_source();
	obs_source_set_balance_value(source, balance);
	struct capture cap = send_packet(source, packet_time(), NULL, NULL);
	release_source(source);
	return cap;
}

static void check_samples(const struct capture *got, const struct capture *expected, float in)
{
	for (size_t i = 0; i < FRAMES; i++) {
		if (bits_of(got->left[i]) != bits_of(expected->left[i]) ||
		    bits_of(got->right[i]) != bits_of(expected->right[i])) {
			fprintf(stderr, "balance %a, frame %zu: got %a %a, expected %a %a\n", (double)in, i,
				(double)got->left[i], (double)got->right[i], (double)expected->left[i],
				(double)expected->right[i]);
			fail();
		}
	}
}

static void setter_clamps_balance(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < sizeof(out_of_range) / sizeof(out_of_range[0]); i++) {
		obs_source_t *source = new_source();
		obs_source_set_balance_value(source, out_of_range[i].in);
		check_balance(source, out_of_range[i].out, "set", out_of_range[i].in);
		struct capture got = send_packet(source, packet_time(), NULL, NULL);
		release_source(source);

		struct capture expected = samples_at(out_of_range[i].out);
		check_samples(&got, &expected, out_of_range[i].in);
	}
}

static void setter_keeps_valid_balance(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < sizeof(valid_balances) / sizeof(valid_balances[0]); i++) {
		obs_source_t *source = new_source();
		obs_source_set_balance_value(source, valid_balances[i]);
		check_balance(source, valid_balances[i], "set", valid_balances[i]);
		release_source(source);
	}

	/* 0.25 still goes through the sine law, with the C's float and double
	 * casts */
	const float left_gain = sinf((float)((double)(1.0f - 0.25f) * (M_PI / 2.0)));
	const float right_gain = sinf((float)((double)0.25f * (M_PI / 2.0)));
	struct capture expected = {0};
	for (size_t i = 0; i < FRAMES; i++) {
		expected.left[i] = left_in[i] * left_gain;
		expected.right[i] = right_in[i] * right_gain;
	}
	struct capture got = samples_at(0.25f);
	check_samples(&got, &expected, 0.25f);
}

static obs_source_t *load_with_balance(double balance)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_double(data, "balance", balance);
	obs_source_t *source = load_source(data);
	obs_data_release(data);
	return source;
}

static void loaded_balance_is_clamped(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < sizeof(out_of_range) / sizeof(out_of_range[0]); i++) {
		obs_source_t *source = load_with_balance((double)out_of_range[i].in);
		check_balance(source, out_of_range[i].out, "loaded", out_of_range[i].in);
		release_source(source);
	}

	/* a double beyond float's range */
	obs_source_t *source = load_with_balance(1e300);
	check_balance(source, 1.0f, "loaded", INFINITY);
	release_source(source);
	source = load_with_balance(-1e300);
	check_balance(source, 0.0f, "loaded", -INFINITY);
	release_source(source);

	for (size_t i = 0; i < sizeof(valid_balances) / sizeof(valid_balances[0]); i++) {
		source = load_with_balance((double)valid_balances[i]);
		check_balance(source, valid_balances[i], "loaded", valid_balances[i]);
		release_source(source);
	}

	/* no balance in the entry: the default */
	obs_data_t *data = obs_data_create();
	source = load_source(data);
	obs_data_release(data);
	check_balance(source, 0.5f, "default", 0.5f);
	release_source(source);
}

/* --- volume (#2108) ----------------------------------------------------- */

struct volume_case {
	float in;
	float out;
};

static const struct volume_case volume_out_of_range[] = {
	{NAN, 1.0f}, {-1.0f, 0.0f}, {-INFINITY, 0.0f}, {INFINITY, 20.0f}, {-FLT_MAX, 0.0f},
};

/* kept bit for bit: -0 compares equal to 0, so it is not negative; 19.95 is
 * +26 dB, the top of the Advanced Audio Properties spin box, and 20 its 2000% */
static const float valid_volumes[] = {0.0f, -0.0f, 0.5f, 1.0f, 19.952623f, 20.0f, 100.0f, FLT_MAX};

static void check_volume(obs_source_t *source, float expected, const char *what, float in)
{
	float got = obs_source_get_volume(source);
	if (bits_of(got) != bits_of(expected)) {
		fprintf(stderr, "%s %a: volume %a, expected %a\n", what, (double)in, (double)got, (double)expected);
		fail();
	}
}

/* The volume the mix applies next: the last volume action queued. The
 * sources are out of the mix, so nothing takes it off the queue. */
static void check_queued_volume(obs_source_t *source, float expected, const char *what, float in)
{
	bool found = false;
	float got = 0.0f;

	pthread_mutex_lock(&source->audio_actions_mutex);
	for (size_t i = 0; i < source->audio_actions.num; i++) {
		if (source->audio_actions.array[i].type == AUDIO_ACTION_VOL) {
			got = source->audio_actions.array[i].vol;
			found = true;
		}
	}
	pthread_mutex_unlock(&source->audio_actions_mutex);

	if (!found) {
		fprintf(stderr, "%s %a: no volume queued for the mix\n", what, (double)in);
		fail();
	}
	if (bits_of(got) != bits_of(expected)) {
		fprintf(stderr, "%s %a: mix volume %a, expected %a\n", what, (double)in, (double)got, (double)expected);
		fail();
	}
}

static void setter_maps_volume(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < sizeof(volume_out_of_range) / sizeof(volume_out_of_range[0]); i++) {
		obs_source_t *source = new_source();
		obs_source_set_volume(source, volume_out_of_range[i].in);
		check_volume(source, volume_out_of_range[i].out, "set", volume_out_of_range[i].in);
		check_queued_volume(source, volume_out_of_range[i].out, "set", volume_out_of_range[i].in);
		release_source(source);
	}
}

static void setter_keeps_valid_volume(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < sizeof(valid_volumes) / sizeof(valid_volumes[0]); i++) {
		obs_source_t *source = new_source();
		obs_source_set_volume(source, valid_volumes[i]);
		check_volume(source, valid_volumes[i], "set", valid_volumes[i]);
		check_queued_volume(source, valid_volumes[i], "set", valid_volumes[i]);
		release_source(source);
	}
}

struct volume_signal {
	float seen;
	bool replace;
	float replacement;
};

static void volume_signal_cb(void *param, calldata_t *data)
{
	struct volume_signal *sig = param;
	sig->seen = (float)calldata_float(data, "volume");
	if (sig->replace)
		calldata_set_float(data, "volume", sig->replacement);
}

/* The "volume" signal's volume is in/out: its handlers see the mapped
 * volume, and what they hand back is mapped too. The mix keeps the volume
 * the setter was given, as before. */
static void volume_signal_is_mapped(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < sizeof(volume_out_of_range) / sizeof(volume_out_of_range[0]); i++) {
		obs_source_t *source = new_source();
		struct volume_signal sig = {0};
		signal_handler_t *handler = obs_source_get_signal_handler(source);
		signal_handler_connect(handler, "volume", volume_signal_cb, &sig);

		obs_source_set_volume(source, volume_out_of_range[i].in);
		if (bits_of(sig.seen) != bits_of(volume_out_of_range[i].out)) {
			fprintf(stderr, "signal %a: handler saw %a, expected %a\n", (double)volume_out_of_range[i].in,
				(double)sig.seen, (double)volume_out_of_range[i].out);
			fail();
		}

		/* a handler that hands back an out-of-range volume */
		sig.replace = true;
		sig.replacement = volume_out_of_range[i].in;
		obs_source_set_volume(source, 0.5f);
		check_volume(source, volume_out_of_range[i].out, "handler", volume_out_of_range[i].in);
		check_queued_volume(source, 0.5f, "handler", volume_out_of_range[i].in);

		signal_handler_disconnect(handler, "volume", volume_signal_cb, &sig);
		release_source(source);
	}

	/* a handler that hands back a valid volume: kept, as before */
	obs_source_t *source = new_source();
	struct volume_signal sig = {.replace = true, .replacement = 0.25f};
	signal_handler_t *handler = obs_source_get_signal_handler(source);
	signal_handler_connect(handler, "volume", volume_signal_cb, &sig);
	obs_source_set_volume(source, 0.5f);
	assert_true(bits_of(sig.seen) == bits_of(0.5f));
	check_volume(source, 0.25f, "handler", 0.25f);
	check_queued_volume(source, 0.5f, "handler", 0.25f);
	signal_handler_disconnect(handler, "volume", volume_signal_cb, &sig);
	release_source(source);
}

static obs_source_t *load_with_volume(double volume)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_double(data, "volume", volume);
	obs_source_t *source = load_source(data);
	obs_data_release(data);
	return source;
}

static void loaded_volume_is_mapped(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (size_t i = 0; i < sizeof(volume_out_of_range) / sizeof(volume_out_of_range[0]); i++) {
		obs_source_t *source = load_with_volume((double)volume_out_of_range[i].in);
		check_volume(source, volume_out_of_range[i].out, "loaded", volume_out_of_range[i].in);
		check_queued_volume(source, volume_out_of_range[i].out, "loaded", volume_out_of_range[i].in);
		release_source(source);
	}

	/* a double beyond float's range: infinite as a float */
	obs_source_t *source = load_with_volume(1e300);
	check_volume(source, 20.0f, "loaded", INFINITY);
	check_queued_volume(source, 20.0f, "loaded", INFINITY);
	release_source(source);
	source = load_with_volume(-1e300);
	check_volume(source, 0.0f, "loaded", -INFINITY);
	check_queued_volume(source, 0.0f, "loaded", -INFINITY);
	release_source(source);

	for (size_t i = 0; i < sizeof(valid_volumes) / sizeof(valid_volumes[0]); i++) {
		source = load_with_volume((double)valid_volumes[i]);
		check_volume(source, valid_volumes[i], "loaded", valid_volumes[i]);
		check_queued_volume(source, valid_volumes[i], "loaded", valid_volumes[i]);
		release_source(source);
	}

	/* a double cast to float as before: 0.3 rounds the same way */
	source = load_with_volume(0.3);
	check_volume(source, (float)0.3, "loaded", (float)0.3);
	release_source(source);

	/* no volume in the entry: the default */
	obs_data_t *data = obs_data_create();
	source = load_source(data);
	obs_data_release(data);
	check_volume(source, 1.0f, "default", 1.0f);
	release_source(source);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);

	for (size_t i = 0; i < FRAMES; i++) {
		left_in[i] = 0.9f * (float)sin(2.0 * M_PI * 1000.0 * (double)i / RATE);
		right_in[i] = -0.6f * (float)cos(2.0 * M_PI * 440.0 * (double)i / RATE);
	}

	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		/* obs_startup() needs a display on Linux; skip without one. */
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}

	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = ar_name,
		.create = ar_create,
		.destroy = ar_destroy,
	};
	obs_register_source(&info);

	/* press the keys as the frontend does, through the router */
	obs_hotkey_enable_callback_rerouting(true);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(delay_zero_closes_at_release),  cmocka_unit_test(delay_normal_holds_after_release),
		cmocka_unit_test(delay_multiply_saturates),      cmocka_unit_test(delay_sum_saturates),
		cmocka_unit_test(loaded_negative_delay_is_zero), cmocka_unit_test(loaded_delays_keep_their_value),
		cmocka_unit_test(setter_clamps_balance),         cmocka_unit_test(setter_keeps_valid_balance),
		cmocka_unit_test(loaded_balance_is_clamped),     cmocka_unit_test(setter_maps_volume),
		cmocka_unit_test(setter_keeps_valid_volume),     cmocka_unit_test(volume_signal_is_mapped),
		cmocka_unit_test(loaded_volume_is_mapped),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
