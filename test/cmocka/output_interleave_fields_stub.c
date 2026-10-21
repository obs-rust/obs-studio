/* The output, caption track and encoder accessors of obs-output.c and
 * obs-encoder.c for tests that compile obs-output-interleave.c without them.
 * They point at the test's struct obs_output and read the test's struct
 * obs_encoder, as libobs does for a real output and encoder. */

#include <obs-internal.h>

struct obs_output_interleave_fields obs_output_interleave_fields(const obs_output_t *output)
{
	struct obs_output *o = (struct obs_output *)(uintptr_t)output;
	struct obs_output_interleave_fields f = {
		.interleaved_mutex = &o->interleaved_mutex,
		.interleaved_packets = &o->interleaved_packets,
		.received_video = o->received_video,
		.received_audio = &o->received_audio,
		.video_offsets = o->video_offsets,
		.audio_offsets = o->audio_offsets,
		.highest_audio_ts = &o->highest_audio_ts,
		.highest_video_ts = o->highest_video_ts,
		.video_encoders = o->video_encoders,
		.audio_encoders = o->audio_encoders,
		.encoder_packet_times = o->encoder_packet_times,
		.keyframe_group_tracking = &o->keyframe_group_tracking,
		.pkt_callbacks_mutex = &o->pkt_callbacks_mutex,
		.pkt_callbacks = &o->pkt_callbacks,
		.total_frames = &o->total_frames,
		.interleaver_max_batch_size = &o->interleaver_max_batch_size,
		.active_delay_ns = &o->active_delay_ns,
		.encoded_packet = &o->info.encoded_packet,
		.data = &o->context.data,
		.name = &o->context.name,
	};

	return f;
}

struct obs_output_caption_track_fields obs_output_caption_track_fields(obs_output_t *output, size_t track_idx)
{
	struct caption_track_data *ctrack = output->caption_tracks[track_idx];
	struct obs_output_caption_track_fields f = {
		.caption_mutex = &ctrack->caption_mutex,
		.caption_head = &ctrack->caption_head,
		.caption_timestamp = &ctrack->caption_timestamp,
		.last_caption_timestamp = &ctrack->last_caption_timestamp,
		.caption_data_size = &ctrack->caption_data.size,
	};

	return f;
}

struct obs_encoder_interleave_info obs_encoder_interleave_info(const obs_encoder_t *encoder)
{
	struct obs_encoder_interleave_info info = {
		.timebase_num = encoder->timebase_num,
		.timebase_den = encoder->timebase_den,
		.framesize = encoder->framesize,
		.group = encoder->encoder_group,
	};

	return info;
}

uint32_t obs_encoder_group_num_started(struct obs_encoder_group *group)
{
	uint32_t num_started;

	pthread_mutex_lock(&group->mutex);
	num_started = group->num_encoders_started;
	pthread_mutex_unlock(&group->mutex);

	return num_started;
}
