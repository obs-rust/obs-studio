#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "monitor-speaker-layout.h"

/* The WASAPI monitor resamples OBS's audio to the speaker layout of the
 * device's mix format, then copies resample_frames * nChannels floats per
 * packet into the device's buffer. So the layout must have exactly the
 * device's channel count, or there is none and the monitor cannot play to
 * the device (#2559).
 *
 * The map is shared code (monitor-speaker-layout.h) so it runs here on every
 * platform; the WASAPI monitor itself only runs on Windows. */

/* ksmedia.h's SPEAKER_* bits */
#define FL 0x1U
#define FR 0x2U
#define FC 0x4U
#define LFE 0x8U
#define BL 0x10U
#define BR 0x20U
#define FLC 0x40U
#define FRC 0x80U
#define BC 0x100U
#define SL 0x200U
#define SR 0x400U
#define TC 0x800U

struct device {
	const char *name;
	uint32_t mask;
	uint32_t channels;
};

/* Masks devices report. The first five are the ones the monitor lists. */
static const struct device devices[] = {
	{"2.1", FL | FR | LFE, 3},
	{"surround", FL | FR | FC | BC, 4},
	{"4.1", FL | FR | FC | LFE | BC, 5},
	{"5.1 (back)", FL | FR | FC | LFE | BL | BR, 6},
	{"7.1 (wide)", FL | FR | FC | LFE | BL | BR | FLC | FRC, 8},
	{"mono", FC, 1},
	{"stereo", FL | FR, 2},
	{"quad", FL | FR | BL | BR, 4},
	{"5.1 (side)", FL | FR | FC | LFE | SL | SR, 6},
	{"6.1", FL | FR | FC | LFE | BC | SL | SR, 7},
	{"7.1 (surround)", FL | FR | FC | LFE | BL | BR | SL | SR, 8},
	{"10 outputs", FL | FR | FC | LFE | BL | BR | FLC | FRC | SL | SR, 10},
	{"12 outputs", FL | FR | FC | LFE | BL | BR | FLC | FRC | BC | SL | SR | TC, 12},
	{"no mask, 7", 0, 7},
	{"no mask, 10", 0, 10},
	{"no mask, 0", 0, 0},
};

static bool is_layout(enum speaker_layout layout)
{
	switch (layout) {
	case SPEAKERS_UNKNOWN:
	case SPEAKERS_MONO:
	case SPEAKERS_STEREO:
	case SPEAKERS_2POINT1:
	case SPEAKERS_4POINT0:
	case SPEAKERS_4POINT1:
	case SPEAKERS_5POINT1:
	case SPEAKERS_7POINT1:
		return true;
	}
	return false;
}

/* Every device gets a real layout with its own channel count, or none. */
static void layout_has_device_channel_count(void **state)
{
	(void)state;
	bool ok = true;

	for (size_t i = 0; i < sizeof(devices) / sizeof(devices[0]); i++) {
		const struct device *d = &devices[i];
		enum speaker_layout layout = audio_monitor_speaker_layout(d->mask, d->channels);

		if (!is_layout(layout)) {
			fprintf(stderr, "%s (mask 0x%03x, %u channels): layout %d does not exist\n", d->name,
				(unsigned)d->mask, (unsigned)d->channels, (int)layout);
			ok = false;
		} else if (layout != SPEAKERS_UNKNOWN && get_audio_channels(layout) != d->channels) {
			fprintf(stderr, "%s (mask 0x%03x, %u channels): layout %d has %u channels\n", d->name,
				(unsigned)d->mask, (unsigned)d->channels, (int)layout,
				(unsigned)get_audio_channels(layout));
			ok = false;
		}
	}

	if (!ok)
		fail();
}

/* A mask whose channel count disagrees with the device's is not used: the
 * monitor copies nChannels floats per frame. */
static void mask_with_other_count_is_not_used(void **state)
{
	(void)state;
	bool ok = true;

	for (size_t i = 0; i < sizeof(devices) / sizeof(devices[0]); i++) {
		const struct device *d = &devices[i];

		if (i > 0 && d->mask == devices[i - 1].mask)
			continue;

		for (uint32_t channels = 0; channels <= 16; channels++) {
			enum speaker_layout layout = audio_monitor_speaker_layout(d->mask, channels);

			if (layout == SPEAKERS_UNKNOWN)
				continue;
			if (!is_layout(layout) || get_audio_channels(layout) != channels) {
				fprintf(stderr, "mask 0x%03x with %u channels: layout %d\n", (unsigned)d->mask,
					(unsigned)channels, (int)layout);
				ok = false;
			}
		}
	}

	if (!ok)
		fail();
}

/* What the map gave before #2559 and still gives: the listed masks map to
 * their layouts, and any other mask to the layout with the device's channel
 * count where there is one. */
static void known_devices_keep_their_layout(void **state)
{
	(void)state;

	assert_int_equal(audio_monitor_speaker_layout(FL | FR | LFE, 3), SPEAKERS_2POINT1);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | BC, 4), SPEAKERS_4POINT0);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | BC, 5), SPEAKERS_4POINT1);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | BL | BR, 6), SPEAKERS_5POINT1);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | BL | BR | FLC | FRC, 8), SPEAKERS_7POINT1);

	assert_int_equal(audio_monitor_speaker_layout(FC, 1), SPEAKERS_MONO);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR, 2), SPEAKERS_STEREO);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | BL | BR, 4), SPEAKERS_4POINT0);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | SL | SR, 6), SPEAKERS_5POINT1);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | BL | BR | SL | SR, 8), SPEAKERS_7POINT1);
	assert_int_equal(audio_monitor_speaker_layout(0, 2), SPEAKERS_STEREO);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(layout_has_device_channel_count),
		cmocka_unit_test(mask_with_other_count_is_not_used),
		cmocka_unit_test(known_devices_keep_their_layout),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
