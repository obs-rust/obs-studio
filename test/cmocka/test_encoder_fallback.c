/* Settings > Output, Advanced: a profile that names an encoder this build
 * does not have (obsproject/obs-studio#3531, reported by YorVeX: an AMD
 * encoder in a profile copied to another machine) showed "No properties
 * available" under the encoder list, and the list showed whatever encoder
 * came first, so the settings could be neither viewed nor edited.
 *
 * encoder_list_select() is the step the settings dialog runs for each video
 * encoder list: keep the profile's encoder when the list offers it, else
 * pick the fallback Advanced output already uses when the service does not
 * take an encoder (adv_video_encoder_fallback()), else x264. The lists below
 * stand in for the dialog's encoder lists. Plain C, no libobs, no Qt. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include "encoder-fallback.h"

/* The encoders a list offers, null-terminated. */
static bool list_has(const char *id, void *param)
{
	for (const char *const *e = param; *e; e++) {
		if (id && strcmp(*e, id) == 0) {
			return true;
		}
	}
	return false;
}

/* Linux without AMD or NVIDIA: what the reporter's machine (and ours) offers. */
static const char *const linux_list[] = {"ffmpeg_svt_av1", "ffmpeg_aom_av1", "obs_x264", NULL};
/* Recording list: "none" is "(Use stream encoder)". */
static const char *const linux_rec_list[] = {"none", "ffmpeg_svt_av1", "ffmpeg_aom_av1", "obs_x264", NULL};
static const char *const amd_h264_list[] = {"h264_texture_amf", "obs_x264", NULL};
static const char *const no_x264_list[] = {"ffmpeg_svt_av1", NULL};

static const char *select_video(const char *configured, const char *const *list)
{
	const char *fallbacks[] = {adv_video_encoder_fallback(configured), "obs_x264", NULL};
	return encoder_list_select(configured, fallbacks, list_has, (void *)list);
}

/* An encoder the list offers stays selected (unchanged). */
static void test_offered_encoder_kept(void **state)
{
	(void)state;
	assert_string_equal(select_video("ffmpeg_svt_av1", linux_list), "ffmpeg_svt_av1");
	assert_string_equal(select_video("obs_x264", linux_list), "obs_x264");
	assert_string_equal(select_video("none", linux_rec_list), "none");
}

/* #3531: AdvOut Encoder=amd_amf_h264 on a machine without AMF. */
static void test_missing_encoder_falls_back_to_x264(void **state)
{
	(void)state;
	assert_string_equal(select_video("amd_amf_h264", linux_list), "obs_x264");
	assert_string_equal(select_video("amd_amf_h264", linux_rec_list), "obs_x264");
	assert_string_equal(select_video("jim_nvenc", linux_list), "obs_x264");
	assert_string_equal(select_video("com.apple.videotoolbox.videoencoder.ave.avc", linux_list), "obs_x264");
}

/* A missing HEVC or AV1 encoder falls back to the same vendor's H.264
 * encoder when the list has it, else to x264. */
static void test_missing_encoder_same_vendor_first(void **state)
{
	(void)state;
	assert_string_equal(select_video("h265_texture_amf", amd_h264_list), "h264_texture_amf");
	assert_string_equal(select_video("av1_texture_amf", amd_h264_list), "h264_texture_amf");
	assert_string_equal(select_video("h265_texture_amf", linux_list), "obs_x264");
}

/* No fallback in the list (x264 not loaded): NULL, the caller selects the
 * list's first entry. */
static void test_nothing_offered(void **state)
{
	(void)state;
	assert_null(select_video("amd_amf_h264", no_x264_list));
}

/* A profile without the key: config_get_string() returns NULL. */
static void test_unset_encoder(void **state)
{
	(void)state;
	const char *selected = select_video(NULL, linux_list);
	assert_non_null(selected);
	assert_string_equal(selected, "obs_x264");
}

/* The fallback table Advanced output also uses when the service does not
 * take the selected encoder (unchanged). */
static void test_fallback_table(void **state)
{
	(void)state;
	assert_string_equal(adv_video_encoder_fallback("obs_nvenc_hevc_tex"), "obs_nvenc_h264_tex");
	assert_string_equal(adv_video_encoder_fallback("obs_nvenc_av1_tex"), "obs_nvenc_h264_tex");
	assert_string_equal(adv_video_encoder_fallback("jim_hevc_nvenc"), "obs_nvenc_h264_tex");
	assert_string_equal(adv_video_encoder_fallback("jim_av1_nvenc"), "obs_nvenc_h264_tex");
	assert_string_equal(adv_video_encoder_fallback("h265_texture_amf"), "h264_texture_amf");
	assert_string_equal(adv_video_encoder_fallback("av1_texture_amf"), "h264_texture_amf");
	assert_string_equal(adv_video_encoder_fallback("com.apple.videotoolbox.videoencoder.ave.hevc"),
			    "com.apple.videotoolbox.videoencoder.ave.avc");
	assert_string_equal(adv_video_encoder_fallback("obs_qsv11_av1"), "obs_qsv11");
	assert_string_equal(adv_video_encoder_fallback("amd_amf_h264"), "obs_x264");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_offered_encoder_kept),
		cmocka_unit_test(test_missing_encoder_falls_back_to_x264),
		cmocka_unit_test(test_missing_encoder_same_vendor_first),
		cmocka_unit_test(test_nothing_offered),
		cmocka_unit_test(test_unset_encoder),
		cmocka_unit_test(test_fallback_table),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
