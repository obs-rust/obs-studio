/******************************************************************************
    Copyright (C) 2023 by Ruwen Hahn <palana@stunned.de>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "build/libobs-media.pch.h"

/* Private adapter for the Rust port of media-io/media-remux.c.h
 * (obs-media-io's media_remux), not a public API. Each function is one
 * libavformat call of the original, or one field it reads or writes, so the
 * FFmpeg struct layouts stay in C and the Rust job decides the calls. The
 * contexts are the original's AVFormatContext pointers. */

struct obs_media_remux_stream {
	int32_t codec_type;
	int32_t codec_id;
	int32_t sample_rate;
	int32_t frame_size;
	int32_t channels;
	AVRational time_base;
};

struct obs_media_remux_packet {
	int64_t pts;
	int64_t dts;
	int64_t duration;
	int64_t pos;
	int32_t size;
	int32_t stream_index;
};

int obs_media_remux_open_input(AVFormatContext **input, const char *path);
int obs_media_remux_find_stream_info(AVFormatContext *input);
void obs_media_remux_dump_format(AVFormatContext *ctx, const char *path, bool is_output);
AVFormatContext *obs_media_remux_alloc_output(const char *path);
uint32_t obs_media_remux_nb_streams(const AVFormatContext *ctx);
void obs_media_remux_stream(const AVFormatContext *ctx, uint32_t index, struct obs_media_remux_stream *out);
bool obs_media_remux_new_stream(AVFormatContext *output);
int obs_media_remux_copy_parameters(const AVFormatContext *input, AVFormatContext *output, uint32_t index);
void obs_media_remux_copy_metadata(const AVFormatContext *input, AVFormatContext *output, uint32_t index);
bool obs_media_remux_has_codec_tags(const AVFormatContext *output);
int32_t obs_media_remux_codec_id_for_tag(const AVFormatContext *output, uint32_t tag);
void obs_media_remux_set_codec_tag(AVFormatContext *output, uint32_t index, uint32_t tag);
void obs_media_remux_default_channel_layout(AVFormatContext *output, uint32_t index, int channels);
void obs_media_remux_set_layout_4point1(AVFormatContext *output, uint32_t index);
int obs_media_remux_output_flags(const AVFormatContext *output);
int obs_media_remux_avio_open(AVFormatContext *output, const char *path);
int obs_media_remux_write_header(AVFormatContext *output);
AVPacket *obs_media_remux_packet_new(void);
void obs_media_remux_packet_free(AVPacket *pkt);
void obs_media_remux_packet_unref(AVPacket *pkt);
int obs_media_remux_read_frame(AVFormatContext *input, AVPacket *pkt, struct obs_media_remux_packet *out);
int obs_media_remux_write_frame(AVFormatContext *output, AVPacket *pkt, const struct obs_media_remux_packet *in);
int obs_media_remux_write_trailer(AVFormatContext *output);
void obs_media_remux_close_input(AVFormatContext *input);
void obs_media_remux_avio_close(AVFormatContext *output);
void obs_media_remux_free_context(AVFormatContext *output);
int obs_media_remux_audio_frame_duration(const AVFormatContext *input, uint32_t index, int size);
int64_t obs_media_remux_rescale_q_rnd(int64_t a, AVRational bq, AVRational cq, int rnd);
int64_t obs_media_remux_rescale_q(int64_t a, AVRational bq, AVRational cq);
int64_t obs_media_remux_rescale_delta(AVRational in_tb, int64_t in_ts, AVRational fs_tb, int duration, int64_t *last,
				      AVRational out_tb);
void obs_media_remux_error_string(int err, char *buf, size_t size);
void obs_media_remux_log(int level, const char *line);

int obs_media_remux_open_input(AVFormatContext **input, const char *path)
{
	return avformat_open_input(input, path, NULL, NULL);
}

int obs_media_remux_find_stream_info(AVFormatContext *input)
{
	return avformat_find_stream_info(input, NULL);
}

void obs_media_remux_dump_format(AVFormatContext *ctx, const char *path, bool is_output)
{
#ifndef NDEBUG
	av_dump_format(ctx, 0, path, is_output);
#else
	UNUSED_PARAMETER(ctx);
	UNUSED_PARAMETER(path);
	UNUSED_PARAMETER(is_output);
#endif
}

AVFormatContext *obs_media_remux_alloc_output(const char *path)
{
	AVFormatContext *output = NULL;
	avformat_alloc_output_context2(&output, NULL, NULL, path);
	return output;
}

uint32_t obs_media_remux_nb_streams(const AVFormatContext *ctx)
{
	return ctx ? ctx->nb_streams : 0;
}

void obs_media_remux_stream(const AVFormatContext *ctx, uint32_t index, struct obs_media_remux_stream *out)
{
	const AVStream *stream = ctx->streams[index];
	const AVCodecParameters *par = stream->codecpar;

	out->codec_type = (int32_t)par->codec_type;
	out->codec_id = (int32_t)par->codec_id;
	out->sample_rate = par->sample_rate;
	out->frame_size = par->frame_size;
	out->channels = par->ch_layout.nb_channels;
	out->time_base = stream->time_base;
}

bool obs_media_remux_new_stream(AVFormatContext *output)
{
	return avformat_new_stream(output, NULL) != NULL;
}

int obs_media_remux_copy_parameters(const AVFormatContext *input, AVFormatContext *output, uint32_t index)
{
	return avcodec_parameters_copy(output->streams[index]->codecpar, input->streams[index]->codecpar);
}

void obs_media_remux_copy_metadata(const AVFormatContext *input, AVFormatContext *output, uint32_t index)
{
	av_dict_copy(&output->streams[index]->metadata, input->streams[index]->metadata, 0);
}

bool obs_media_remux_has_codec_tags(const AVFormatContext *output)
{
	return output->oformat->codec_tag != NULL;
}

int32_t obs_media_remux_codec_id_for_tag(const AVFormatContext *output, uint32_t tag)
{
	return (int32_t)av_codec_get_id(output->oformat->codec_tag, tag);
}

void obs_media_remux_set_codec_tag(AVFormatContext *output, uint32_t index, uint32_t tag)
{
	output->streams[index]->codecpar->codec_tag = tag;
}

void obs_media_remux_default_channel_layout(AVFormatContext *output, uint32_t index, int channels)
{
	av_channel_layout_default(&output->streams[index]->codecpar->ch_layout, channels);
}

void obs_media_remux_set_layout_4point1(AVFormatContext *output, uint32_t index)
{
	output->streams[index]->codecpar->ch_layout = (AVChannelLayout)AV_CHANNEL_LAYOUT_4POINT1;
}

int obs_media_remux_output_flags(const AVFormatContext *output)
{
	return output->oformat->flags;
}

int obs_media_remux_avio_open(AVFormatContext *output, const char *path)
{
	return avio_open(&output->pb, path, AVIO_FLAG_WRITE);
}

int obs_media_remux_write_header(AVFormatContext *output)
{
	return avformat_write_header(output, NULL);
}

/* The original reads into an AVPacket on its stack; the Rust job keeps one
 * from obs_media_remux_packet_new for the whole remux. */
AVPacket *obs_media_remux_packet_new(void)
{
	return bzalloc(sizeof(AVPacket));
}

void obs_media_remux_packet_free(AVPacket *pkt)
{
	bfree(pkt);
}

void obs_media_remux_packet_unref(AVPacket *pkt)
{
	av_packet_unref(pkt);
}

int obs_media_remux_read_frame(AVFormatContext *input, AVPacket *pkt, struct obs_media_remux_packet *out)
{
	int ret = av_read_frame(input, pkt);
	if (ret < 0)
		return ret;

	out->pts = pkt->pts;
	out->dts = pkt->dts;
	out->duration = pkt->duration;
	out->pos = pkt->pos;
	out->size = pkt->size;
	out->stream_index = pkt->stream_index;
	return ret;
}

int obs_media_remux_write_frame(AVFormatContext *output, AVPacket *pkt, const struct obs_media_remux_packet *in)
{
	pkt->pts = in->pts;
	pkt->dts = in->dts;
	pkt->duration = in->duration;
	pkt->pos = in->pos;

	int ret = av_interleaved_write_frame(output, pkt);
	av_packet_unref(pkt);
	return ret;
}

int obs_media_remux_write_trailer(AVFormatContext *output)
{
	return av_write_trailer(output);
}

void obs_media_remux_close_input(AVFormatContext *input)
{
	avformat_close_input(&input);
}

void obs_media_remux_avio_close(AVFormatContext *output)
{
	avio_close(output->pb);
}

void obs_media_remux_free_context(AVFormatContext *output)
{
	avformat_free_context(output);
}

int obs_media_remux_audio_frame_duration(const AVFormatContext *input, uint32_t index, int size)
{
	return av_get_audio_frame_duration2(input->streams[index]->codecpar, size);
}

int64_t obs_media_remux_rescale_q_rnd(int64_t a, AVRational bq, AVRational cq, int rnd)
{
	return av_rescale_q_rnd(a, bq, cq, (enum AVRounding)rnd);
}

int64_t obs_media_remux_rescale_q(int64_t a, AVRational bq, AVRational cq)
{
	return av_rescale_q(a, bq, cq);
}

int64_t obs_media_remux_rescale_delta(AVRational in_tb, int64_t in_ts, AVRational fs_tb, int duration, int64_t *last,
				      AVRational out_tb)
{
	return av_rescale_delta(in_tb, in_ts, fs_tb, duration, last, out_tb);
}

void obs_media_remux_error_string(int err, char *buf, size_t size)
{
	av_strerror(err, buf, size);
}

void obs_media_remux_log(int level, const char *line)
{
	blog(level, "%s", line);
}
