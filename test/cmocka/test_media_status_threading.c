/* Status queries must be safe while the media thread updates its state (#3572).
 * Include the plugin to exercise its actual private state setter/getter. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs-module.h>
#include <util/threading.h>
#include "shared/media-playback/media-playback/media-playback.h"
#include "shared/media-playback/media-playback/media.h"
#include "plugins/obs-ffmpeg/obs-ffmpeg-source.c.h"

const char *obs_module_text(const char *val)
{
	return val;
}

enum { STATUS_QUERIES = 10000 };

static void *change_state(void *data)
{
	for (int i = 0; i < STATUS_QUERIES; i++)
		set_media_state(data, i % 2 ? OBS_MEDIA_STATE_PLAYING : OBS_MEDIA_STATE_PAUSED);
	return NULL;
}

static void state_queries(void **unused)
{
	(void)unused;
	struct ffmpeg_source source = {0};
	pthread_t writer;
	assert_int_equal(pthread_create(&writer, NULL, change_state, &source), 0);
	bool valid = true;
	for (int i = 0; i < STATUS_QUERIES; i++) {
		enum obs_media_state state = ffmpeg_source_get_state(&source);
		valid &= state == OBS_MEDIA_STATE_NONE || state == OBS_MEDIA_STATE_PLAYING ||
			 state == OBS_MEDIA_STATE_PAUSED;
	}
	assert_int_equal(pthread_join(writer, NULL), 0);
	assert_true(valid);
	assert_int_equal(ffmpeg_source_get_state(&source), OBS_MEDIA_STATE_PLAYING);
}

static void *flush_decoder(void *data)
{
	struct mp_decode *decoder = data;
	for (int i = 0; i < STATUS_QUERIES; i++)
		mp_decode_flush(decoder);
	return NULL;
}

static void timeline_queries(void **unused)
{
	(void)unused;
	mp_media_t media = {0};
	pthread_mutex_init(&media.mutex, NULL);
	media.speed = 100;
	media.has_video = true;
	media.v.m = &media;
	media.v.next_pts = 1000000000;
	const AVCodec *codec = avcodec_find_decoder(AV_CODEC_ID_RAWVIDEO);
	media.v.decoder = avcodec_alloc_context3(codec);
	assert_non_null(media.v.decoder);
	media.v.decoder->width = 16;
	media.v.decoder->height = 16;
	media.v.decoder->pix_fmt = AV_PIX_FMT_YUV420P;
	assert_int_equal(avcodec_open2(media.v.decoder, codec, NULL), 0);
	pthread_t writer;
	assert_int_equal(pthread_create(&writer, NULL, flush_decoder, &media.v), 0);
	bool valid = true;
	for (int i = 0; i < STATUS_QUERIES; i++) {
		int64_t time = mp_media_get_current_time(&media);
		valid &= time == 0 || time == 1000;
	}
	assert_int_equal(pthread_join(writer, NULL), 0);
	assert_true(valid);
	assert_int_equal(mp_media_get_current_time(&media), 0);
	avcodec_free_context(&media.v.decoder);
	pthread_mutex_destroy(&media.mutex);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(state_queries), cmocka_unit_test(timeline_queries)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
