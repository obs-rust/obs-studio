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

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The encoder group reconfigure rules of obs-encoder.c, on plain values
 * (obs-encoder-group-reconfig.c). When an active encoder of a group gets new
 * settings, every encoder of the group applies them on the same frame, so
 * that their new keyframes line up:
 * - obs_encoder_group_reconfig_update(): obs_encoder_update(), the group
 *   branch: start a new request, or hold a second one back
 *   (reconfigure_again);
 * - obs_encoder_group_reconfig_new_request():
 *   encoder_group_new_reconfigure_request();
 * - obs_encoder_group_reconfig_frame(): handle_encoder_group_reconfigure_request(),
 *   for each video frame of a grouped encoder (receive_video() and the GPU
 *   encode thread): apply the settings on this frame or not, and the
 *   group's next shared frame;
 * - obs_encoder_group_reconfig_lcm(): add_connection(), the least common
 *   multiple of the group's frame rate divisors (gcd() and lcm());
 * - obs_encoder_group_reconfig_reset(): remove_connection(), the reconfigure
 *   fields once no encoder of the group is started.
 *
 * The arithmetic is C's. lcm() works in int; a product above INT_MAX is
 * undefined in C, and so are a divisor of 0 and a value above INT_MAX
 * (gcd() divides by 0 or does not end). In the frame step the uint32_t
 * products wrap; next_pts beyond int64_t and a timestep of 0 (timebase_num
 * or the LCM 0) are undefined in C.
 *
 * obs-encoder.c keeps obs_encoder_valid(), obs_data_apply(), the
 * context.data, info.update and active tests, the group mutex, every read
 * and write of the encoder and the group (it copies the fields in and out
 * around each call), info.update() in do_encode() and the GPU encode thread,
 * obs_encoder_set_group() and the group create and destroy. Nothing here
 * takes a lock, calls into libobs or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The reconfigure fields of struct obs_encoder_group. */
struct obs_encoder_group_reconfig {
	uint64_t reconfigure_request;
	int64_t next_pts;
	uint32_t encoders_updated_next_pts;
	uint32_t encoders_reconfigured;
	bool reconfigure_again;
};

/* The reconfigure fields of struct obs_encoder. */
struct obs_encoder_reconfig {
	uint64_t last_reconfigure_request;
	uint64_t last_handled_reconfigure_request;
};

/* encoder_group_new_reconfigure_request(): the request counter steps by one,
 * and the counts of this request and reconfigure_again start again. */
void obs_encoder_group_reconfig_new_request(struct obs_encoder_group_reconfig *group);

/* obs_encoder_update() for an active encoder in a group, under the group
 * mutex: num_encoders is the group's encoders.num, and
 * last_handled_reconfigure_request the encoder's. The first request, or one
 * after every encoder applied the last, starts a new request; an encoder
 * that already applied the current request sets reconfigure_again. */
void obs_encoder_group_reconfig_update(struct obs_encoder_group_reconfig *group, size_t num_encoders,
				       uint64_t last_handled_reconfigure_request);

/* handle_encoder_group_reconfigure_request() under the group mutex, for each
 * video frame of an encoder in a group. num_encoders is the group's
 * encoders.num, num_encoders_started and frame_rate_divisors_lcm the group's;
 * cur_pts, timebase_num and frame_rate_divisor the encoder's. True: the
 * encoder applies its settings on this frame (the caller sets
 * reconfigure_requested). */
bool obs_encoder_group_reconfig_frame(struct obs_encoder_group_reconfig *group, size_t num_encoders,
				      uint32_t num_encoders_started, uint32_t frame_rate_divisors_lcm,
				      struct obs_encoder_reconfig *encoder, int64_t cur_pts, uint32_t timebase_num,
				      uint32_t frame_rate_divisor);

/* add_connection() under the group mutex: the group's new
 * frame_rate_divisors_lcm, lcm(lcm, frame_rate_divisor) in int, stored as
 * uint32_t. */
uint32_t obs_encoder_group_reconfig_lcm(uint32_t lcm, uint32_t frame_rate_divisor);

/* remove_connection() under the group mutex, once no encoder of the group is
 * started: every reconfigure field back to 0 (false). */
void obs_encoder_group_reconfig_reset(struct obs_encoder_group_reconfig *group);
