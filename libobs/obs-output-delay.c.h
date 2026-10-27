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

#include "libobs/build/libobs-ported-output-delay.pch.h"

/* The delay fields are read through obs_output_delay_fields(), not the
 * struct obs_output layout. */
typedef struct obs_output_delay_fields od_fields_t;

static inline bool od_delay_active(const od_fields_t *f)
{
	return os_atomic_load_bool(f->delay_active);
}

static inline bool od_delay_capturing(const od_fields_t *f)
{
	return os_atomic_load_bool(f->delay_capturing);
}

static inline bool od_flag_encoded(const od_fields_t *f)
{
	return (*f->flags & OBS_OUTPUT_ENCODED) != 0;
}

static inline bool od_log_flag_encoded(const od_fields_t *f, const char *func_name, bool inverse_log)
{
	const char *prefix = inverse_log ? "n encoded" : " raw";
	bool ret = od_flag_encoded(f);
	if ((!inverse_log && !ret) || (inverse_log && ret))
		blog(LOG_WARNING, "Output '%s': Tried to use %s on a%s output", *f->name, func_name, prefix);
	return ret;
}

static inline void push_packet(const od_fields_t *f, struct encoder_packet *packet,
			       struct encoder_packet_time *packet_time, uint64_t t)
{
	struct delay_data dd;

	dd.msg = DELAY_MSG_PACKET;
	dd.ts = t;
	dd.packet_time_valid = packet_time != NULL;
	if (packet_time != NULL)
		dd.packet_time = *packet_time;
	obs_encoder_packet_create_instance(&dd.packet, packet);

	pthread_mutex_lock(f->delay_mutex);
	deque_push_back(f->delay_data, &dd, sizeof(dd));
	pthread_mutex_unlock(f->delay_mutex);
}

static inline void process_delay_data(struct obs_output *output, const od_fields_t *f, struct delay_data *dd)
{
	switch (dd->msg) {
	case DELAY_MSG_PACKET:
		if (!od_delay_active(f) || !od_delay_capturing(f))
			obs_encoder_packet_release(&dd->packet);
		else
			(*f->delay_callback)(output, &dd->packet, dd->packet_time_valid ? &dd->packet_time : NULL);
		break;
	case DELAY_MSG_START:
		obs_output_actual_start(output);
		break;
	case DELAY_MSG_STOP:
		obs_output_actual_stop(output, false, dd->ts);
		break;
	}
}

void obs_output_cleanup_delay(obs_output_t *output)
{
	od_fields_t f = obs_output_delay_fields(output);
	struct delay_data dd;

	while (f.delay_data->size) {
		deque_pop_front(f.delay_data, &dd, sizeof(dd));
		if (dd.msg == DELAY_MSG_PACKET) {
			obs_encoder_packet_release(&dd.packet);
		}
	}

	*f.active_delay_ns = 0;
	os_atomic_set_long(f.delay_restart_refs, 0);
}

static inline bool pop_packet(struct obs_output *output, const od_fields_t *f, uint64_t t)
{
	uint64_t elapsed_time;
	struct delay_data dd;
	bool popped = false;
	bool preserve;

	/* ------------------------------------------------ */

	preserve = (*f->delay_cur_flags & OBS_OUTPUT_DELAY_PRESERVE) != 0;

	pthread_mutex_lock(f->delay_mutex);

	if (f->delay_data->size) {
		deque_peek_front(f->delay_data, &dd, sizeof(dd));
		elapsed_time = (t - dd.ts);

		if (preserve && *f->reconnecting) {
			*f->active_delay_ns = elapsed_time;

		} else if (elapsed_time > *f->active_delay_ns) {
			deque_pop_front(f->delay_data, NULL, sizeof(dd));
			popped = true;
		}
	}

	pthread_mutex_unlock(f->delay_mutex);

	/* ------------------------------------------------ */

	if (popped)
		process_delay_data(output, f, &dd);

	return popped;
}

void process_delay(void *data, struct encoder_packet *packet, struct encoder_packet_time *packet_time)
{
	struct obs_output *output = data;
	od_fields_t f = obs_output_delay_fields(output);
	uint64_t t = os_gettime_ns();
	push_packet(&f, packet, packet_time, t);
	while (pop_packet(output, &f, t))
		;
}

bool obs_output_delay_start(obs_output_t *output)
{
	od_fields_t f = obs_output_delay_fields(output);
	struct delay_data dd = {
		.msg = DELAY_MSG_START,
		.ts = os_gettime_ns(),
	};

	if (!od_delay_active(&f)) {
		bool can_begin = obs_output_can_begin_data_capture(output, 0);
		if (!can_begin)
			return false;
		os_atomic_set_bool(f.starting, true);
		if (!obs_output_initialize_encoders(output, 0)) {
			os_atomic_set_bool(f.starting, false);
			return false;
		}
	}

	pthread_mutex_lock(f.delay_mutex);
	deque_push_back(f.delay_data, &dd, sizeof(dd));
	pthread_mutex_unlock(f.delay_mutex);

	os_atomic_inc_long(f.delay_restart_refs);

	if (od_delay_active(&f)) {
		obs_output_delay_signal(output, "starting");
		return true;
	}

	if (!obs_output_begin_data_capture(output, 0)) {
		obs_output_cleanup_delay(output);
		return false;
	}

	return true;
}

void obs_output_delay_stop(obs_output_t *output)
{
	od_fields_t f = obs_output_delay_fields(output);
	struct delay_data dd = {
		.msg = DELAY_MSG_STOP,
		.ts = os_gettime_ns(),
	};

	pthread_mutex_lock(f.delay_mutex);
	deque_push_back(f.delay_data, &dd, sizeof(dd));
	pthread_mutex_unlock(f.delay_mutex);

	obs_output_delay_signal(output, "stopping");
}

void obs_output_set_delay(obs_output_t *output, uint32_t delay_sec, uint32_t flags)
{
	if (!obs_output_valid(output, "obs_output_set_delay"))
		return;

	od_fields_t f = obs_output_delay_fields(output);
	if (!od_log_flag_encoded(&f, __func__, false))
		return;

	*f.delay_sec = delay_sec;
	*f.delay_flags = flags;
}

uint32_t obs_output_get_delay(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_set_delay") ? *obs_output_delay_fields(output).delay_sec : 0;
}

uint32_t obs_output_get_active_delay(const obs_output_t *output)
{
	return obs_output_valid(output, "obs_output_set_delay")
		       ? (uint32_t)(*obs_output_delay_fields(output).active_delay_ns / 1000000000ULL)
		       : 0;
}
