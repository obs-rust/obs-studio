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
 * packet into the device's buffer. This pins the layout map as it is
 * (audio_monitor_speaker_layout, moved unchanged from wasapi-output.c).
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

/* The listed masks map to their layouts, and any other mask to the layout
 * with the device's channel count where there is one. */
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

/* Characterized, not endorsed: any other mask gives the channel count cast
 * to a layout, which does not exist for 7 or more than 8 channels
 * (get_audio_channels() gives 0 for them), and a listed mask is used
 * whatever the device's channel count (#2559). */
static void unknown_counts_become_layouts_that_do_not_exist(void **state)
{
	(void)state;

	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | BC | SL | SR, 7), 7);
	assert_int_equal(get_audio_channels(audio_monitor_speaker_layout(FL | FR | FC | LFE | BC | SL | SR, 7)), 0);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | BL | BR | FLC | FRC | SL | SR, 10), 10);
	assert_int_equal(audio_monitor_speaker_layout(0, 7), 7);
	assert_int_equal(audio_monitor_speaker_layout(0, 12), 12);
	assert_int_equal(audio_monitor_speaker_layout(0, 0), SPEAKERS_UNKNOWN);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | FC | LFE | BL | BR | FLC | FRC, 2), SPEAKERS_7POINT1);
	assert_int_equal(audio_monitor_speaker_layout(FL | FR | LFE, 6), SPEAKERS_2POINT1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(known_devices_keep_their_layout),
		cmocka_unit_test(unknown_counts_become_layouts_that_do_not_exist),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
