#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include <pulse/channelmap.h>

#include "pulseaudio-monitor-format.h"

/* Characterization of the PulseAudio monitor's sample format, channel map
 * and volume, the arithmetic of catching up after a stall, and the
 * feedback-loop device match, before their Rust port. They moved from
 * pulseaudio-output.c and pulseaudio-wrapper.c into
 * pulseaudio-monitor-format.c unchanged; this test compiles that file into
 * itself, so it checks the C.
 *
 * The monitor plays every monitored source on the chosen sink. It asks the
 * sink for its spec and plays in that spec where OBS can produce it (U8,
 * S16LE, S32LE, FLOAT32LE; 1-6 or 8 channels), and as FLOAT32LE or stereo
 * where it cannot; the server converts. The resampler hands it interleaved
 * samples, and it applies the source's volume itself, in the sink format.
 *
 * With a volume above 1 (Advanced Audio Properties allows +26 dB,
 * obs-websocket 20x, a scene file anything), a loud sample of an integer
 * format leaves the type's range. It saturates to the range (u8 0..255,
 * s16, s32), as the server clips a float sample; converting it unclamped
 * was undefined behaviour and wrapped (bug 3). Results inside the range
 * are the same as without the clamp. */

static const pa_sample_format_t all_formats[] = {
	PA_SAMPLE_U8,        PA_SAMPLE_ALAW,      PA_SAMPLE_ULAW,     PA_SAMPLE_S16LE,   PA_SAMPLE_S16BE,
	PA_SAMPLE_FLOAT32LE, PA_SAMPLE_FLOAT32BE, PA_SAMPLE_S32LE,    PA_SAMPLE_S32BE,   PA_SAMPLE_S24LE,
	PA_SAMPLE_S24BE,     PA_SAMPLE_S24_32LE,  PA_SAMPLE_S24_32BE, PA_SAMPLE_INVALID,
};

#define N(a) (sizeof(a) / sizeof((a)[0]))

static enum audio_format expected_obs_format(pa_sample_format_t format)
{
	switch (format) {
	case PA_SAMPLE_U8:
		return AUDIO_FORMAT_U8BIT;
	case PA_SAMPLE_S16LE:
		return AUDIO_FORMAT_16BIT;
	case PA_SAMPLE_S32LE:
		return AUDIO_FORMAT_32BIT;
	case PA_SAMPLE_FLOAT32LE:
		return AUDIO_FORMAT_FLOAT;
	default:
		return AUDIO_FORMAT_UNKNOWN;
	}
}

static void sample_formats(void **state)
{
	(void)state;

	/* The pa_sample_format_t numbers are libpulse ABI. */
	assert_int_equal(PA_SAMPLE_U8, 0);
	assert_int_equal(PA_SAMPLE_S16LE, 3);
	assert_int_equal(PA_SAMPLE_FLOAT32LE, 5);
	assert_int_equal(PA_SAMPLE_S32LE, 7);
	assert_int_equal(PA_SAMPLE_S24_32BE, 12);
	assert_int_equal(PA_SAMPLE_MAX, 13);
	assert_int_equal(PA_SAMPLE_INVALID, -1);
	assert_int_equal(N(all_formats), PA_SAMPLE_MAX + 1);

	for (size_t i = 0; i < N(all_formats); i++) {
		pa_sample_format_t format = all_formats[i];
		enum audio_format obs_format = pulseaudio_to_obs_audio_format(format);
		if (obs_format != expected_obs_format(format)) {
			fprintf(stderr, "%s maps to audio_format %d\n", pa_sample_format_to_string(format),
				(int)obs_format);
			fail();
		}

		/* Big-endian, 24-bit, a-law and mu-law sinks (and an invalid
		 * spec) are played as FLOAT32LE. */
		pa_sample_format_t played = pulseaudio_monitor_sink_format(format);
		pa_sample_format_t expected = obs_format == AUDIO_FORMAT_UNKNOWN ? PA_SAMPLE_FLOAT32LE : format;
		if (played != expected) {
			fprintf(stderr, "a %s sink is played as %s\n", pa_sample_format_to_string(format),
				pa_sample_format_to_string(played));
			fail();
		}
	}

	assert_int_equal(pulseaudio_to_obs_audio_format(PA_SAMPLE_MAX), AUDIO_FORMAT_UNKNOWN);
	assert_int_equal(pulseaudio_monitor_sink_format(PA_SAMPLE_MAX), PA_SAMPLE_FLOAT32LE);
}

static void channel_counts(void **state)
{
	(void)state;

	const enum speaker_layout expected[] = {
		SPEAKERS_UNKNOWN, SPEAKERS_MONO,    SPEAKERS_STEREO,  SPEAKERS_2POINT1, SPEAKERS_4POINT0,
		SPEAKERS_4POINT1, SPEAKERS_5POINT1, SPEAKERS_UNKNOWN, SPEAKERS_7POINT1, SPEAKERS_UNKNOWN,
	};
	for (uint_fast32_t channels = 0; channels < N(expected); channels++)
		assert_int_equal(pulseaudio_channels_to_obs_speakers(channels), expected[channels]);

	const uint_fast32_t more[] = {10, 16, 32, PA_CHANNELS_MAX, 255, 256, UINT32_MAX};
	for (size_t i = 0; i < N(more); i++)
		assert_int_equal(pulseaudio_channels_to_obs_speakers(more[i]), SPEAKERS_UNKNOWN);

	/* 0, 7 and more than 8 channels are played as stereo; every other
	 * count as it is. */
	for (unsigned channels = 0; channels <= UINT8_MAX; channels++) {
		uint8_t played = pulseaudio_monitor_sink_channels((uint8_t)channels);
		bool supported = (channels >= 1 && channels <= 6) || channels == 8;
		if (played != (supported ? channels : 2)) {
			fprintf(stderr, "a %u-channel sink is played with %u channels\n", channels, (unsigned)played);
			fail();
		}
	}
}

struct layout_map {
	enum speaker_layout layout;
	uint8_t channels;
	pa_channel_position_t map[8];
};

#define FL PA_CHANNEL_POSITION_FRONT_LEFT
#define FR PA_CHANNEL_POSITION_FRONT_RIGHT
#define FC PA_CHANNEL_POSITION_FRONT_CENTER
#define LFE PA_CHANNEL_POSITION_LFE
#define RL PA_CHANNEL_POSITION_REAR_LEFT
#define RR PA_CHANNEL_POSITION_REAR_RIGHT
#define RC PA_CHANNEL_POSITION_REAR_CENTER
#define SL PA_CHANNEL_POSITION_SIDE_LEFT
#define SR PA_CHANNEL_POSITION_SIDE_RIGHT
#define MONO PA_CHANNEL_POSITION_MONO

/* map[] past channels is filled too (the default order); it is pinned so a
 * port returns the same bytes. */
static const struct layout_map layout_maps[] = {
	{SPEAKERS_UNKNOWN, 0, {FL, FR, FC, LFE, RL, RR, SL, SR}},
	{SPEAKERS_MONO, 1, {MONO, FR, FC, LFE, RL, RR, SL, SR}},
	{SPEAKERS_STEREO, 2, {FL, FR, FC, LFE, RL, RR, SL, SR}},
	{SPEAKERS_2POINT1, 3, {FL, FR, LFE, LFE, RL, RR, SL, SR}},
	{SPEAKERS_4POINT0, 4, {FL, FR, FC, RC, RL, RR, SL, SR}},
	{SPEAKERS_4POINT1, 5, {FL, FR, FC, LFE, RC, RR, SL, SR}},
	{SPEAKERS_5POINT1, 6, {FL, FR, FC, LFE, RL, RR, SL, SR}},
	{(enum speaker_layout)7, 0, {FL, FR, FC, LFE, RL, RR, SL, SR}},
	{SPEAKERS_7POINT1, 8, {FL, FR, FC, LFE, RL, RR, SL, SR}},
	{(enum speaker_layout)9, 0, {FL, FR, FC, LFE, RL, RR, SL, SR}},
	{(enum speaker_layout)32, 0, {FL, FR, FC, LFE, RL, RR, SL, SR}},
};

static void channel_maps(void **state)
{
	(void)state;

	/* The pa_channel_position_t numbers are libpulse ABI. */
	assert_int_equal(MONO, 0);
	assert_int_equal(FL, 1);
	assert_int_equal(FR, 2);
	assert_int_equal(FC, 3);
	assert_int_equal(RC, 4);
	assert_int_equal(RL, 5);
	assert_int_equal(RR, 6);
	assert_int_equal(LFE, 7);
	assert_int_equal(SL, 10);
	assert_int_equal(SR, 11);

	for (size_t i = 0; i < N(layout_maps); i++) {
		const struct layout_map *want = &layout_maps[i];
		pa_channel_map got = pulseaudio_channel_map(want->layout);
		if (got.channels != want->channels) {
			fprintf(stderr, "layout %d: %u channels\n", (int)want->layout, (unsigned)got.channels);
			fail();
		}
		for (size_t c = 0; c < 8; c++) {
			if (got.map[c] != want->map[c]) {
				fprintf(stderr, "layout %d: map[%zu] is %d, not %d\n", (int)want->layout, c,
					(int)got.map[c], (int)want->map[c]);
				fail();
			}
		}
	}

	/* Every count the monitor plays gets a valid map of that many
	 * channels, and different positions. */
	for (uint8_t channels = 1; channels <= 8; channels++) {
		enum speaker_layout layout = pulseaudio_channels_to_obs_speakers(channels);
		if (layout == SPEAKERS_UNKNOWN)
			continue;
		pa_channel_map map = pulseaudio_channel_map(layout);
		assert_int_equal(map.channels, channels);
		assert_true(pa_channel_map_valid(&map));
		for (uint8_t a = 0; a < channels; a++)
			for (uint8_t b = a + 1; b < channels; b++)
				assert_int_not_equal(map.map[a], map.map[b]);
	}
}

/* Runs the volume over samples with a guard sample on each side, and checks
 * the guards: it writes exactly frames * channels samples. */
#define GUARD 2

static void apply(pa_sample_format_t format, uint_fast8_t channels, float vol, void *samples, size_t sample_size,
		  size_t count)
{
	uint8_t buffer[(GUARD * 2 + 64) * 4];
	assert_true(count % channels == 0);
	assert_true((count + GUARD * 2) * sample_size <= sizeof(buffer));

	memset(buffer, 0xA5, sizeof(buffer));
	memcpy(buffer + GUARD * sample_size, samples, count * sample_size);
	uint8_t *data[1] = {buffer + GUARD * sample_size};

	pulseaudio_monitor_process_volume(format, channels, vol, data, (uint32_t)(count / channels));

	for (size_t i = 0; i < GUARD * sample_size; i++) {
		assert_int_equal(buffer[i], 0xA5);
		assert_int_equal(buffer[(GUARD + count) * sample_size + i], 0xA5);
	}
	memcpy(samples, data[0], count * sample_size);
}

static void volume_u8(void **state)
{
	(void)state;

	/* ((int)x - 128) * vol + 128, the offset added in float, then
	 * truncated. */
	const uint8_t in[] = {0, 1, 64, 127, 128, 129, 192, 254, 255, 100};
	const struct {
		float vol;
		uint8_t out[N(in)];
	} cases[] = {
		{0.0f, {128, 128, 128, 128, 128, 128, 128, 128, 128, 128}},
		{0.5f, {64, 64, 96, 127, 128, 128, 160, 191, 191, 114}},
		{0.25f, {96, 96, 112, 127, 128, 128, 144, 159, 159, 121}},
		{0.1f, {115, 115, 121, 127, 128, 128, 134, 140, 140, 125}},
		{1.0f, {0, 1, 64, 127, 128, 129, 192, 254, 255, 100}},
	};

	for (size_t i = 0; i < N(cases); i++) {
		for (uint_fast8_t channels = 1; channels <= 2; channels++) {
			uint8_t got[N(in)];
			memcpy(got, in, sizeof(in));
			apply(PA_SAMPLE_U8, channels, cases[i].vol, got, 1, N(in));
			assert_memory_equal(got, cases[i].out, sizeof(got));
		}
	}

	/* Above 1, samples that stay in range, and samples that leave it
	 * saturate to 0 or 255 (bug 3). */
	const struct {
		float vol;
		uint8_t in, out;
	} above[] = {
		{2.0f, 127, 126},       {2.0f, 128, 128},     {2.0f, 129, 130},       {2.0f, 100, 72},
		{2.0f, 64, 0},          {2.0f, 191, 254},     {19.952623f, 130, 167}, {19.952623f, 126, 88},
		{2.0f, 192, 255},       {2.0f, 255, 255},     {2.0f, 63, 0},          {2.0f, 0, 0},
		{4.0f, 200, 255},       {4.0f, 50, 0},        {19.952623f, 255, 255}, {19.952623f, 0, 0},
		{19.952623f, 135, 255}, {19.952623f, 121, 0},
	};
	for (size_t i = 0; i < N(above); i++) {
		uint8_t got = above[i].in;
		apply(PA_SAMPLE_U8, 1, above[i].vol, &got, 1, 1);
		assert_int_equal(got, above[i].out);
	}
}

static void volume_s16(void **state)
{
	(void)state;

	/* x * vol in float, truncated toward zero. */
	const int16_t in[] = {0, 1, -1, 3, -3, 100, -100, 12345, -12345, 32767, -32768, 32766};
	const struct {
		float vol;
		int16_t out[N(in)];
	} cases[] = {
		{0.0f, {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}},
		{0.5f, {0, 0, 0, 1, -1, 50, -50, 6172, -6172, 16383, -16384, 16383}},
		{0.25f, {0, 0, 0, 0, 0, 25, -25, 3086, -3086, 8191, -8192, 8191}},
		{0.1f, {0, 0, 0, 0, 0, 10, -10, 1234, -1234, 3276, -3276, 3276}},
		{0.9999f, {0, 0, 0, 2, -2, 99, -99, 12343, -12343, 32763, -32764, 32762}},
		{1.0f, {0, 1, -1, 3, -3, 100, -100, 12345, -12345, 32767, -32768, 32766}},
	};

	for (size_t i = 0; i < N(cases); i++) {
		for (uint_fast8_t channels = 1; channels <= 6; channels++) {
			if (N(in) % channels)
				continue;
			int16_t got[N(in)];
			memcpy(got, in, sizeof(in));
			apply(PA_SAMPLE_S16LE, channels, cases[i].vol, got, sizeof(int16_t), N(in));
			for (size_t s = 0; s < N(in); s++) {
				if (got[s] != cases[i].out[s]) {
					fprintf(stderr, "s16 %d * %g: %d, not %d\n", in[s], cases[i].vol, got[s],
						cases[i].out[s]);
					fail();
				}
			}
		}
	}

	/* Above 1, samples that stay in range, and samples that leave it
	 * saturate to INT16_MAX or INT16_MIN (bug 3). 19.952623 is +26 dB,
	 * the most Advanced Audio Properties sets. */
	const struct {
		float vol;
		int16_t in, out;
	} above[] = {
		{2.0f, 1000, 2000},
		{2.0f, -1000, -2000},
		{2.0f, 16383, 32766},
		{2.0f, -16384, -32768},
		{1.5f, 3, 4},
		{1.5f, -3, -4},
		{19.952623f, 100, 1995},
		{19.952623f, -100, -1995},
		{19.952623f, 1642, 32762},
		{2.0f, 20000, 32767},
		{2.0f, -20000, -32768},
		{2.0f, 16384, 32767},
		{2.0f, -16385, -32768},
		{2.0f, 32767, 32767},
		{2.0f, -32768, -32768},
		{4.0f, 10000, 32767},
		{4.0f, -10000, -32768},
		{19.952623f, 1643, 32767},
		{19.952623f, -1643, -32768},
		{19.952623f, 32767, 32767},
		{19.952623f, -32768, -32768},
	};
	for (size_t i = 0; i < N(above); i++) {
		int16_t got = above[i].in;
		apply(PA_SAMPLE_S16LE, 1, above[i].vol, &got, sizeof(got), 1);
		assert_int_equal(got, above[i].out);
	}
}

static void volume_s32(void **state)
{
	(void)state;

	/* x is converted to float first, so it keeps 24 bits: 123456789
	 * becomes 123456792, and INT32_MAX 2^31, before the multiply. */
	const int32_t in[] = {0, 1, -1, 3, -3, 1 << 24, -(1 << 24), 123456789, -123456789, INT32_MIN, INT32_MAX};
	const struct {
		float vol;
		int32_t out[N(in)];
	} cases[] = {
		{0.0f, {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}},
		{0.5f, {0, 0, 0, 1, -1, 1 << 23, -(1 << 23), 61728396, -61728396, -1073741824, 1073741824}},
		{0.25f, {0, 0, 0, 0, 0, 1 << 22, -(1 << 22), 30864198, -30864198, -536870912, 536870912}},
	};

	for (size_t i = 0; i < N(cases); i++) {
		int32_t got[N(in)];
		memcpy(got, in, sizeof(in));
		apply(PA_SAMPLE_S32LE, 1, cases[i].vol, got, sizeof(int32_t), N(in));
		for (size_t s = 0; s < N(in); s++) {
			if (got[s] != cases[i].out[s]) {
				fprintf(stderr, "s32 %d * %g: %d, not %d\n", in[s], cases[i].vol, got[s],
					cases[i].out[s]);
				fail();
			}
		}
	}

	/* At 1, the float round trip changes the low bits. INT32_MAX at 1
	 * becomes 2^31, out of range: the caller never applies a volume of 1
	 * (within EPSILON), so that case is not pinned. */
	int32_t one[] = {0, 1, -1, 123456789, -123456789, INT32_MIN, 16777217};
	const int32_t one_out[] = {0, 1, -1, 123456792, -123456792, INT32_MIN, 16777216};
	apply(PA_SAMPLE_S32LE, 1, 1.0f, one, sizeof(int32_t), N(one));
	assert_memory_equal(one, one_out, sizeof(one));

	/* Above 1, samples that stay in range, and samples that leave it
	 * saturate to INT32_MAX or INT32_MIN (bug 3). 2^30 * 2 is 2^31, one
	 * past INT32_MAX; -2^30 * 2 is INT32_MIN itself, in range. */
	const struct {
		float vol;
		int32_t in, out;
	} above[] = {
		{2.0f, 1 << 20, 1 << 21},           {2.0f, -(1 << 29), -(1 << 30)},      {19.952623f, 1000, 19952},
		{19.952623f, -1000, -19952},        {2.0f, 1 << 30, INT32_MAX},          {2.0f, -(1 << 30), INT32_MIN},
		{2.0f, (1 << 30) + 128, INT32_MAX}, {2.0f, -(1 << 30) - 128, INT32_MIN}, {2.0f, INT32_MAX, INT32_MAX},
		{2.0f, INT32_MIN, INT32_MIN},       {4.0f, 1000000000, INT32_MAX},       {4.0f, -1000000000, INT32_MIN},
		{19.952623f, 123456789, INT32_MAX}, {19.952623f, -123456789, INT32_MIN},
	};
	for (size_t i = 0; i < N(above); i++) {
		int32_t got = above[i].in;
		apply(PA_SAMPLE_S32LE, 1, above[i].vol, &got, sizeof(got), 1);
		assert_int_equal(got, above[i].out);
	}
}

static void volume_float(void **state)
{
	(void)state;

	/* No clamp: above 1 the samples leave [-1, 1] and the server clips
	 * them. That is defined, so it is pinned. */
	const float in[] = {0.0f, 1.0f, -1.0f, 0.5f, -0.25f, 0.75f, 1.5f};
	const struct {
		float vol;
		float out[N(in)];
	} cases[] = {
		{0.0f, {0.0f, 0.0f, -0.0f, 0.0f, -0.0f, 0.0f, 0.0f}},
		{0.5f, {0.0f, 0.5f, -0.5f, 0.25f, -0.125f, 0.375f, 0.75f}},
		{1.0f, {0.0f, 1.0f, -1.0f, 0.5f, -0.25f, 0.75f, 1.5f}},
		{2.0f, {0.0f, 2.0f, -2.0f, 1.0f, -0.5f, 1.5f, 3.0f}},
	};

	for (size_t i = 0; i < N(cases); i++) {
		float got[N(in)];
		memcpy(got, in, sizeof(in));
		apply(PA_SAMPLE_FLOAT32LE, 1, cases[i].vol, got, sizeof(float), N(in));
		assert_memory_equal(got, cases[i].out, sizeof(got));
	}

	float one = 0.3f;
	apply(PA_SAMPLE_FLOAT32LE, 1, 19.952623f, &one, sizeof(one), 1);
	assert_true(one == 0.3f * 19.952623f);
}

static void volume_other_formats_untouched(void **state)
{
	(void)state;

	for (size_t i = 0; i < N(all_formats); i++) {
		pa_sample_format_t format = all_formats[i];
		if (expected_obs_format(format) != AUDIO_FORMAT_UNKNOWN)
			continue;

		uint8_t got[24];
		uint8_t want[24];
		for (size_t b = 0; b < sizeof(want); b++)
			want[b] = (uint8_t)(b * 37 + 11);
		memcpy(got, want, sizeof(got));
		apply(format, 2, 0.5f, got, 4, 6);
		assert_memory_equal(got, want, sizeof(got));
	}
}

static void volume_frame_count(void **state)
{
	(void)state;

	/* frames * channels samples, for every count the monitor plays; the
	 * guards around them stay unchanged (apply() checks). */
	for (uint_fast8_t channels = 1; channels <= 8; channels++) {
		if (pulseaudio_channels_to_obs_speakers(channels) == SPEAKERS_UNKNOWN)
			continue;
		for (size_t frames = 0; frames * channels <= 16; frames++) {
			int32_t s32[16];
			int16_t s16[16];
			float f32[16];
			uint8_t u8[16];
			for (size_t s = 0; s < 16; s++) {
				s32[s] = 1000;
				s16[s] = 1000;
				f32[s] = 0.5f;
				u8[s] = 200;
			}
			size_t count = frames * channels;
			if (count) {
				apply(PA_SAMPLE_S32LE, channels, 0.5f, s32, sizeof(int32_t), count);
				apply(PA_SAMPLE_S16LE, channels, 0.5f, s16, sizeof(int16_t), count);
				apply(PA_SAMPLE_FLOAT32LE, channels, 0.5f, f32, sizeof(float), count);
				apply(PA_SAMPLE_U8, channels, 0.5f, u8, 1, count);
			}
			for (size_t s = 0; s < 16; s++) {
				bool done = s < count;
				assert_int_equal(s32[s], done ? 500 : 1000);
				assert_int_equal(s16[s], done ? 500 : 1000);
				assert_true(f32[s] == (done ? 0.25f : 0.5f));
				assert_int_equal(u8[s], done ? 164 : 200);
			}
		}
	}
}

static void queued_bytes(void **state)
{
	(void)state;

	const uint64_t bps = 48000 * 4; /* s16 stereo */

	/* What the server holds plus what is not written yet. */
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 0, bps, 0), 600);
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 0, bps, 50), 650);
	/* Less what it played since the timing update: 1 ms is 192 bytes. */
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 1000, bps, 0), 408);
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 1000, bps, 7), 415);
	/* Rounded down: 1 usec is 0.192 bytes, 6 usec 1.152. */
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 1, bps, 0), 600);
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 6, bps, 0), 599);
	/* Played past what it holds, or a read index ahead of the write
	 * index: nothing held. */
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 5000, bps, 0), 0);
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 400, 5000, bps, 30), 30);
	assert_int_equal(pulseaudio_monitor_queued_bytes(400, 1000, 0, bps, 30), 30);
	assert_int_equal(pulseaudio_monitor_queued_bytes(1000, 1000, 0, bps, 0), 0);
	/* Held exactly what was played. */
	assert_int_equal(pulseaudio_monitor_queued_bytes(1192, 1000, 1000, bps, 3), 3);
	/* Large indices (hours of 7.1 float at 192 kHz). */
	const int64_t big = 192000LL * 32 * 3600 * 10;
	assert_int_equal(pulseaudio_monitor_queued_bytes(big + 6144000, big, 1000000, 6144000, 0), 0);
	assert_int_equal(pulseaudio_monitor_queued_bytes(big + 6144000, big, 500000, 6144000, 0), 3072000);
}

static void drop_split(void **state)
{
	(void)state;

	struct pulseaudio_monitor_drop drop;

	/* 1 ms of s16 stereo at 48 kHz is 192 bytes. */
	drop = pulseaudio_monitor_drop_split(1000, 192000, 4, 1000);
	assert_int_equal(drop.unwritten, 192);
	assert_int_equal(drop.rewind, 0);
	/* Not enough unwritten: the rest is rewound. */
	drop = pulseaudio_monitor_drop_split(1000, 192000, 4, 100);
	assert_int_equal(drop.unwritten, 100);
	assert_int_equal(drop.rewind, 92);
	drop = pulseaudio_monitor_drop_split(1000, 192000, 4, 0);
	assert_int_equal(drop.unwritten, 0);
	assert_int_equal(drop.rewind, 192);
	drop = pulseaudio_monitor_drop_split(1000, 192000, 4, 192);
	assert_int_equal(drop.unwritten, 192);
	assert_int_equal(drop.rewind, 0);
	/* Nothing to drop. */
	drop = pulseaudio_monitor_drop_split(0, 192000, 4, 500);
	assert_int_equal(drop.unwritten, 0);
	assert_int_equal(drop.rewind, 0);

	/* Rounded down to whole frames, at every remainder: 3 channels of
	 * s32 are 12-byte frames, so at 1 byte per usec 12k + r usec drop
	 * 12k bytes. */
	for (size_t frame = 1; frame <= 32; frame++) {
		for (uint64_t usec = 0; usec < 4 * frame; usec++) {
			size_t want = (size_t)usec - (size_t)usec % frame;
			drop = pulseaudio_monitor_drop_split(usec, 1000000, frame, SIZE_MAX);
			assert_int_equal(drop.unwritten, want);
			assert_int_equal(drop.rewind, 0);
			drop = pulseaudio_monitor_drop_split(usec, 1000000, frame, 0);
			assert_int_equal(drop.unwritten, 0);
			assert_int_equal(drop.rewind, want);
			drop = pulseaudio_monitor_drop_split(usec, 1000000, frame, frame + 1);
			assert_int_equal(drop.unwritten + drop.rewind, want);
			assert_int_equal(drop.unwritten, want < frame + 1 ? want : frame + 1);
		}
	}

	/* The byte count is rounded down before the frames: 10 usec of 6
	 * channels of s32 at 48 kHz (1.152 bytes per usec) is 11 bytes, less
	 * than one 24-byte frame. */
	drop = pulseaudio_monitor_drop_split(10, 1152000, 24, 100);
	assert_int_equal(drop.unwritten, 0);
	assert_int_equal(drop.rewind, 0);
	drop = pulseaudio_monitor_drop_split(1000000, 1152000, 24, 100);
	assert_int_equal(drop.unwritten, 100);
	assert_int_equal(drop.rewind, 1152000 - 100);
}

static void devices_match_rules(void **state)
{
	(void)state;

	const struct {
		const char *id1, *id2, *default_sink;
		bool match;
	} cases[] = {
		/* A source (id1) matches the output (id2) when it is that
		 * output's .monitor. */
		{"sinkA.monitor", "sinkA", NULL, true},
		{"sinkA", "sinkA", NULL, false},
		{"sinkB.monitor", "sinkA", NULL, false},
		{"sinkA.monitor", "sinkA.monitor", NULL, false},
		{"sinkA.monitor.monitor", "sinkA.monitor", NULL, true},
		{"SinkA.monitor", "sinkA", NULL, false},
		{"", "", NULL, false},
		{".monitor", "", NULL, true},
		/* "default" stands for the default sink. */
		{"default", "default", "sinkA", true},
		{"default", "sinkA", "sinkA", true},
		{"default", "sinkB", "sinkA", false},
		{"sinkA.monitor", "default", "sinkA", true},
		{"sinkB.monitor", "default", "sinkA", false},
		{"sinkA", "default", "sinkA", false},
		{"Default", "sinkA", "sinkA", false},
		{"sinkA.monitor", "Default", "sinkA", false},
		/* No default sink (""). */
		{"default", "default", "", true},
		{".monitor", "default", "", true},
		{"default", "", "", true},
		{"default", "sinkA", "", false},
		{"sinkA.monitor", "default", "", false},
		/* A default sink that is itself a monitor name. */
		{"default", "sinkA.monitor", "sinkA.monitor", true},
		{"sinkA.monitor.monitor", "default", "sinkA.monitor", true},
		/* NULL never matches. */
		{NULL, "sinkA", "sinkA", false},
		{"sinkA.monitor", NULL, "sinkA", false},
		{NULL, NULL, NULL, false},
		{NULL, "default", NULL, false},
		{"default", NULL, NULL, false},
	};

	for (size_t i = 0; i < N(cases); i++) {
		bool match = pulseaudio_monitor_devices_match(cases[i].id1, cases[i].id2, cases[i].default_sink);
		if (match != cases[i].match) {
			fprintf(stderr, "devices_match('%s', '%s') with default '%s': %d\n",
				cases[i].id1 ? cases[i].id1 : "(null)", cases[i].id2 ? cases[i].id2 : "(null)",
				cases[i].default_sink ? cases[i].default_sink : "(null)", match);
			fail();
		}
	}

	/* A long name. */
	char id[600], monitor[600 + 8];
	memset(id, 'x', sizeof(id) - 1);
	id[sizeof(id) - 1] = 0;
	snprintf(monitor, sizeof(monitor), "%s.monitor", id);
	assert_true(pulseaudio_monitor_devices_match(monitor, id, NULL));
	assert_true(pulseaudio_monitor_devices_match(monitor, "default", id));
	assert_true(pulseaudio_monitor_devices_match("default", id, id));
	assert_false(pulseaudio_monitor_devices_match(id, id, NULL));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(sample_formats),     cmocka_unit_test(channel_counts),
		cmocka_unit_test(channel_maps),       cmocka_unit_test(volume_u8),
		cmocka_unit_test(volume_s16),         cmocka_unit_test(volume_s32),
		cmocka_unit_test(volume_float),       cmocka_unit_test(volume_other_formats_untouched),
		cmocka_unit_test(volume_frame_count), cmocka_unit_test(queued_bytes),
		cmocka_unit_test(drop_split),         cmocka_unit_test(devices_match_rules),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
