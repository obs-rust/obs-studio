/*
 * Remuxing an MKV recording to MP4 or MOV must keep the audio sample
 * accurate (upstream obsproject/obs-studio#13249, reported by
 * angeloruggieridj; also obsproject/obs-studio#4740): the remuxed file
 * crackled or popped in Final Cut Pro and QuickTime, while the MKV and an
 * MP4 made with `ffmpeg -c copy` from the same MKV played cleanly.
 *
 * Matroska stores timestamps in milliseconds, so an AAC frame of 1024
 * samples at 48 kHz (21.333 ms) is stored as 0, 21, 43, 64, ... ms. MP4 and
 * MOV store audio in samples and take each frame's length from the gap to
 * the next one. media_remux rescaled every millisecond timestamp on its
 * own, so the frames came out 1008 or 1056 samples long instead of 1024:
 * players that honour those lengths stretch or overlap the audio by 16 to
 * 32 samples at every frame boundary, which is heard as crackling.
 *
 * The cases encode a tone with FFmpeg's AAC encoder into an MKV, remux it
 * with media_remux_job_create()/media_remux_job_process() and read the
 * result back: every audio frame must start exactly one frame after the
 * previous one, the last one must last one frame, and a real gap in the
 * recording must still be there. No video or display is needed.
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

#include <util/platform.h>
#include <media-io/media-remux.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#define FRAMES 200
#define GAP_AT 100
#define GAP_FRAMES 10

struct clip {
	int rate;
	bool gap;      /* leave GAP_FRAMES frames out after frame GAP_AT */
	int short_end; /* samples left out of the last frame */
	char mkv[512];
	int64_t encoded;      /* packets from the encoder */
	int64_t muxed;        /* packets written to the MKV */
	int64_t last_samples; /* length of the last one */
};

static char tmp_dir[400];
static uint64_t run_id;
static int file_seq;

static void make_path(char *dst, size_t size, const char *ext)
{
	snprintf(dst, size, "%s/obs-remux-%llu-%d.%s", tmp_dir, (unsigned long long)run_id, file_seq++, ext);
}

/* ------------------------------------------------------------------------ */
/* Source MKV */

static bool write_frame(struct clip *clip, AVFormatContext *oc, AVCodecContext *enc, AVStream *st, AVFrame *frame)
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

		/* Start the file at 0 like an OBS recording. */
		pkt->pts += enc->initial_padding;
		pkt->dts += enc->initial_padding;
		int64_t index = clip->encoded++;
		if (clip->gap && index >= GAP_AT && index < GAP_AT + GAP_FRAMES) {
			av_packet_unref(pkt);
			continue;
		}

		clip->last_samples = pkt->duration;
		pkt->stream_index = st->index;
		av_packet_rescale_ts(pkt, enc->time_base, st->time_base);
		ok = av_interleaved_write_frame(oc, pkt) >= 0;
		clip->muxed++;
	}

	av_packet_free(&pkt);
	return ok;
}

/* FRAMES AAC frames of a 440 Hz tone, stereo, at the clip's sample rate. */
static bool write_mkv(struct clip *clip)
{
	const AVCodec *codec = avcodec_find_encoder(AV_CODEC_ID_AAC);
	const AVOutputFormat *fmt = av_guess_format("matroska", NULL, NULL);
	if (!codec || !fmt)
		return false;

	make_path(clip->mkv, sizeof(clip->mkv), "mkv");

	AVFormatContext *oc = NULL;
	AVCodecContext *enc = avcodec_alloc_context3(codec);
	AVFrame *frame = av_frame_alloc();
	bool ok = false;
	bool opened = false;

	if (!enc || !frame || avformat_alloc_output_context2(&oc, fmt, NULL, clip->mkv) < 0)
		goto done;

	enc->sample_fmt = AV_SAMPLE_FMT_FLTP;
	enc->sample_rate = clip->rate;
	enc->bit_rate = 128000;
	enc->time_base = (AVRational){1, clip->rate};
	av_channel_layout_default(&enc->ch_layout, 2);
	if (oc->oformat->flags & AVFMT_GLOBALHEADER)
		enc->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
	if (avcodec_open2(enc, codec, NULL) < 0 || enc->frame_size <= 0)
		goto done;

	AVStream *st = avformat_new_stream(oc, NULL);
	if (!st || avcodec_parameters_from_context(st->codecpar, enc) < 0)
		goto done;
	st->time_base = enc->time_base;

	if (avio_open(&oc->pb, clip->mkv, AVIO_FLAG_WRITE) < 0)
		goto done;
	opened = true;
	if (avformat_write_header(oc, NULL) < 0)
		goto done;

	frame->format = enc->sample_fmt;
	frame->sample_rate = enc->sample_rate;
	frame->nb_samples = enc->frame_size;
	if (av_channel_layout_copy(&frame->ch_layout, &enc->ch_layout) < 0 || av_frame_get_buffer(frame, 0) < 0)
		goto done;

	ok = true;
	for (int64_t i = 0; ok && i < FRAMES; i++) {
		if (av_frame_make_writable(frame) < 0) {
			ok = false;
			break;
		}
		frame->nb_samples = i == FRAMES - 1 ? enc->frame_size - clip->short_end : enc->frame_size;
		for (int c = 0; c < 2; c++) {
			float *dst = (float *)frame->data[c];
			for (int s = 0; s < frame->nb_samples; s++) {
				int64_t n = i * enc->frame_size + s;
				dst[s] = 0.5f * (float)sin(2.0 * M_PI * 440.0 * (double)n / clip->rate);
			}
		}
		frame->pts = i * enc->frame_size;
		ok = write_frame(clip, oc, enc, st, frame);
	}
	ok = ok && write_frame(clip, oc, enc, st, NULL);
	ok = ok && av_write_trailer(oc) >= 0;

done:
	if (opened)
		avio_closep(&oc->pb);
	avformat_free_context(oc);
	av_frame_free(&frame);
	avcodec_free_context(&enc);
	return ok;
}

/* ------------------------------------------------------------------------ */
/* Checks */

struct audio_track {
	int rate;
	int frame_size;
	int64_t count;
	int64_t dts[FRAMES * 2]; /* in samples */
	int64_t last_duration;   /* in samples */
};

static bool read_track(const char *path, struct audio_track *t)
{
	AVFormatContext *ic = NULL;
	if (avformat_open_input(&ic, path, NULL, NULL) < 0)
		return false;

	bool ok = false;
	int idx = avformat_find_stream_info(ic, NULL) >= 0
			  ? av_find_best_stream(ic, AVMEDIA_TYPE_AUDIO, -1, -1, NULL, 0)
			  : -1;
	AVPacket *pkt = av_packet_alloc();
	if (idx < 0 || !pkt)
		goto done;

	AVStream *st = ic->streams[idx];
	AVRational samples = {1, st->codecpar->sample_rate};
	t->rate = st->codecpar->sample_rate;
	t->frame_size = st->codecpar->frame_size;
	t->count = 0;

	while (av_read_frame(ic, pkt) >= 0) {
		if (pkt->stream_index == idx && t->count < FRAMES * 2) {
			t->dts[t->count++] = av_rescale_q(pkt->dts, st->time_base, samples);
			t->last_duration = av_rescale_q(pkt->duration, st->time_base, samples);
		}
		av_packet_unref(pkt);
	}
	ok = t->count > 0;

done:
	av_packet_free(&pkt);
	avformat_close_input(&ic);
	return ok;
}

static void remux_and_check(struct clip *clip, const char *ext)
{
	if (!write_mkv(clip)) {
		fprintf(stderr, "skipping: FFmpeg has no AAC encoder or Matroska muxer\n");
		os_unlink(clip->mkv);
		skip();
	}

	char out[512];
	make_path(out, sizeof(out), ext);

	/* What the MKV itself stores for the last frame: some FFmpeg versions'
	 * Matroska muxer keeps a trimmed last frame's shorter length, others
	 * write it as a full frame. The remux can only keep what is there. */
	static struct audio_track in;
	if (!read_track(clip->mkv, &in)) {
		fprintf(stderr, "could not read back the %d Hz MKV\n", clip->rate);
		os_unlink(clip->mkv);
		fail();
	}

	media_remux_job_t job = NULL;
	bool created = media_remux_job_create(&job, clip->mkv, out);
	bool processed = created && media_remux_job_process(job, NULL, NULL);
	media_remux_job_destroy(job);

	static struct audio_track t;
	bool read = processed && read_track(out, &t);
	os_unlink(clip->mkv);
	os_unlink(out);

	if (!created || !processed || !read) {
		fprintf(stderr, "remux of the %d Hz MKV to .%s failed (create %d, process %d, read %d)\n", clip->rate,
			ext, created, processed, read);
		fail();
	}

	int64_t expect_count = clip->muxed;
	int frame = t.frame_size > 0 ? t.frame_size : 1024;
	int64_t bad = 0;
	int64_t settled = 0;
	bool gap_kept = !clip->gap;
	int64_t ms = t.rate / 1000;

	for (int64_t i = 1; i < t.count; i++) {
		int64_t delta = t.dts[i] - t.dts[i - 1];
		if (clip->gap && i == GAP_AT) {
			/* The left-out audio stays left out, to the
			 * millisecond the MKV stores. */
			int64_t want = (int64_t)frame * (GAP_FRAMES + 1);
			gap_kept = delta >= want - ms && delta <= want + ms;
			if (!gap_kept)
				fprintf(stderr, "  gap before frame %lld: %lld samples, expected %lld (+-%lld)\n",
					(long long)i, (long long)delta, (long long)want, (long long)ms);
			continue;
		}
		/* After a gap the remux restarts from the MKV's millisecond
		 * timestamp, so the next few frames may move back onto the
		 * sample grid, each by less than a millisecond. */
		if (clip->gap && i > GAP_AT && delta != frame && delta >= frame - ms && delta <= frame + ms &&
		    settled < 3) {
			settled++;
			continue;
		}
		if (delta != frame) {
			if (bad < 5)
				fprintf(stderr, "  frame %lld starts %lld samples after frame %lld, expected %d\n",
					(long long)i, (long long)delta, (long long)i - 1, frame);
			bad++;
		}
	}

	/* A last frame the MKV stores as full (to the millisecond) keeps its
	 * exact length; a shorter one keeps its length to the millisecond. */
	int64_t stored = in.last_duration;
	bool stored_full = clip->last_samples == frame || stored >= frame - ms;
	bool last_ok = stored_full ? t.last_duration == frame
				   : t.last_duration >= stored - ms && t.last_duration <= stored + ms;
	if (t.count != expect_count || bad || !gap_kept || !last_ok) {
		fprintf(stderr,
			"%d Hz MKV -> .%s: %lld audio frames (expected %lld), %lld of %lld frame starts off the "
			"%d-sample grid, last frame %lld samples long (encoded %lld, stored in the MKV %lld)\n",
			clip->rate, ext, (long long)t.count, (long long)expect_count, (long long)bad,
			(long long)(t.count - 1), frame, (long long)t.last_duration, (long long)clip->last_samples,
			(long long)stored);
		fail();
	}
}

static void mkv_48k_to_mp4(void **state)
{
	(void)state;
	struct clip clip = {.rate = 48000};
	remux_and_check(&clip, "mp4");
}

static void mkv_44k_to_mp4(void **state)
{
	(void)state;
	struct clip clip = {.rate = 44100, .short_end = 400};
	remux_and_check(&clip, "mp4");
}

static void mkv_48k_to_mov(void **state)
{
	(void)state;
	struct clip clip = {.rate = 48000};
	remux_and_check(&clip, "mov");
}

static void mkv_with_gap_to_mp4(void **state)
{
	(void)state;
	struct clip clip = {.rate = 48000, .gap = true};
	remux_and_check(&clip, "mp4");
}

int main(void)
{
	const char *dir = getenv("TMPDIR");
#ifdef _WIN32
	if (!dir || !*dir)
		dir = getenv("TEMP");
	if (!dir || !*dir)
		dir = ".";
#else
	if (!dir || !*dir)
		dir = "/tmp";
#endif
	snprintf(tmp_dir, sizeof(tmp_dir), "%s", dir);
	run_id = os_gettime_ns();
	av_log_set_level(AV_LOG_WARNING);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(mkv_48k_to_mp4),
		cmocka_unit_test(mkv_44k_to_mp4),
		cmocka_unit_test(mkv_48k_to_mov),
		cmocka_unit_test(mkv_with_gap_to_mp4),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
