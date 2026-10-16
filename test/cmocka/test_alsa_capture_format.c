/* linux-alsa: a device that offers 24-bit samples must be captured at 24 bits.
 *
 * USB audio interfaces often offer only S24_3LE (24-bit samples packed in 3
 * bytes) and S16_LE, through a hw or front: PCM with no plug layer. The
 * capture code tried S16_LE, S32_LE, FLOAT_LE, U8 in that order and has no
 * S24_3LE support, so such a device always ran at 16 bits and lost 8 bits of
 * resolution (noise floor near -96 dBFS instead of the device's own).
 *
 * The real capture code (plugins/linux-alsa/alsa-input.c) runs on ALSA's null
 * PCM, which needs no sound card. snd_pcm_hw_params_test_format is wrapped to
 * offer only S24_3LE and S16_LE, snd_pcm_readi is wrapped to deliver known
 * samples in the format that was set, and obs_source_output_audio is wrapped
 * to keep what OBS receives. The log is captured with base_set_log_handler:
 * it must name the format that was set, the only place where a user can see
 * the bit depth. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <alsa/asoundlib.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/platform.h>
#include <util/threading.h>

extern struct obs_source_info alsa_input_capture;

/* What linux-alsa.c's module boilerplate gives the capture code. */
const char *obs_module_text(const char *val)
{
	return val;
}

void __wrap_obs_source_set_async_compensation(obs_source_t *source, bool enable)
{
	(void)source;
	(void)enable;
}

/* 24-bit test values (two's complement, low 24 bits) and the 32-bit sample
 * OBS must receive for each: the 24 bits in the high bytes. */
#define VALUE_COUNT 6
static const uint32_t values24[VALUE_COUNT] = {0x7FFFFF, 0x800000, 0x000001, 0xFFFFFF, 0x123456, 0x000000};
static const int32_t expected32[VALUE_COUNT] = {INT32_MAX - 0xFF, INT32_MIN, 0x100, -0x100, 0x12345600, 0};

static pthread_mutex_t log_mutex = PTHREAD_MUTEX_INITIALIZER;
static bool log_has_format_line;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	char line[512];
	(void)level;
	(void)param;
	vsnprintf(line, sizeof(line), format, args);
	pthread_mutex_lock(&log_mutex);
	if (strcmp(line, "alsa-input: PCM 'null' format set to S24_3LE") == 0)
		log_has_format_line = true;
	pthread_mutex_unlock(&log_mutex);
}

static pthread_mutex_t got_mutex = PTHREAD_MUTEX_INITIALIZER;
static volatile bool got_audio;
static enum audio_format got_format;
static uint32_t got_frames;
static int32_t got_samples[2 * VALUE_COUNT];

void __wrap_obs_source_output_audio(obs_source_t *source, const struct obs_source_audio *audio)
{
	(void)source;
	pthread_mutex_lock(&got_mutex);
	if (!got_audio) {
		got_format = audio->format;
		got_frames = audio->frames;
		if (audio->format == AUDIO_FORMAT_32BIT && audio->frames >= VALUE_COUNT)
			memcpy(got_samples, audio->data[0], sizeof(got_samples));
		os_atomic_set_bool(&got_audio, true);
	}
	pthread_mutex_unlock(&got_mutex);
}

/* The device: S24_3LE and S16_LE only. */
int __real_snd_pcm_hw_params_test_format(snd_pcm_t *pcm, snd_pcm_hw_params_t *params, snd_pcm_format_t val);
int __wrap_snd_pcm_hw_params_test_format(snd_pcm_t *pcm, snd_pcm_hw_params_t *params, snd_pcm_format_t val)
{
	if (val != SND_PCM_FORMAT_S24_3LE && val != SND_PCM_FORMAT_S16_LE)
		return -EINVAL;
	return __real_snd_pcm_hw_params_test_format(pcm, params, val);
}

/* Each frame: the same test value on both channels, value i in frame i. */
snd_pcm_sframes_t __wrap_snd_pcm_readi(snd_pcm_t *pcm, void *buffer, snd_pcm_uframes_t size)
{
	snd_pcm_hw_params_t *params;
	snd_pcm_format_t format;
	uint8_t *out = buffer;

	snd_pcm_hw_params_alloca(&params);
	if (snd_pcm_hw_params_current(pcm, params) < 0 || snd_pcm_hw_params_get_format(params, &format) < 0)
		return -EIO;

	for (snd_pcm_uframes_t f = 0; f < size; f++) {
		uint32_t v = values24[f % VALUE_COUNT];
		for (int ch = 0; ch < 2; ch++) {
			if (format == SND_PCM_FORMAT_S24_3LE) {
				*out++ = (uint8_t)v;
				*out++ = (uint8_t)(v >> 8);
				*out++ = (uint8_t)(v >> 16);
			} else {
				int16_t s = (int16_t)(v >> 8);
				memcpy(out, &s, sizeof(s));
				out += sizeof(s);
			}
		}
	}
	os_sleep_ms(5);
	return (snd_pcm_sframes_t)size;
}

/* Captures from the null PCM until OBS gets its first audio (the capture code
 * holds audio back for 500 ms after the start). */
static void s24_3le_device_is_captured_at_24_bits(void **state)
{
	(void)state;
	base_set_log_handler(capture_log, NULL);
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", "null");
	obs_data_set_int(settings, "rate", 48000);
	void *data = alsa_input_capture.create(settings, NULL);
	obs_data_release(settings);
	assert_non_null(data);

	for (int i = 0; i < 300 && !os_atomic_load_bool(&got_audio); i++)
		os_sleep_ms(10);
	alsa_input_capture.destroy(data);
	base_set_log_handler(NULL, NULL);

	if (!os_atomic_load_bool(&got_audio))
		fail_msg("no audio reached OBS within 3 s");
	if (got_format != AUDIO_FORMAT_32BIT)
		fail_msg("OBS got audio format %d, expected AUDIO_FORMAT_32BIT (%d): the S24_3LE device ran at "
			 "a lower bit depth",
			 got_format, AUDIO_FORMAT_32BIT);
	if (!log_has_format_line)
		fail_msg("the log does not name the format: no line \"alsa-input: PCM 'null' format set to S24_3LE\"");
	assert_true(got_frames >= VALUE_COUNT);
	for (int i = 0; i < 2 * VALUE_COUNT; i++) {
		if (got_samples[i] != expected32[i / 2])
			fail_msg("sample %d: got 0x%08x, expected 0x%08x (24-bit 0x%06x)", i, (uint32_t)got_samples[i],
				 (uint32_t)expected32[i / 2], values24[i / 2]);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(s24_3le_device_is_captured_at_24_bits),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
