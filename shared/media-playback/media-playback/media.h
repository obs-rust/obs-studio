/*
 * Copyright (c) 2023 Lain Bailey <lain@obsproject.com>
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */

#pragma once

#include <obs.h>
#include "decode.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ffmpeg headers, with MSVC warnings 4244/4204 silenced around them only. */
#include "platform-media-ffmpeg.h"

struct mp_media {
	AVFormatContext *fmt;

	mp_video_cb v_preload_cb;
	mp_video_cb v_seek_cb;
	mp_stop_cb stop_cb;
	mp_video_cb v_cb;
	mp_audio_cb a_cb;
	void *opaque;

	char *path;
	char *format_name;
	char *ffmpeg_options;
	int buffering;
	int speed;

	enum AVPixelFormat scale_format;
	struct SwsContext *swscale;
	int scale_linesizes[4];
	uint8_t *scale_pic[4];
	enum AVPixelFormat scale_src_format;
	int scale_src_width;
	int scale_src_height;
	int scale_space;
	int scale_range;

	DARRAY(AVPacket *) packet_pool;
	struct mp_decode v;
	struct mp_decode a;
	bool request_preload;
	bool is_local_file;
	bool reconnecting;
	bool has_video;
	bool has_audio;
	bool is_file;
	bool eof;
	bool hw;

	struct obs_source_frame obsframe;
	enum video_colorspace cur_space;
	enum video_range_type cur_range;
	enum video_range_type force_range;
	bool is_linear_alpha;

	int64_t play_sys_ts;
	int64_t next_pts_ns;
	uint64_t next_ns;
	int64_t start_ts;
	int64_t base_ts;
	int64_t v_output_pts;
	int64_t v_output_interval;
	bool last_frame_held;
	bool full_decode;
	bool measure_duration;
	int64_t measured_duration;
	/* the first timestamp of the file (AVFormatContext::start_time, in
	 * microseconds), 0 if unknown: the media time and seeks count from it */
	int64_t start_time;
	int64_t measured_frames;

	uint64_t interrupt_poll_ts;

	pthread_mutex_t mutex;
	os_sem_t *sem;
	bool preload_frame;
	bool stopping;
	bool looping;
	bool active;
	bool reset;
	bool kill;

	bool thread_valid;
	pthread_t thread;

	bool pause;
	bool reset_ts;
	bool seek;
	bool seek_next_ts;
	int64_t seek_pos;

	/* after a seek, frames that end at or before this time (in decoded
	 * pts, nanoseconds) are dropped, so playback starts at the seek
	 * position, not at the keyframe before it */
	bool seek_exact;
	int64_t seek_target_ns;
};

typedef struct mp_media mp_media_t;

extern bool mp_media_init(mp_media_t *media, const struct mp_media_info *info);
extern void mp_media_free(mp_media_t *media);

extern void mp_media_play(mp_media_t *media, bool loop, bool reconnecting);
extern void mp_media_stop(mp_media_t *media);
extern void mp_media_play_pause(mp_media_t *media, bool pause);
extern void mp_media_preload_frame(mp_media_t *media);
extern int64_t mp_media_get_current_time(mp_media_t *m);
extern int64_t mp_media_get_frames(mp_media_t *m);
extern int64_t mp_media_get_duration(mp_media_t *m);
extern void mp_media_seek(mp_media_t *m, int64_t pos);

/* #define DETAILED_DEBUG_INFO */

#ifdef __cplusplus
}
#endif
