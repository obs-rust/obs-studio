/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

#include "libobs/build/libobs.pch.h"

/* The encoded packet interleaver (moved verbatim from obs-output.c.h). This file
 * comes first in its unity group; these are defined in obs-output.c.h. */
static inline bool active(const struct obs_output *output);
static bool get_first_audio_encoder_index(const struct obs_output *output, size_t *index);
static bool get_first_video_encoder_index(const struct obs_output *output, size_t *index);
static bool add_caption(struct obs_output *output, struct encoder_packet *out);

static inline void free_packets(struct obs_output *output)
{
	for (size_t i = 0; i < output->interleaved_packets.num; i++)
		obs_encoder_packet_release(output->interleaved_packets.array + i);
	da_free(output->interleaved_packets);
}

static size_t get_encoder_index(const struct obs_output *output, struct encoder_packet *pkt)
{
	if (pkt->type == OBS_ENCODER_VIDEO) {
		for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
			struct obs_encoder *encoder = output->video_encoders[i];

			if (encoder && pkt->encoder == encoder)
				return i;
		}
	} else if (pkt->type == OBS_ENCODER_AUDIO) {
		for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
			struct obs_encoder *encoder = output->audio_encoders[i];

			if (encoder && pkt->encoder == encoder)
				return i;
		}
	}

	assert(false);
	return 0;
}

static inline void check_received(struct obs_output *output, struct encoder_packet *out)
{
	if (out->type == OBS_ENCODER_VIDEO) {
		if (!output->received_video[out->track_idx])
			output->received_video[out->track_idx] = true;
	} else {
		if (!output->received_audio)
			output->received_audio = true;
	}
}

static inline void apply_interleaved_packet_offset(struct obs_output *output, struct encoder_packet *out,
						   struct encoder_packet_time *packet_time)
{
	int64_t offset;

	/* audio and video need to start at timestamp 0, and the encoders
	 * may not currently be at 0 when we get data.  so, we store the
	 * current dts as offset and subtract that value from the dts/pts
	 * of the output packet. */
	offset = (out->type == OBS_ENCODER_VIDEO) ? output->video_offsets[out->track_idx]
						  : output->audio_offsets[out->track_idx];

	out->dts -= offset;
	out->pts -= offset;
	if (packet_time)
		packet_time->pts -= offset;

	/* convert the newly adjusted dts to relative dts time to ensure proper
	 * interleaving.  if we're using an audio encoder that's already been
	 * started on another output, then the first audio packet may not be
	 * quite perfectly synced up in terms of system time (and there's
	 * nothing we can really do about that), but it will always at least be
	 * within a 23ish millisecond threshold (at least for AAC) */
	out->dts_usec = packet_dts_usec(out);
}

static inline bool has_higher_opposing_ts(struct obs_output *output, struct encoder_packet *packet)
{
	bool has_higher = true;

	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (!output->video_encoders[i] || (packet->type == OBS_ENCODER_VIDEO && i == packet->track_idx))
			continue;
		has_higher = has_higher && output->highest_video_ts[i] > packet->dts_usec;
	}

	return packet->type == OBS_ENCODER_AUDIO ? has_higher
						 : (has_higher && output->highest_audio_ts > packet->dts_usec);
}

static inline void send_interleaved(struct obs_output *output)
{
	struct encoder_packet out = output->interleaved_packets.array[0];
	struct encoder_packet_time ept_local = {0};
	bool found_ept = false;

	da_erase(output->interleaved_packets, 0);

	if (out.type == OBS_ENCODER_VIDEO) {
		output->total_frames++;

		pthread_mutex_lock(&output->caption_tracks[out.track_idx]->caption_mutex);

		double frame_timestamp = (out.pts * out.timebase_num) / (double)out.timebase_den;

		struct caption_track_data *ctrack = output->caption_tracks[out.track_idx];

		if (ctrack->caption_head && ctrack->caption_timestamp <= frame_timestamp) {
			blog(LOG_DEBUG, "Sending caption: %f \"%s\"", frame_timestamp, &ctrack->caption_head->text[0]);

			double display_duration = ctrack->caption_head->display_duration;

			if (add_caption(output, &out)) {
				ctrack->caption_timestamp = frame_timestamp + display_duration;
			}
		}

		if (ctrack->caption_data.size > 0) {
			if (ctrack->last_caption_timestamp < frame_timestamp) {
				ctrack->last_caption_timestamp = frame_timestamp;
				add_caption(output, &out);
			}
		}
		pthread_mutex_unlock(&ctrack->caption_mutex);

		/* Iterate the array of encoder packet times to
		 * find a matching PTS entry, and drain the array.
		 * Packet timing currently applies to video only.
		 */
		struct encoder_packet_time *ept = NULL;
		size_t num_ept = output->encoder_packet_times[out.track_idx].num;
		if (num_ept) {
			for (size_t i = 0; i < num_ept; i++) {
				ept = &output->encoder_packet_times[out.track_idx].array[i];
				if (ept->pts == out.pts) {
					ept_local = *ept;
					da_erase(output->encoder_packet_times[out.track_idx], i);
					found_ept = true;
					break;
				}
			}
			if (found_ept == false) {
				blog(LOG_DEBUG, "%s: Track %lu encoder packet timing for PTS%" PRId64 " not found.",
				     __FUNCTION__, out.track_idx, out.pts);
			}
		} else {
			// encoder_packet_times should not be empty; log if so.
			blog(LOG_DEBUG, "%s: Track %lu encoder packet timing array empty.", __FUNCTION__,
			     out.track_idx);
		}
	}

	/* Iterate the registered packet callback(s) and invoke
	 * each one. The caption track logic further above should
	 * eventually migrate to the packet callback mechanism.
	 */
	pthread_mutex_lock(&output->pkt_callbacks_mutex);
	for (size_t i = 0; i < output->pkt_callbacks.num; ++i) {
		struct packet_callback *const callback = &output->pkt_callbacks.array[i];
		// Packet interleave request timestamp
		ept_local.pir = os_gettime_ns();
		callback->packet_cb(output, &out, found_ept ? &ept_local : NULL, callback->param);
	}
	pthread_mutex_unlock(&output->pkt_callbacks_mutex);

	output->info.encoded_packet(output->context.data, &out);
	obs_encoder_packet_release(&out);
}

static inline void set_higher_ts(struct obs_output *output, struct encoder_packet *packet)
{
	if (packet->type == OBS_ENCODER_VIDEO) {
		if (output->highest_video_ts[packet->track_idx] < packet->dts_usec)
			output->highest_video_ts[packet->track_idx] = packet->dts_usec;
	} else {
		if (output->highest_audio_ts < packet->dts_usec)
			output->highest_audio_ts = packet->dts_usec;
	}
}

static inline struct encoder_packet *find_first_packet_type(struct obs_output *output, enum obs_encoder_type type,
							    size_t audio_idx);
static int find_first_packet_type_idx(struct obs_output *output, enum obs_encoder_type type, size_t audio_idx);

/* gets the point where audio and video are closest together */
static size_t get_interleaved_start_idx(struct obs_output *output)
{
	int64_t closest_diff = 0x7FFFFFFFFFFFFFFFLL;
	struct encoder_packet *first_video = find_first_packet_type(output, OBS_ENCODER_VIDEO, 0);
	size_t video_idx = DARRAY_INVALID;
	size_t idx = 0;

	for (size_t i = 0; i < output->interleaved_packets.num; i++) {
		struct encoder_packet *packet = &output->interleaved_packets.array[i];
		int64_t diff;

		if (packet->type != OBS_ENCODER_AUDIO) {
			if (packet == first_video)
				video_idx = i;
			continue;
		}

		diff = llabs(packet->dts_usec - first_video->dts_usec);
		if (diff < closest_diff) {
			closest_diff = diff;
			idx = i;
		}
	}

	idx = video_idx < idx ? video_idx : idx;

	/* Early AAC/Opus audio packets will be for "priming" the encoder and contain silence, but they should not be
	 * discarded. Set the idx to the first audio packet if closest PTS was <= 0. */
	size_t first_audio_idx = idx;
	while (output->interleaved_packets.array[first_audio_idx].type != OBS_ENCODER_AUDIO)
		first_audio_idx++;

	if (output->interleaved_packets.array[first_audio_idx].pts <= 0) {
		for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
			int audio_idx = find_first_packet_type_idx(output, OBS_ENCODER_AUDIO, i);
			if (audio_idx >= 0 && (size_t)audio_idx < idx)
				idx = audio_idx;
		}
	}

	return idx;
}

static int64_t get_encoder_duration(struct obs_encoder *encoder)
{
	return (encoder->timebase_num * 1000000LL / encoder->timebase_den) * encoder->framesize;
}

static int prune_premature_packets(struct obs_output *output)
{
	struct encoder_packet *video;
	int video_idx;
	int max_idx;
	int64_t duration_usec, max_audio_duration_usec = 0;
	int64_t max_diff = 0;
	int64_t diff = 0;
	int audio_encoders = 0;

	video_idx = find_first_packet_type_idx(output, OBS_ENCODER_VIDEO, 0);
	if (video_idx == -1)
		return -1;

	max_idx = video_idx;
	video = &output->interleaved_packets.array[video_idx];
	duration_usec = video->timebase_num * 1000000LL / video->timebase_den;

	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		struct encoder_packet *audio;
		int audio_idx;
		int64_t audio_duration_usec = 0;

		if (!output->audio_encoders[i])
			continue;
		audio_encoders++;

		audio_idx = find_first_packet_type_idx(output, OBS_ENCODER_AUDIO, i);
		if (audio_idx == -1) {
			output->received_audio = false;
			return -1;
		}

		audio = &output->interleaved_packets.array[audio_idx];
		if (audio_idx > max_idx)
			max_idx = audio_idx;

		diff = audio->dts_usec - video->dts_usec;
		if (diff > max_diff)
			max_diff = diff;

		audio_duration_usec = get_encoder_duration(output->audio_encoders[i]);
		if (audio_duration_usec > max_audio_duration_usec)
			max_audio_duration_usec = audio_duration_usec;
	}

	/* Once multiple audio encoders are running they are almost always out
	 * of phase by ~Xms. If users change their video to > 100fps then it
	 * becomes probable that this phase difference will be larger than the
	 * video duration preventing us from ever finding a synchronization
	 * point due to their larger frame duration. Instead give up on a tight
	 * video sync. */
	if (audio_encoders > 1 && duration_usec < max_audio_duration_usec) {
		duration_usec = max_audio_duration_usec;
	}

	return diff > duration_usec ? max_idx + 1 : 0;
}

#define DEBUG_STARTING_PACKETS 0

static void discard_to_idx(struct obs_output *output, size_t idx)
{
	for (size_t i = 0; i < idx; i++) {
		struct encoder_packet *packet = &output->interleaved_packets.array[i];
#if DEBUG_STARTING_PACKETS == 1
		blog(LOG_DEBUG, "discarding %s packet, dts: %lld, pts: %lld",
		     packet->type == OBS_ENCODER_VIDEO ? "video" : "audio", packet->dts, packet->pts);
#endif
		if (packet->type == OBS_ENCODER_VIDEO) {
			da_pop_front(output->encoder_packet_times[packet->track_idx]);
		}
		obs_encoder_packet_release(packet);
	}

	da_erase_range(output->interleaved_packets, 0, idx);
}

static bool prune_interleaved_packets(struct obs_output *output)
{
	size_t start_idx = 0;
	int prune_start = prune_premature_packets(output);

#if DEBUG_STARTING_PACKETS == 1
	blog(LOG_DEBUG, "--------- Pruning! %d ---------", prune_start);
	for (size_t i = 0; i < output->interleaved_packets.num; i++) {
		struct encoder_packet *packet = &output->interleaved_packets.array[i];
		blog(LOG_DEBUG, "packet: %s %d, ts: %lld, pruned = %s",
		     packet->type == OBS_ENCODER_AUDIO ? "audio" : "video", (int)packet->track_idx, packet->dts_usec,
		     (int)i < prune_start ? "true" : "false");
	}
#endif

	/* prunes the first video packet if it's too far away from audio */
	if (prune_start == -1)
		return false;
	else if (prune_start != 0)
		start_idx = (size_t)prune_start;
	else
		start_idx = get_interleaved_start_idx(output);

	if (start_idx)
		discard_to_idx(output, start_idx);

	return true;
}

static int find_first_packet_type_idx(struct obs_output *output, enum obs_encoder_type type, size_t idx)
{
	for (size_t i = 0; i < output->interleaved_packets.num; i++) {
		struct encoder_packet *packet = &output->interleaved_packets.array[i];

		if (packet->type == type && packet->track_idx == idx)
			return (int)i;
	}

	return -1;
}

static int find_last_packet_type_idx(struct obs_output *output, enum obs_encoder_type type, size_t idx)
{
	for (size_t i = output->interleaved_packets.num; i > 0; i--) {
		struct encoder_packet *packet = &output->interleaved_packets.array[i - 1];

		if (packet->type == type && packet->track_idx == idx)
			return (int)(i - 1);
	}

	return -1;
}

static inline struct encoder_packet *find_first_packet_type(struct obs_output *output, enum obs_encoder_type type,
							    size_t audio_idx)
{
	int idx = find_first_packet_type_idx(output, type, audio_idx);
	return (idx != -1) ? &output->interleaved_packets.array[idx] : NULL;
}

static inline struct encoder_packet *find_last_packet_type(struct obs_output *output, enum obs_encoder_type type,
							   size_t audio_idx)
{
	int idx = find_last_packet_type_idx(output, type, audio_idx);
	return (idx != -1) ? &output->interleaved_packets.array[idx] : NULL;
}

static bool get_audio_and_video_packets(struct obs_output *output, struct encoder_packet **video,
					struct encoder_packet **audio)
{
	bool found_video = false;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i]) {
			video[i] = find_first_packet_type(output, OBS_ENCODER_VIDEO, i);
			if (!video[i]) {
				output->received_video[i] = false;
				return false;
			} else {
				found_video = true;
			}
		}
	}

	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		if (output->audio_encoders[i]) {
			audio[i] = find_first_packet_type(output, OBS_ENCODER_AUDIO, i);
			if (!audio[i]) {
				output->received_audio = false;
				return false;
			}
		}
	}

	return found_video;
}

static bool initialize_interleaved_packets(struct obs_output *output)
{
	struct encoder_packet *video[MAX_OUTPUT_VIDEO_ENCODERS] = {0};
	struct encoder_packet *audio[MAX_OUTPUT_AUDIO_ENCODERS] = {0};
	struct encoder_packet *last_audio[MAX_OUTPUT_AUDIO_ENCODERS] = {0};
	size_t start_idx;
	size_t first_audio_idx;
	size_t first_video_idx;

	if (!get_first_audio_encoder_index(output, &first_audio_idx))
		return false;
	if (!get_first_video_encoder_index(output, &first_video_idx))
		return false;

	if (!get_audio_and_video_packets(output, video, audio))
		return false;

	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		if (output->audio_encoders[i]) {
			last_audio[i] = find_last_packet_type(output, OBS_ENCODER_AUDIO, i);
		}
	}

	/* ensure that there is audio past the first video packet */
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		if (output->audio_encoders[i]) {
			if (last_audio[i]->dts_usec < video[first_video_idx]->dts_usec) {
				output->received_audio = false;
				return false;
			}
		}
	}

	/* clear out excess starting audio if it hasn't been already */
	start_idx = get_interleaved_start_idx(output);
	if (start_idx) {
		discard_to_idx(output, start_idx);
		if (!get_audio_and_video_packets(output, video, audio))
			return false;
	}

	/* get new offsets */
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i]) {
			output->video_offsets[i] = video[i]->pts;
		}
	}
	for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
		if (output->audio_encoders[i] && audio[i]->dts > 0) {
			output->audio_offsets[i] = audio[i]->dts;
		}
	}
#if DEBUG_STARTING_PACKETS == 1
	int64_t v = video[first_video_idx]->dts_usec;
	int64_t a = audio[first_audio_idx]->dts_usec;
	int64_t diff = v - a;

	blog(LOG_DEBUG,
	     "output '%s' offset for video: %lld, audio: %lld, "
	     "diff: %lldms",
	     output->context.name, v, a, diff / 1000LL);
#endif

	/* subtract offsets from highest TS offset variables */
	output->highest_audio_ts -= audio[first_audio_idx]->dts_usec;

	/* apply new offsets to all existing packet DTS/PTS values */
	for (size_t i = 0; i < output->interleaved_packets.num; i++) {
		struct encoder_packet *packet = &output->interleaved_packets.array[i];
		apply_interleaved_packet_offset(output, packet, NULL);
	}

	return true;
}

static inline void insert_interleaved_packet(struct obs_output *output, struct encoder_packet *out)
{
	size_t idx;
	for (idx = 0; idx < output->interleaved_packets.num; idx++) {
		struct encoder_packet *cur_packet;
		cur_packet = output->interleaved_packets.array + idx;

		// sort video packets with same DTS by track index,
		// to prevent the pruning logic from removing additional
		// video tracks
		if (out->dts_usec == cur_packet->dts_usec && out->type == OBS_ENCODER_VIDEO &&
		    cur_packet->type == OBS_ENCODER_VIDEO && out->track_idx > cur_packet->track_idx)
			continue;

		if (out->dts_usec == cur_packet->dts_usec && out->type == OBS_ENCODER_VIDEO) {
			break;
		} else if (out->dts_usec < cur_packet->dts_usec) {
			break;
		}
	}

	da_insert(output->interleaved_packets, idx, out);
}

static void resort_interleaved_packets(struct obs_output *output)
{
	DARRAY(struct encoder_packet) old_array;

	old_array.da = output->interleaved_packets.da;
	memset(&output->interleaved_packets, 0, sizeof(output->interleaved_packets));

	for (size_t i = 0; i < old_array.num; i++) {
		set_higher_ts(output, &old_array.array[i]);

		insert_interleaved_packet(output, &old_array.array[i]);
	}

	da_free(old_array);
}

static void discard_unused_audio_packets(struct obs_output *output, int64_t dts_usec)
{
	size_t idx = 0;

	for (; idx < output->interleaved_packets.num; idx++) {
		struct encoder_packet *p = &output->interleaved_packets.array[idx];

		if (p->dts_usec >= dts_usec)
			break;
	}

	if (idx)
		discard_to_idx(output, idx);
}

static bool purge_encoder_group_keyframe_data(obs_output_t *output, size_t idx)
{
	struct keyframe_group_data *data = &output->keyframe_group_tracking.array[idx];
	uint32_t modified_count = 0;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (data->seen_on_track[i] != KEYFRAME_TRACK_STATUS_NOT_SEEN)
			modified_count += 1;
	}

	if (modified_count == data->required_tracks) {
		da_erase(output->keyframe_group_tracking, idx);
		return true;
	}
	return false;
}

/* Check whether keyframes are emitted from all grouped encoders, and log
 * if keyframes haven't been emitted from all grouped encoders. */
static void check_encoder_group_keyframe_alignment(obs_output_t *output, struct encoder_packet *packet)
{
	size_t idx = 0;
	struct keyframe_group_data insert_data = {0};

	if (!packet->keyframe || packet->type != OBS_ENCODER_VIDEO || !packet->encoder->encoder_group)
		return;

	for (; idx < output->keyframe_group_tracking.num;) {
		struct keyframe_group_data *data = &output->keyframe_group_tracking.array[idx];
		if (data->pts > packet->pts)
			break;
		if (data->group_id != (uintptr_t)packet->encoder->encoder_group) {
			idx += 1;
			continue;
		}

		if (data->pts < packet->pts) {
			if (data->seen_on_track[packet->track_idx] == KEYFRAME_TRACK_STATUS_NOT_SEEN) {
				blog(LOG_WARNING,
				     "obs-output '%s': Missing keyframe with pts %" PRIi64
				     " for encoder '%s' (track: %zu)",
				     obs_output_get_name(output), data->pts, obs_encoder_get_name(packet->encoder),
				     packet->track_idx);
			}

			data->seen_on_track[packet->track_idx] = KEYFRAME_TRACK_STATUS_SKIPPED;

			if (!purge_encoder_group_keyframe_data(output, idx))
				idx += 1;
			continue;
		}

		data->seen_on_track[packet->track_idx] = KEYFRAME_TRACK_STATUS_SEEN;
		purge_encoder_group_keyframe_data(output, idx);
		return;
	}

	insert_data.group_id = (uintptr_t)packet->encoder->encoder_group;
	insert_data.pts = packet->pts;
	insert_data.seen_on_track[packet->track_idx] = KEYFRAME_TRACK_STATUS_SEEN;

	pthread_mutex_lock(&packet->encoder->encoder_group->mutex);
	insert_data.required_tracks = packet->encoder->encoder_group->num_encoders_started;
	pthread_mutex_unlock(&packet->encoder->encoder_group->mutex);

	da_insert(output->keyframe_group_tracking, idx, &insert_data);
}

static void apply_ept_offsets(struct obs_output *output)
{
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		for (size_t j = 0; j < output->encoder_packet_times[i].num; j++) {
			output->encoder_packet_times[i].array[j].pts -= output->video_offsets[i];
		}
	}
}

static inline size_t count_streamable_frames(struct obs_output *output)
{
	size_t eligible = 0;

	for (size_t idx = 0; idx < output->interleaved_packets.num; idx++) {
		struct encoder_packet *pkt = &output->interleaved_packets.array[idx];

		/* Only count an interleaved packet as streamable if there are packets of the opposing type and of a
		 * higher timestamp in the interleave buffer. This ensures that the timestamps are monotonic. */
		if (!has_higher_opposing_ts(output, pkt))
			break;

		eligible++;
	}

	return eligible;
}

static void interleave_packets(void *data, struct encoder_packet *packet, struct encoder_packet_time *packet_time)
{
	struct obs_output *output = data;
	struct encoder_packet out;
	bool was_started;
	bool received_video;
	struct encoder_packet_time *output_packet_time = NULL;

	if (!active(output))
		return;

	packet->track_idx = get_encoder_index(output, packet);

	pthread_mutex_lock(&output->interleaved_mutex);

	/* if first video frame is not a keyframe, discard until received */
	if (packet->type == OBS_ENCODER_VIDEO && !output->received_video[packet->track_idx] && !packet->keyframe) {
		discard_unused_audio_packets(output, packet->dts_usec);
		pthread_mutex_unlock(&output->interleaved_mutex);

		if (output->active_delay_ns)
			obs_encoder_packet_release(packet);
		return;
	}

	received_video = true;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i])
			received_video = received_video && output->received_video[i];
	}

	check_encoder_group_keyframe_alignment(output, packet);

	was_started = output->received_audio && received_video;

	if (output->active_delay_ns)
		out = *packet;
	else
		obs_encoder_packet_create_instance(&out, packet);

	if (packet_time) {
		output_packet_time = da_push_back_new(output->encoder_packet_times[packet->track_idx]);
		*output_packet_time = *packet_time;
	}

	if (was_started)
		apply_interleaved_packet_offset(output, &out, output_packet_time);
	else
		check_received(output, packet);

	insert_interleaved_packet(output, &out);

	received_video = true;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i])
			received_video = received_video && output->received_video[i];
	}

	/* when both video and audio have been received, we're ready
	 * to start sending out packets (one at a time) */
	if (output->received_audio && received_video) {
		if (!was_started) {
			if (prune_interleaved_packets(output)) {
				if (initialize_interleaved_packets(output)) {
					resort_interleaved_packets(output);
					apply_ept_offsets(output);
					send_interleaved(output);
				}
			}
		} else {
			set_higher_ts(output, &out);

			size_t streamable = count_streamable_frames(output);
			if (streamable) {
				send_interleaved(output);

				/* If we have more eligible packets queued than we normally should have,
				 * send one additional packet until we're back below the limit. */
				if (--streamable > output->interleaver_max_batch_size)
					send_interleaved(output);
			}
		}
	}

	pthread_mutex_unlock(&output->interleaved_mutex);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef DEBUG_STARTING_PACKETS
