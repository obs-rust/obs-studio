/*
 * VLC Video Source must hand its audio to libobs as it plays, not in
 * one-second bursts (upstream obsproject/obs-studio#2406, reported by
 * univrsal).
 *
 * With an Opus file the source's volume meter flickers on and off: it
 * shows a level for a moment, then resets about every second, although the
 * audio itself plays and records fine. CodyJung traced it to the timing of
 * the audio: libVLC's Ogg and Matroska demuxers decode up to two seconds
 * ahead and pass the audio to the source in bursts, about a second of it at
 * once and then nothing for 0.9 s, and the source forwarded every packet to
 * libobs the moment it arrived. Volume meters (and every other
 * obs_source_add_audio_capture_callback user, such as obs-websocket's
 * InputVolumeMeters) see the audio when it is handed over, so they show a
 * second of audio at once and then reset when nothing has come for 0.5 s.
 *
 * The cases play generated files through the real vlc_source module and
 * record what the source hands to libobs: every hand-over (frames, timestamp
 * and when it came), and the program mix. During playback no hand-over may
 * carry more than 50 ms of audio, consecutive hand-overs may be at most
 * 100 ms apart, the timestamps must run on without gaps and the whole file
 * must reach the program mix. Opus and Vorbis in Ogg come in bursts of about
 * a second, AAC in MP4 in bursts of a quarter of a second; WAV and Opus in
 * WebM, which libVLC already hands over steadily, are the controls.
 *
 * The cases need core video (run under xvfb-run) and skip without a display
 * or when libVLC is not installed; the test then exits with 77 so ctest
 * reports it as skipped. The encoded cases also skip when FFmpeg has no
 * encoder for the codec or libVLC plays no audio from the file.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/channel_layout.h>
#include <libavutil/log.h>

#include <obs.h>
#include <obs-module.h>
#include <util/platform.h>
#include <util/threading.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#define RATE 48000
#define FILE_SECONDS 4.0
#define FILE_FRAMES ((uint32_t)(RATE * FILE_SECONDS))
#define TONE_HZ 1000.0
#define LEVEL 0.5

/* Limits for the hand-overs during playback. The meter resets after 0.5 s
 * without audio; libVLC's bursts are 0.9 s apart. */
#define MAX_CALL_MS 50.0
#define MAX_GAP_MS 100.0
/* libVLC opens the file and starts its clock; the first hand-overs come
 * together. Start measuring the gaps after this much of the playback. */
#define SETTLE_MS 300.0

static char tmp_dir[400];

static void make_path(char *dst, size_t size, const char *name)
{
	snprintf(dst, size, "%s/obs-vlc-pacing-%llu-%s", tmp_dir, (unsigned long long)os_gettime_ns(), name);
}

/* ------------------------------------------------------------------------ */
/* Files                                                                    */

static float tone(uint32_t i)
{
	return (float)(LEVEL * sin(2.0 * M_PI * TONE_HZ * i / RATE));
}

static void put_u16(FILE *f, uint16_t v)
{
	fwrite(&v, 2, 1, f);
}

static void put_u32(FILE *f, uint32_t v)
{
	fwrite(&v, 4, 1, f);
}

/* Stereo 32-bit float WAV of the tone. */
static bool write_wav(const char *path)
{
	FILE *f = fopen(path, "wb");
	if (!f)
		return false;

	const uint32_t data_size = FILE_FRAMES * 2 * 4;
	fwrite("RIFF", 1, 4, f);
	put_u32(f, 4 + 8 + 16 + 8 + data_size);
	fwrite("WAVE", 1, 4, f);
	fwrite("fmt ", 1, 4, f);
	put_u32(f, 16);
	put_u16(f, 3); /* WAVE_FORMAT_IEEE_FLOAT */
	put_u16(f, 2);
	put_u32(f, RATE);
	put_u32(f, RATE * 2 * 4);
	put_u16(f, 2 * 4);
	put_u16(f, 32);
	fwrite("data", 1, 4, f);
	put_u32(f, data_size);
	for (uint32_t i = 0; i < FILE_FRAMES; i++) {
		float v = tone(i);
		fwrite(&v, 4, 1, f);
		fwrite(&v, 4, 1, f);
	}
	fclose(f);
	return true;
}

/* An FFmpeg encoder for the codec: the external library's (libopus,
 * libvorbis) when FFmpeg has it, else FFmpeg's own. */
static const AVCodec *find_encoder(enum AVCodecID id)
{
	const char *lib = id == AV_CODEC_ID_OPUS ? "libopus" : id == AV_CODEC_ID_VORBIS ? "libvorbis" : NULL;
	const AVCodec *codec = lib ? avcodec_find_encoder_by_name(lib) : NULL;
	return codec ? codec : avcodec_find_encoder(id);
}

static bool encode(AVFormatContext *oc, AVCodecContext *enc, AVStream *st, AVFrame *frame)
{
	AVPacket *pkt = av_packet_alloc();
	bool ok = pkt && avcodec_send_frame(enc, frame) >= 0;

	while (ok) {
		int ret = avcodec_receive_packet(enc, pkt);
		if (ret == AVERROR(EAGAIN) || ret == AVERROR_EOF)
			break;
		if (ret < 0) {
			ok = false;
			break;
		}
		pkt->stream_index = st->index;
		av_packet_rescale_ts(pkt, enc->time_base, st->time_base);
		ok = av_interleaved_write_frame(oc, pkt) >= 0;
	}

	av_packet_free(&pkt);
	return ok;
}

/* The tone, stereo, in the given codec and container ("ogg", "webm", "mp4"). */
static bool write_encoded(const char *path, const char *format, enum AVCodecID id)
{
	const AVCodec *codec = find_encoder(id);
	const AVOutputFormat *fmt = av_guess_format(format, NULL, NULL);
	if (!codec || !fmt)
		return false;

	AVFormatContext *oc = NULL;
	AVCodecContext *enc = avcodec_alloc_context3(codec);
	AVFrame *frame = av_frame_alloc();
	bool ok = false;
	bool opened = false;

	if (!enc || !frame || avformat_alloc_output_context2(&oc, fmt, NULL, path) < 0)
		goto done;

	/* libopus takes interleaved floats, the others planar ones */
	enc->sample_fmt = strcmp(codec->name, "libopus") == 0 ? AV_SAMPLE_FMT_FLT : AV_SAMPLE_FMT_FLTP;
	enc->sample_rate = RATE;
	enc->bit_rate = 128000;
	enc->time_base = (AVRational){1, RATE};
	/* FFmpeg's own Opus and Vorbis encoders are experimental */
	enc->strict_std_compliance = FF_COMPLIANCE_EXPERIMENTAL;
	av_channel_layout_default(&enc->ch_layout, 2);
	if (oc->oformat->flags & AVFMT_GLOBALHEADER)
		enc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
	if (avcodec_open2(enc, codec, NULL) < 0 || enc->frame_size <= 0)
		goto done;

	AVStream *st = avformat_new_stream(oc, NULL);
	if (!st || avcodec_parameters_from_context(st->codecpar, enc) < 0)
		goto done;
	st->time_base = enc->time_base;

	if (avio_open(&oc->pb, path, AVIO_FLAG_WRITE) < 0)
		goto done;
	opened = true;
	if (avformat_write_header(oc, NULL) < 0)
		goto done;

	frame->format = enc->sample_fmt;
	frame->sample_rate = RATE;
	frame->nb_samples = enc->frame_size;
	if (av_channel_layout_copy(&frame->ch_layout, &enc->ch_layout) < 0 || av_frame_get_buffer(frame, 0) < 0)
		goto done;

	ok = true;
	for (uint32_t pos = 0; ok && pos < FILE_FRAMES; pos += (uint32_t)enc->frame_size) {
		if (av_frame_make_writable(frame) < 0) {
			ok = false;
			break;
		}
		for (int s = 0; s < enc->frame_size; s++) {
			float v = pos + s < FILE_FRAMES ? tone(pos + s) : 0.0f;
			if (enc->sample_fmt == AV_SAMPLE_FMT_FLT) {
				((float *)frame->data[0])[2 * s] = v;
				((float *)frame->data[0])[2 * s + 1] = v;
			} else {
				((float *)frame->data[0])[s] = v;
				((float *)frame->data[1])[s] = v;
			}
		}
		frame->pts = pos;
		ok = encode(oc, enc, st, frame);
	}
	ok = ok && encode(oc, enc, st, NULL);
	ok = ok && av_write_trailer(oc) >= 0;

done:
	if (opened)
		avio_closep(&oc->pb);
	avformat_free_context(oc);
	av_frame_free(&frame);
	avcodec_free_context(&enc);
	return ok;
}

/* What the written file holds (FFmpeg versions mux differently): its audio
 * codec and the duration of its audio packets. */
static bool read_file(const char *path, enum AVCodecID *codec, double *seconds)
{
	AVFormatContext *ic = NULL;
	if (avformat_open_input(&ic, path, NULL, NULL) < 0)
		return false;

	bool ok = false;
	int idx = avformat_find_stream_info(ic, NULL) >= 0
			  ? av_find_best_stream(ic, AVMEDIA_TYPE_AUDIO, -1, -1, NULL, 0)
			  : -1;
	AVPacket *pkt = av_packet_alloc();
	if (idx >= 0 && pkt) {
		AVStream *st = ic->streams[idx];
		int64_t duration = 0;
		while (av_read_frame(ic, pkt) >= 0) {
			if (pkt->stream_index == idx)
				duration += pkt->duration;
			av_packet_unref(pkt);
		}
		*codec = st->codecpar->codec_id;
		*seconds = (double)duration * av_q2d(st->time_base);
		ok = true;
	}

	av_packet_free(&pkt);
	avformat_close_input(&ic);
	return ok;
}

/* ------------------------------------------------------------------------ */
/* Capture                                                                  */

#define MAX_CALLS 4096

struct call {
	uint64_t at; /* os_gettime_ns() when it came */
	uint64_t timestamp;
	uint32_t frames;
	uint32_t rate;
};

struct capture {
	pthread_mutex_t mutex;
	struct call calls[MAX_CALLS];
	size_t num_calls;
	uint64_t frames;

	/* program mix */
	uint64_t mix_sound_frames;
	uint64_t frames_since_sound;
	uint64_t paused_sound_frames;
	uint64_t mixed_since_heard;
	int64_t max_lead; /* frames handed over but not yet mixed */
	bool heard;
};

static void source_cb(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	struct capture *cap = param;
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(muted);

	struct obs_audio_info oai;
	obs_get_audio_info(&oai);

	pthread_mutex_lock(&cap->mutex);
	if (cap->num_calls < MAX_CALLS) {
		struct call *c = &cap->calls[cap->num_calls++];
		c->at = os_gettime_ns();
		c->timestamp = audio->timestamp;
		c->frames = audio->frames;
		c->rate = oai.samples_per_sec;
	}
	cap->frames += audio->frames;
	pthread_mutex_unlock(&cap->mutex);
}

static void mix_cb(void *param, size_t mix_idx, struct audio_data *data)
{
	struct capture *cap = param;
	const float *l = (const float *)data->data[0];
	UNUSED_PARAMETER(mix_idx);

	double peak = 0.0;
	for (uint32_t i = 0; l && i < data->frames; i++) {
		double a = fabs((double)l[i]);
		if (a > peak)
			peak = a;
	}

	pthread_mutex_lock(&cap->mutex);
	if (cap->heard) {
		cap->mixed_since_heard += data->frames;
		int64_t lead = (int64_t)cap->frames - (int64_t)cap->mixed_since_heard;
		if (lead > cap->max_lead)
			cap->max_lead = lead;
	}
	if (peak > LEVEL / 2) {
		cap->heard = true;
		cap->frames_since_sound = 0;
		cap->mix_sound_frames += data->frames;
	} else {
		cap->frames_since_sound += data->frames;
	}
	pthread_mutex_unlock(&cap->mutex);
}

/* ------------------------------------------------------------------------ */
/* libobs setup                                                             */

/* libobs is up (a display is available) */
static bool obs_up;
/* ... and the module registered vlc_source (libVLC is installed) */
static bool vlc_ready;

static int start_obs(void **state)
{
	UNUSED_PARAMETER(state);
	av_log_set_level(AV_LOG_ERROR);

	const char *tmp = getenv("TMPDIR");
	snprintf(tmp_dir, sizeof(tmp_dir), "%s", tmp && *tmp ? tmp : "/tmp");

	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;

	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return 0;

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai))
		return 0;

	obs_up = true;

	obs_module_t *mod = NULL;
	if (obs_open_module(&mod, VLC_MODULE_PATH, VLC_DATA_PATH) != MODULE_SUCCESS || !obs_init_module(mod))
		return 0;

	/* the module only registers vlc_source when libVLC is installed */
	vlc_ready = obs_source_get_display_name("vlc_source") != NULL;
	return 0;
}

static int stop_obs(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

/* Plays path through vlc_source until it has been heard and the mix stayed
 * silent for 1.5 s, or 12 s. With pause_at, pauses it that long after it was
 * first heard for PAUSE_MS, and counts how much sound reaches the mix from
 * 300 ms into the pause until its end. */
#define PAUSE_MS 1000
static void play(const char *path, struct capture *cap, double pause_at)
{
	obs_data_t *settings = obs_data_create();
	obs_data_array_t *list = obs_data_array_create();
	obs_data_t *item = obs_data_create();
	obs_data_set_string(item, "value", path);
	obs_data_array_push_back(list, item);
	obs_data_release(item);
	obs_data_set_array(settings, "playlist", list);
	obs_data_array_release(list);
	obs_data_set_string(settings, "playback_behavior", "always_play");
	obs_data_set_bool(settings, "loop", false);

	memset(cap, 0, sizeof(*cap));
	pthread_mutex_init(&cap->mutex, NULL);
	obs_add_raw_audio_callback(0, NULL, mix_cb, cap);

	obs_source_t *src = obs_source_create("vlc_source", "vlc", settings, NULL);
	obs_data_release(settings);
	if (!src) {
		obs_remove_raw_audio_callback(0, mix_cb, cap);
		pthread_mutex_destroy(&cap->mutex);
		fprintf(stderr, "could not create vlc_source\n");
		fail();
	}
	obs_source_add_audio_capture_callback(src, source_cb, cap);
	obs_set_output_source(0, src);

	uint64_t deadline = os_gettime_ns() + 12000000000ULL;
	uint64_t heard_at = 0;
	bool paused = false;
	for (;;) {
		os_sleep_ms(20);
		pthread_mutex_lock(&cap->mutex);
		bool heard = cap->heard;
		bool done = cap->heard && cap->frames_since_sound > RATE * 3 / 2;
		pthread_mutex_unlock(&cap->mutex);
		if (heard && !heard_at)
			heard_at = os_gettime_ns();
		if (pause_at > 0.0 && !paused && heard_at && os_gettime_ns() - heard_at > pause_at * 1e9) {
			paused = true;
			obs_source_media_play_pause(src, true);
			os_sleep_ms(300);
			pthread_mutex_lock(&cap->mutex);
			uint64_t before = cap->mix_sound_frames;
			pthread_mutex_unlock(&cap->mutex);
			os_sleep_ms(PAUSE_MS - 300);
			pthread_mutex_lock(&cap->mutex);
			cap->paused_sound_frames = cap->mix_sound_frames - before;
			pthread_mutex_unlock(&cap->mutex);
			obs_source_media_play_pause(src, false);
			continue;
		}
		if (done || os_gettime_ns() > deadline)
			break;
	}

	obs_set_output_source(0, NULL);
	obs_source_remove_audio_capture_callback(src, source_cb, cap);
	obs_remove_raw_audio_callback(0, mix_cb, cap);
	obs_source_release(src);
	pthread_mutex_destroy(&cap->mutex);
}

/* Checks the hand-overs and the mix for a file of the given duration. */
static bool check(const char *what, const struct capture *cap, double seconds)
{
	bool ok = true;

	if (!cap->num_calls) {
		fprintf(stderr, "%s: vlc_source handed over no audio\n", what);
		return false;
	}

	const struct call *first = &cap->calls[0];
	const struct call *last = &cap->calls[cap->num_calls - 1];
	double max_call_ms = 0.0, max_gap_ms = 0.0, max_jump_ms = 0.0;
	double gap_at = 0.0;

	for (size_t i = 0; i < cap->num_calls; i++) {
		const struct call *c = &cap->calls[i];
		double ms = (double)c->frames * 1000.0 / c->rate;
		if (ms > max_call_ms)
			max_call_ms = ms;
		if (!i)
			continue;

		const struct call *p = &cap->calls[i - 1];
		double since_start = (double)(c->at - first->at) / 1e6;
		double gap = (double)(c->at - p->at) / 1e6;
		if (since_start > SETTLE_MS && gap > max_gap_ms) {
			max_gap_ms = gap;
			gap_at = since_start / 1000.0;
		}

		uint64_t expected = p->timestamp + util_mul_div64(p->frames, 1000000000ULL, p->rate);
		double jump = fabs((double)c->timestamp - (double)expected) / 1e6;
		if (jump > max_jump_ms)
			max_jump_ms = jump;
	}

	double handed = (double)cap->frames / RATE;
	double span = (double)(last->at - first->at) / 1e9;
	double heard = (double)cap->mix_sound_frames / RATE;
	fprintf(stderr,
		"%s: %zu hand-overs, %.3f s of audio over %.3f s; largest %.1f ms, longest pause %.1f ms "
		"(at %.3f s), largest timestamp jump %.1f ms; %.3f s of %.3f s in the mix; handed over up to "
		"%.3f s ahead of the mix\n",
		what, cap->num_calls, handed, span, max_call_ms, max_gap_ms, gap_at, max_jump_ms, heard, seconds,
		(double)cap->max_lead / RATE);

	if (max_call_ms > MAX_CALL_MS) {
		fprintf(stderr, "%s: a hand-over carried %.1f ms of audio (limit %.0f ms)\n", what, max_call_ms,
			MAX_CALL_MS);
		ok = false;
	}
	if (max_gap_ms > MAX_GAP_MS) {
		fprintf(stderr, "%s: no audio was handed over for %.1f ms during playback (limit %.0f ms)\n", what,
			max_gap_ms, MAX_GAP_MS);
		ok = false;
	}
	if (max_jump_ms > 2.0) {
		fprintf(stderr, "%s: the timestamps jumped by %.1f ms\n", what, max_jump_ms);
		ok = false;
	}
	if (handed < seconds - 0.1) {
		fprintf(stderr, "%s: only %.3f s of %.3f s were handed over\n", what, handed, seconds);
		ok = false;
	}
	if (heard < seconds - 0.2) {
		fprintf(stderr, "%s: only %.3f s of %.3f s reached the program mix\n", what, heard, seconds);
		ok = false;
	}
	return ok;
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                    */

static void play_encoded(const char *format, const char *ext, enum AVCodecID id)
{
	if (!obs_up)
		skip(); /* no display */
	if (!vlc_ready)
		skip(); /* libVLC is not installed */
	if (!find_encoder(id)) {
		fprintf(stderr, "%s: FFmpeg has no %s encoder\n", ext, avcodec_get_name(id));
		skip();
	}

	char path[512];
	make_path(path, sizeof(path), ext);
	if (!write_encoded(path, format, id)) {
		os_unlink(path);
		fprintf(stderr, "could not write %s\n", path);
		fail();
	}

	enum AVCodecID codec = AV_CODEC_ID_NONE;
	double seconds = 0.0;
	if (!read_file(path, &codec, &seconds) || codec != id || seconds < FILE_SECONDS - 0.05) {
		os_unlink(path);
		fprintf(stderr, "%s: the written file holds %.3f s of %s, not %.1f s of %s\n", ext, seconds,
			avcodec_get_name(codec), FILE_SECONDS, avcodec_get_name(id));
		fail();
	}

	struct capture *cap = bzalloc(sizeof(*cap));
	play(path, cap, 0.0);
	os_unlink(path);

	if (!cap->num_calls) {
		bfree(cap);
		fprintf(stderr, "%s: libVLC played no audio (no %s decoder or %s demuxer?)\n", ext,
			avcodec_get_name(id), format);
		skip();
	}

	/* Encoders pad the end to a whole packet; the decoder drops the
	 * padding, so the file plays for the tone's length. */
	bool ok = check(ext, cap, FILE_SECONDS);
	bfree(cap);
	if (!ok) {
		fprintf(stderr, "%s: vlc_source did not hand its audio over as it played\n", ext);
		fail();
	}
}

/* The report: an .opus file (Opus in Ogg). libVLC's Ogg demuxer hands over
 * about a second at a time. */
static void opus_in_ogg_is_handed_over_as_it_plays(void **state)
{
	UNUSED_PARAMETER(state);
	play_encoded("ogg", "opus", AV_CODEC_ID_OPUS);
}

/* Vorbis in Ogg: the same demuxer. */
static void vorbis_in_ogg_is_handed_over_as_it_plays(void **state)
{
	UNUSED_PARAMETER(state);
	play_encoded("ogg", "ogg", AV_CODEC_ID_VORBIS);
}

/* AAC in MP4: libVLC's MP4 demuxer hands over a quarter of a second at a
 * time. */
static void aac_in_mp4_is_handed_over_as_it_plays(void **state)
{
	UNUSED_PARAMETER(state);
	play_encoded("mp4", "m4a", AV_CODEC_ID_AAC);
}

/* Control: Opus in WebM (Matroska) is handed over steadily already. */
static void opus_in_webm_is_handed_over_as_it_plays(void **state)
{
	UNUSED_PARAMETER(state);
	play_encoded("webm", "webm", AV_CODEC_ID_OPUS);
}

/* Guard: holding the audio until it is due must not lose any of it when
 * the source is paused and resumed. (How much already handed-over audio
 * still plays during the pause is printed, not checked.) */
static void opus_in_ogg_pause_and_resume(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();
	if (!find_encoder(AV_CODEC_ID_OPUS))
		skip();

	char path[512];
	make_path(path, sizeof(path), "opus");
	if (!write_encoded(path, "ogg", AV_CODEC_ID_OPUS)) {
		os_unlink(path);
		fprintf(stderr, "could not write %s\n", path);
		fail();
	}

	struct capture *cap = bzalloc(sizeof(*cap));
	play(path, cap, 1.5);
	os_unlink(path);

	if (!cap->num_calls) {
		bfree(cap);
		skip(); /* libVLC cannot play it, see play_encoded() */
	}

	double handed = (double)cap->frames / RATE;
	double heard = (double)cap->mix_sound_frames / RATE;
	double paused = (double)cap->paused_sound_frames / RATE;
	fprintf(stderr, "pause: %.3f s handed over, %.3f s in the mix, %.3f s of it during the pause\n", handed, heard,
		paused);
	bfree(cap);

	bool ok = true;
	if (handed < FILE_SECONDS - 0.1 || heard < FILE_SECONDS - 0.3) {
		fprintf(stderr, "pause: audio went missing around the pause\n");
		ok = false;
	}
	if (!ok)
		fail();
}

/* Control: libVLC hands a WAV file over steadily already. */
static void wav_is_handed_over_as_it_plays(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	char path[512];
	make_path(path, sizeof(path), "wav");
	if (!write_wav(path)) {
		fprintf(stderr, "could not write %s\n", path);
		fail();
	}

	struct capture *cap = bzalloc(sizeof(*cap));
	play(path, cap, 0.0);
	os_unlink(path);

	bool ok = check("wav", cap, FILE_SECONDS);
	bfree(cap);
	if (!ok) {
		fprintf(stderr, "wav: vlc_source did not hand its audio over as it played\n");
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(wav_is_handed_over_as_it_plays),
		cmocka_unit_test(opus_in_webm_is_handed_over_as_it_plays),
		cmocka_unit_test(opus_in_ogg_is_handed_over_as_it_plays),
		cmocka_unit_test(vorbis_in_ogg_is_handed_over_as_it_plays),
		cmocka_unit_test(aac_in_mp4_is_handed_over_as_it_plays),
		cmocka_unit_test(opus_in_ogg_pause_and_resume),
	};

	int failed = cmocka_run_group_tests(tests, start_obs, stop_obs);
	if (failed)
		return failed;
	return vlc_ready ? 0 : 77;
}
