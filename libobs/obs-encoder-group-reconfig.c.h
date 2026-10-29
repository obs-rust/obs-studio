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

#include "libobs/build/libobs-ported-encoder-group-reconfig.pch.h"

/* encoder_group_new_reconfigure_request(). */
void obs_encoder_group_reconfig_new_request(struct obs_encoder_group_reconfig *group)
{
	group->reconfigure_request += 1;
	group->encoders_updated_next_pts = 0;
	group->encoders_reconfigured = 0;
	group->reconfigure_again = false;
}

/* obs_encoder_update(), the group branch. */
void obs_encoder_group_reconfig_update(struct obs_encoder_group_reconfig *group, size_t num_encoders,
				       uint64_t last_handled_reconfigure_request)
{
	bool first_reconfigure_request = group->reconfigure_request == 0;
	bool all_encoders_reconfigured = group->encoders_reconfigured == num_encoders;
	bool current_encoder_reconfigured = last_handled_reconfigure_request == group->reconfigure_request;
	if (first_reconfigure_request || all_encoders_reconfigured) {
		obs_encoder_group_reconfig_new_request(group);
	} else if (current_encoder_reconfigured) {
		group->reconfigure_again = true;
	}
}

/* handle_encoder_group_reconfigure_request(). */
bool obs_encoder_group_reconfig_frame(struct obs_encoder_group_reconfig *group, size_t num_encoders,
				      uint32_t num_encoders_started, uint32_t frame_rate_divisors_lcm,
				      struct obs_encoder_reconfig *encoder, int64_t cur_pts, uint32_t timebase_num,
				      uint32_t frame_rate_divisor)
{
	bool reconfigure_requested = false;
	bool new_reconfigure_request = encoder->last_reconfigure_request != group->reconfigure_request;
	if (!new_reconfigure_request && group->encoders_updated_next_pts == num_encoders_started &&
	    cur_pts == group->next_pts &&
	    encoder->last_handled_reconfigure_request != encoder->last_reconfigure_request) {
		group->encoders_reconfigured += 1;

		encoder->last_handled_reconfigure_request = encoder->last_reconfigure_request;
		reconfigure_requested = true;

		if (group->encoders_reconfigured == num_encoders && group->reconfigure_again)
			obs_encoder_group_reconfig_new_request(group);

	} else if (new_reconfigure_request || cur_pts == group->next_pts) {
		encoder->last_reconfigure_request = group->reconfigure_request;

		int64_t overall_timestep = timebase_num * frame_rate_divisors_lcm;

		int64_t next_pts = cur_pts + timebase_num * frame_rate_divisor;

		if (next_pts % overall_timestep != 0)
			next_pts = (next_pts / overall_timestep + 1) * overall_timestep;

		if (next_pts > group->next_pts)
			group->next_pts = next_pts;

		if (new_reconfigure_request)
			group->encoders_updated_next_pts += 1;
	}
	return reconfigure_requested;
}

static int obs_encoder_group_reconfig_gcd(int a, int b)
{
	if (a < b) {
		int temp = a;
		a = b;
		b = temp;
	}

	int remainder = a % b;
	if (!remainder)
		return b;

	return obs_encoder_group_reconfig_gcd(b, remainder);
}

/* add_connection(): lcm() on the group's LCM and the encoder's divisor. */
uint32_t obs_encoder_group_reconfig_lcm(uint32_t lcm, uint32_t frame_rate_divisor)
{
	int a = (int)lcm;
	int b = (int)frame_rate_divisor;

	return (uint32_t)((a / obs_encoder_group_reconfig_gcd(a, b)) * b);
}

/* remove_connection(), once the group is idle. */
void obs_encoder_group_reconfig_reset(struct obs_encoder_group_reconfig *group)
{
	group->reconfigure_request = 0;
	group->next_pts = 0;
	group->encoders_reconfigured = 0;
	group->encoders_updated_next_pts = 0;
	group->reconfigure_again = false;
}
