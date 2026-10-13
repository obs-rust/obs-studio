/* Dynamic bitrate and a bitrate the user changes while live.
 *
 * With "Dynamically change bitrate to manage congestion" on, the RTMP output
 * remembers the video bitrate it had when the stream started and never
 * looks at it again. Changing the bitrate in Settings > Output while live
 * updates the encoder, but the next time the connection gets congested the
 * output lowers the bitrate and then climbs back to the bitrate the stream
 * started with, not to the one the user chose.
 *
 * The test builds the real rtmp-stream.c into this executable and drives
 * its dynamic bitrate code the way a stream does: init_connect() sets it up
 * from the encoder, and check_to_drop_frames() sees a send buffer that
 * holds more than 200 ms of video (congestion), then a series of expired
 * increase timers (recovery). The output, encoder and service are small
 * fakes, so no network, display or video is needed. The encoder's
 * "bitrate" setting is what the user sees in Settings and what the encoder
 * uses.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

/* Route the libobs calls the dynamic bitrate code makes to the fakes below.
 * The rest of rtmp-stream.c links against libobs as usual. */
#define obs_output_get_name fake_output_get_name
#define obs_output_get_delay fake_output_get_delay
#define obs_output_get_settings fake_output_get_settings
#define obs_output_get_video_encoder2 fake_output_get_video_encoder2
#define obs_output_get_audio_encoder fake_output_get_audio_encoder
#define obs_output_get_service fake_output_get_service
#define obs_encoder_get_name fake_encoder_get_name
#define obs_encoder_get_codec fake_encoder_get_codec
#define obs_encoder_update fake_encoder_update
#define obs_encoder_get_settings fake_encoder_get_settings
#define obs_encoder_get_caps fake_encoder_get_caps
#define obs_service_get_connect_info fake_service_get_connect_info

#include "rtmp-stream.c"

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

/* ------------------------------------------------------------------------- */
/* fakes */

struct fake_encoder {
	obs_data_t *settings;
};

static struct fake_encoder video_encoder;
static obs_data_t *output_settings;
static int fake_output;  /* only its address is used */
static int fake_service; /* only its address is used */

#define FAKE_OUTPUT ((obs_output_t *)&fake_output)
#define FAKE_ENCODER ((obs_encoder_t *)&video_encoder)

const char *fake_output_get_name(const obs_output_t *output)
{
	(void)output;
	return "dbr test";
}

uint32_t fake_output_get_delay(const obs_output_t *output)
{
	(void)output;
	return 0;
}

obs_data_t *fake_output_get_settings(const obs_output_t *output)
{
	(void)output;
	obs_data_addref(output_settings);
	return output_settings;
}

obs_encoder_t *fake_output_get_video_encoder2(const obs_output_t *output, size_t idx)
{
	(void)output;
	return idx == 0 ? FAKE_ENCODER : NULL;
}

obs_encoder_t *fake_output_get_audio_encoder(const obs_output_t *output, size_t idx)
{
	(void)output;
	(void)idx;
	return NULL;
}

obs_service_t *fake_output_get_service(const obs_output_t *output)
{
	(void)output;
	return (obs_service_t *)&fake_service;
}

const char *fake_service_get_connect_info(const obs_service_t *service, uint32_t type)
{
	(void)service;
	return type == OBS_SERVICE_CONNECT_INFO_SERVER_URL ? "rtmp://127.0.0.1/live" : "key";
}

const char *fake_encoder_get_name(const obs_encoder_t *encoder)
{
	(void)encoder;
	return "video encoder";
}

const char *fake_encoder_get_codec(const obs_encoder_t *encoder)
{
	(void)encoder;
	return "h264";
}

uint32_t fake_encoder_get_caps(const obs_encoder_t *encoder)
{
	(void)encoder;
	return OBS_ENCODER_CAP_DYN_BITRATE;
}

/* Like libobs: the new values are merged into the encoder's settings. */
void fake_encoder_update(obs_encoder_t *encoder, obs_data_t *settings)
{
	struct fake_encoder *enc = (struct fake_encoder *)encoder;
	if (settings != enc->settings)
		obs_data_apply(enc->settings, settings);
}

obs_data_t *fake_encoder_get_settings(const obs_encoder_t *encoder)
{
	const struct fake_encoder *enc = (const struct fake_encoder *)encoder;
	obs_data_addref(enc->settings);
	return enc->settings;
}

/* ------------------------------------------------------------------------- */
/* helpers */

static long encoder_bitrate(void)
{
	return (long)obs_data_get_int(video_encoder.settings, "bitrate");
}

/* What Settings > Output does when the bitrate is changed while live. */
static void user_sets_bitrate(long kbps)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "bitrate", kbps);
	obs_encoder_update(FAKE_ENCODER, settings);
	obs_data_release(settings);
}

/* The send buffer holds 500 ms of video while the measured send rate is
 * est_kbps: the output lowers the bitrate. */
static void congestion(struct rtmp_stream *stream, long est_kbps)
{
	for (int i = 0; i < 6; i++) {
		struct encoder_packet packet = {
			.type = OBS_ENCODER_VIDEO,
			.dts_usec = i * 100000,
			.keyframe = false,
		};
		deque_push_back(&stream->packets, &packet, sizeof(packet));
	}
	stream->last_dts_usec = 500000;

	pthread_mutex_lock(&stream->dbr_mutex);
	stream->dbr_est_bitrate = est_kbps;
	pthread_mutex_unlock(&stream->dbr_mutex);

	check_to_drop_frames(stream, false);
	free_packets(stream);
}

/* The connection is fine again: let every increase timer expire until the
 * output stops raising the bitrate. */
static void recovery(struct rtmp_stream *stream)
{
	for (int i = 0; i < 100; i++) {
		if (!stream->dbr_inc_timeout)
			break;
		stream->dbr_inc_timeout = 1;
		check_to_drop_frames(stream, false);
	}
}

static struct rtmp_stream *start_stream(long kbps)
{
	video_encoder.settings = obs_data_create();
	obs_data_set_int(video_encoder.settings, "bitrate", kbps);

	output_settings = obs_data_create();
	obs_data_set_bool(output_settings, OPT_DYN_BITRATE, true);
	obs_data_set_int(output_settings, OPT_DROP_THRESHOLD, 700);
	obs_data_set_int(output_settings, OPT_PFRAME_DROP_THRESHOLD, 900);

	struct rtmp_stream *stream = bzalloc(sizeof(*stream));
	stream->output = FAKE_OUTPUT;
	assert_int_equal(pthread_mutex_init(&stream->packets_mutex, NULL), 0);
	assert_int_equal(pthread_mutex_init(&stream->dbr_mutex, NULL), 0);
	assert_int_equal(os_event_init(&stream->stop_event, OS_EVENT_TYPE_MANUAL), 0);

	assert_true(init_connect(stream));
	assert_true(stream->dbr_enabled);
	return stream;
}

static void end_stream(struct rtmp_stream *stream)
{
	free_packets(stream);
	deque_free(&stream->packets);
	deque_free(&stream->dbr_frames);
	da_free(stream->dbr_interpolation_table);
	dstr_free(&stream->path);
	dstr_free(&stream->key);
	dstr_free(&stream->username);
	dstr_free(&stream->password);
	dstr_free(&stream->bind_ip);
	os_event_destroy(stream->stop_event);
	pthread_mutex_destroy(&stream->dbr_mutex);
	pthread_mutex_destroy(&stream->packets_mutex);
	bfree(stream);

	obs_data_release(output_settings);
	obs_data_release(video_encoder.settings);
	output_settings = NULL;
	video_encoder.settings = NULL;
}

/* ------------------------------------------------------------------------- */
/* cases */

/* The report: start at 6000, lower to 3000 while live, congestion, then
 * recovery. The bitrate must not climb back above 3000. */
static void lowered_bitrate_stays_the_ceiling(void **state)
{
	(void)state;
	struct rtmp_stream *stream = start_stream(6000);

	user_sets_bitrate(3000);
	congestion(stream, 2000);
	assert_int_equal(encoder_bitrate(), 2000);

	recovery(stream);
	if (encoder_bitrate() != 3000)
		fail_msg("lowered to 3000 while live: after congestion the bitrate went back to %ld",
			 encoder_bitrate());

	end_stream(stream);
}

/* Raising the bitrate while live: after congestion the output climbs back
 * to the new bitrate, not to the old one. */
static void raised_bitrate_is_reached_again(void **state)
{
	(void)state;
	struct rtmp_stream *stream = start_stream(3000);

	user_sets_bitrate(5000);
	congestion(stream, 2000);
	assert_int_equal(encoder_bitrate(), 2000);

	recovery(stream);
	if (encoder_bitrate() != 5000)
		fail_msg("raised to 5000 while live: after congestion the bitrate went back to %ld", encoder_bitrate());

	end_stream(stream);
}

/* Congestion with a send rate above the user's new, lower bitrate must not
 * raise the bitrate to the send rate. */
static void congestion_never_raises_above_the_new_bitrate(void **state)
{
	(void)state;
	struct rtmp_stream *stream = start_stream(6000);

	user_sets_bitrate(3000);
	congestion(stream, 4000);
	if (encoder_bitrate() > 3000)
		fail_msg("lowered to 3000 while live: congestion set the bitrate to %ld", encoder_bitrate());

	recovery(stream);
	assert_int_equal(encoder_bitrate(), 3000);

	end_stream(stream);
}

/* Guard: without a change by the user the output recovers to the bitrate
 * the stream started with. */
static void unchanged_bitrate_recovers_to_start(void **state)
{
	(void)state;
	struct rtmp_stream *stream = start_stream(6000);

	congestion(stream, 2000);
	assert_int_equal(encoder_bitrate(), 2000);

	recovery(stream);
	assert_int_equal(encoder_bitrate(), 6000);

	end_stream(stream);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(lowered_bitrate_stays_the_ceiling),
		cmocka_unit_test(raised_bitrate_is_reached_again),
		cmocka_unit_test(congestion_never_raises_above_the_new_bitrate),
		cmocka_unit_test(unchanged_bitrate_recovers_to_start),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
