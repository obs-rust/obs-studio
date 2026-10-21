/* Test-only oracle: the original libobs/obs-output-interleave.c.h,
 * unmodified, with its three external functions renamed oracle_* so they
 * can link next to the Rust shims, os_gettime_ns() renamed to a clock the
 * test sets, and non-static oracle_output_interleave_* wrappers around the
 * static functions the Tier 3 test compares one by one.
 *
 * Everything else the file calls keeps its libobs name: the accessors
 * obs_output_interleave_fields(), obs_output_caption_track_fields(),
 * obs_encoder_interleave_info() and obs_encoder_group_num_started(), the
 * obs-output.c wrappers (obs_output_interleave_active(), the two first
 * encoder index helpers and obs_output_interleave_add_caption()),
 * obs_encoder_packet_create_instance() and _release(), obs_output_get_name()
 * and obs_encoder_get_name(). The cargo test that links this file defines
 * them, recording each call, and the Rust shim calls the same ones, so both
 * sides are seen through the same stubs. pthread, blog and the darray come
 * from the real headers and libraries. */
/* As libobs is built for release (RelWithDebInfo defines NDEBUG): the
 * file's asserts and those of the darray.h inlines it calls are off. One of
 * them is reachable: discard_to_idx() pops the packet time of each video
 * packet it drops, and darray_pop_front() asserts that there is one, then
 * does nothing when there is not; a video packet that came without a packet
 * time hits it. The Rust darray port does nothing there too. */
#define NDEBUG 1

#include <stddef.h>
#include <stdint.h>

#define interleave_packets oracle_interleave_packets
#define get_encoder_index oracle_get_encoder_index
#define free_packets oracle_free_packets
#define os_gettime_ns oracle_output_interleave_clock_ns

/* send_interleaved() names itself with __FUNCTION__, a GCC, Clang and MSVC
 * extension that -Wpedantic rejects, and the file converts between signed
 * and unsigned integers implicitly (an int index into a size_t, the size_t
 * frame size in get_encoder_duration()), which -Wsign-conversion rejects,
 * and its two packet-timing debug lines print a size_t with %lu, which
 * -Wformat rejects where size_t is not unsigned long (64-bit Windows).
 * libobs builds it without those warnings (MSVC on Windows). The file is
 * compiled unmodified. */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wformat"
#endif
#include "obs-output-interleave.c.h"
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#endif

/* The static functions, as the Rust shim's ffi::output_interleave::statics
 * has them. */
size_t oracle_output_interleave_start_idx(struct obs_output *output);
int oracle_output_interleave_prune_premature(struct obs_output *output);
bool oracle_output_interleave_prune(struct obs_output *output);
bool oracle_output_interleave_initialize(struct obs_output *output);
void oracle_output_interleave_resort(struct obs_output *output);
void oracle_output_interleave_discard_unused_audio(struct obs_output *output, int64_t dts_usec);
void oracle_output_interleave_send(struct obs_output *output);
void oracle_output_interleave_check_group_keyframe(struct obs_output *output, struct encoder_packet *packet);
size_t oracle_output_interleave_count_streamable(struct obs_output *output);
void oracle_output_interleave_apply_ept_offsets(struct obs_output *output);

size_t oracle_output_interleave_start_idx(struct obs_output *output)
{
	return get_interleaved_start_idx(output);
}

int oracle_output_interleave_prune_premature(struct obs_output *output)
{
	return prune_premature_packets(output);
}

bool oracle_output_interleave_prune(struct obs_output *output)
{
	return prune_interleaved_packets(output);
}

bool oracle_output_interleave_initialize(struct obs_output *output)
{
	return initialize_interleaved_packets(output);
}

void oracle_output_interleave_resort(struct obs_output *output)
{
	resort_interleaved_packets(output);
}

void oracle_output_interleave_discard_unused_audio(struct obs_output *output, int64_t dts_usec)
{
	discard_unused_audio_packets(output, dts_usec);
}

void oracle_output_interleave_send(struct obs_output *output)
{
	send_interleaved(output);
}

void oracle_output_interleave_check_group_keyframe(struct obs_output *output, struct encoder_packet *packet)
{
	check_encoder_group_keyframe_alignment(output, packet);
}

size_t oracle_output_interleave_count_streamable(struct obs_output *output)
{
	return count_streamable_frames(output);
}

void oracle_output_interleave_apply_ept_offsets(struct obs_output *output)
{
	apply_ept_offsets(output);
}

/* The values and layouts the C uses, in the order of
 * rust/obs-c-oracle/src/output_interleave.rs. */
enum {
	ORACLE_OUTPUT_INTERLEAVE_VALUE_COUNT = 9,
	ORACLE_OUTPUT_INTERLEAVE_LAYOUT_COUNT = 67,
};

int64_t oracle_output_interleave_value(size_t which);
size_t oracle_output_interleave_layout(size_t which);

int64_t oracle_output_interleave_value(size_t which)
{
	const int64_t values[ORACLE_OUTPUT_INTERLEAVE_VALUE_COUNT] = {
		OBS_ENCODER_AUDIO,
		OBS_ENCODER_VIDEO,
		KEYFRAME_TRACK_STATUS_NOT_SEEN,
		KEYFRAME_TRACK_STATUS_SEEN,
		KEYFRAME_TRACK_STATUS_SKIPPED,
		LOG_WARNING,
		LOG_DEBUG,
		MAX_OUTPUT_VIDEO_ENCODERS,
		MAX_OUTPUT_AUDIO_ENCODERS,
	};
	return which < ORACLE_OUTPUT_INTERLEAVE_VALUE_COUNT ? values[which] : -1;
}

size_t oracle_output_interleave_layout(size_t which)
{
	const size_t values[ORACLE_OUTPUT_INTERLEAVE_LAYOUT_COUNT] = {
		sizeof(struct keyframe_group_data),
		_Alignof(struct keyframe_group_data),
		offsetof(struct keyframe_group_data, group_id),
		offsetof(struct keyframe_group_data, pts),
		offsetof(struct keyframe_group_data, required_tracks),
		offsetof(struct keyframe_group_data, seen_on_track),
		sizeof(enum keyframe_group_track_status),
		sizeof(struct packet_callback),
		_Alignof(struct packet_callback),
		offsetof(struct packet_callback, packet_cb),
		offsetof(struct packet_callback, param),
		sizeof(struct caption_text),
		_Alignof(struct caption_text),
		offsetof(struct caption_text, display_duration),
		offsetof(struct caption_text, next),
		offsetof(struct caption_text, text),
		sizeof(struct darray),
		_Alignof(struct darray),
		offsetof(struct darray, array),
		offsetof(struct darray, num),
		offsetof(struct darray, capacity),
		sizeof(obs_encoder_packet_array_t),
		sizeof(obs_encoder_packet_time_array_t),
		sizeof(obs_keyframe_group_array_t),
		sizeof(obs_packet_callback_array_t),
		sizeof(struct obs_output_interleave_fields),
		_Alignof(struct obs_output_interleave_fields),
		offsetof(struct obs_output_interleave_fields, interleaved_mutex),
		offsetof(struct obs_output_interleave_fields, interleaved_packets),
		offsetof(struct obs_output_interleave_fields, received_video),
		offsetof(struct obs_output_interleave_fields, received_audio),
		offsetof(struct obs_output_interleave_fields, video_offsets),
		offsetof(struct obs_output_interleave_fields, audio_offsets),
		offsetof(struct obs_output_interleave_fields, highest_audio_ts),
		offsetof(struct obs_output_interleave_fields, highest_video_ts),
		offsetof(struct obs_output_interleave_fields, video_encoders),
		offsetof(struct obs_output_interleave_fields, audio_encoders),
		offsetof(struct obs_output_interleave_fields, encoder_packet_times),
		offsetof(struct obs_output_interleave_fields, keyframe_group_tracking),
		offsetof(struct obs_output_interleave_fields, pkt_callbacks_mutex),
		offsetof(struct obs_output_interleave_fields, pkt_callbacks),
		offsetof(struct obs_output_interleave_fields, total_frames),
		offsetof(struct obs_output_interleave_fields, interleaver_max_batch_size),
		offsetof(struct obs_output_interleave_fields, active_delay_ns),
		offsetof(struct obs_output_interleave_fields, encoded_packet),
		offsetof(struct obs_output_interleave_fields, data),
		offsetof(struct obs_output_interleave_fields, name),
		sizeof(struct obs_output_caption_track_fields),
		_Alignof(struct obs_output_caption_track_fields),
		offsetof(struct obs_output_caption_track_fields, caption_mutex),
		offsetof(struct obs_output_caption_track_fields, caption_head),
		offsetof(struct obs_output_caption_track_fields, caption_timestamp),
		offsetof(struct obs_output_caption_track_fields, last_caption_timestamp),
		offsetof(struct obs_output_caption_track_fields, caption_data_size),
		sizeof(struct obs_encoder_interleave_info),
		_Alignof(struct obs_encoder_interleave_info),
		offsetof(struct obs_encoder_interleave_info, timebase_num),
		offsetof(struct obs_encoder_interleave_info, timebase_den),
		offsetof(struct obs_encoder_interleave_info, framesize),
		offsetof(struct obs_encoder_interleave_info, group),
		sizeof(struct encoder_packet),
		_Alignof(struct encoder_packet),
		sizeof(struct encoder_packet_time),
		_Alignof(struct encoder_packet_time),
		sizeof(int),
		sizeof(size_t),
		sizeof(bool),
	};
	return which < ORACLE_OUTPUT_INTERLEAVE_LAYOUT_COUNT ? values[which] : 0;
}
