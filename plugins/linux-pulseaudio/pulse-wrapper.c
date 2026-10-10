/*
Copyright (C) 2014 by Leonhard Oelke <leonhard@in-verted.de>

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
*/

#include <pthread.h>

#include <pulse/thread-mainloop.h>
#include <pulse/rtclock.h>
#include <pulse/timeval.h>

#include <util/base.h>
#include <util/darray.h>
#include <obs.h>

#include "pulse-wrapper.h"

/* global data */
static uint_fast32_t pulse_refs = 0;
static pthread_mutex_t pulse_mutex = PTHREAD_MUTEX_INITIALIZER;
static pa_threaded_mainloop *pulse_mainloop = NULL;
static pa_context *pulse_context = NULL;

/* Reconnection after the server went away. Only a connection that worked
 * once is retried; if there never was a server, fail as before. */
#define PULSE_RECONNECT_MAX_DELAY_USEC (10 * PA_USEC_PER_SEC)
static bool pulse_was_ready = false;
static bool pulse_reconnecting = false;
static uint32_t pulse_reconnect_attempts = 0;

struct pulse_reconnected_callback {
	pulse_reconnected_cb cb;
	void *userdata;
};
static DARRAY(struct pulse_reconnected_callback) pulse_reconnected_callbacks;

static void pulse_connect_context(pa_context_flags_t flags);

/* Replace the failed context. Runs from a timer, outside the failed context's
 * own callbacks, so it can be freed here. */
static void pulse_reconnect(pa_mainloop_api *api, pa_time_event *e, const struct timeval *tv, void *userdata)
{
	UNUSED_PARAMETER(tv);
	UNUSED_PARAMETER(userdata);

	api->time_free(e);

	/* pulse_unref() ran in the meantime. */
	if (!pulse_reconnecting)
		return;

	pa_context_unref(pulse_context);
	/* NOFAIL: wait for the server to come back instead of failing. */
	pulse_connect_context(PA_CONTEXT_NOAUTOSPAWN | PA_CONTEXT_NOFAIL);
}

/**
 * context status change callback
 *
 * When the connection to the server is lost (e.g. pipewire-pulse restarted),
 * replace the context and tell the sources once the new one is ready, so they
 * can recreate their streams.
 */
static void pulse_context_state_changed(pa_context *c, void *userdata)
{
	UNUSED_PARAMETER(userdata);

	if (c != pulse_context)
		goto out;

	switch (pa_context_get_state(c)) {
	case PA_CONTEXT_FAILED: {
		pa_usec_t delay = 0;

		if (!pulse_was_ready)
			break;

		if (!pulse_reconnecting)
			blog(LOG_WARNING, "pulse-input: Lost the connection to the server, reconnecting");

		/* Retry at once, then back off: a server that refuses or rejects
		 * the connection fails again right away. */
		if (pulse_reconnect_attempts > 0) {
			delay = (PA_USEC_PER_SEC / 4) << (pulse_reconnect_attempts < 6 ? pulse_reconnect_attempts : 6);
			if (delay > PULSE_RECONNECT_MAX_DELAY_USEC)
				delay = PULSE_RECONNECT_MAX_DELAY_USEC;
		}
		pulse_reconnect_attempts++;
		pulse_reconnecting = true;

		pa_context_set_state_callback(c, NULL, NULL);
		pa_context_rttime_new(c, pa_rtclock_now() + delay, pulse_reconnect, NULL);
		break;
	}
	case PA_CONTEXT_READY:
		pulse_was_ready = true;
		pulse_reconnect_attempts = 0;
		if (!pulse_reconnecting)
			break;
		pulse_reconnecting = false;
		blog(LOG_INFO, "pulse-input: Reconnected to the server");
		for (size_t i = 0; i < pulse_reconnected_callbacks.num; i++)
			pulse_reconnected_callbacks.array[i].cb(pulse_reconnected_callbacks.array[i].userdata);
		break;
	default:
		break;
	}

out:
	pulse_signal(0);
}

void pulse_add_reconnected_callback(pulse_reconnected_cb cb, void *userdata)
{
	struct pulse_reconnected_callback callback = {cb, userdata};

	da_push_back(pulse_reconnected_callbacks, &callback);
}

void pulse_remove_reconnected_callback(pulse_reconnected_cb cb, void *userdata)
{
	for (size_t i = 0; i < pulse_reconnected_callbacks.num; i++) {
		struct pulse_reconnected_callback *callback = &pulse_reconnected_callbacks.array[i];

		if (callback->cb == cb && callback->userdata == userdata) {
			da_erase(pulse_reconnected_callbacks, i);
			return;
		}
	}
}

struct pulse_deferred_call {
	pulse_reconnected_cb cb;
	void *userdata;
	void (*free_userdata)(void *userdata);
};

static void pulse_deferred_call_fire(pa_mainloop_api *api, pa_time_event *e, const struct timeval *tv, void *userdata)
{
	UNUSED_PARAMETER(tv);

	struct pulse_deferred_call *call = userdata;
	pulse_reconnected_cb cb = call->cb;
	void *cb_userdata = call->userdata;

	/* The callback now owns userdata; time_free() runs the destroy hook. */
	call->free_userdata = NULL;
	api->time_free(e);
	cb(cb_userdata);
}

/* Also runs for calls still pending when the mainloop is freed. */
static void pulse_deferred_call_destroy(pa_mainloop_api *api, pa_time_event *e, void *userdata)
{
	UNUSED_PARAMETER(api);
	UNUSED_PARAMETER(e);

	struct pulse_deferred_call *call = userdata;

	if (call->free_userdata)
		call->free_userdata(call->userdata);
	bfree(call);
}

void pulse_call_later(pulse_reconnected_cb cb, void *userdata, void (*free_userdata)(void *userdata),
		      uint64_t delay_usec)
{
	pa_mainloop_api *api = pa_threaded_mainloop_get_api(pulse_mainloop);
	struct pulse_deferred_call *call = bzalloc(sizeof(*call));
	pa_time_event *e;

	call->cb = cb;
	call->userdata = userdata;
	call->free_userdata = free_userdata;
	e = pa_context_rttime_new(pulse_context, pa_rtclock_now() + delay_usec, pulse_deferred_call_fire, call);
	api->time_set_destroy(e, pulse_deferred_call_destroy);
}

/**
 * get the default properties
 */
static pa_proplist *pulse_properties()
{
	pa_proplist *p = pa_proplist_new();

	pa_proplist_sets(p, PA_PROP_APPLICATION_NAME, "OBS");
	pa_proplist_sets(p, PA_PROP_APPLICATION_ICON_NAME, "obs");
	pa_proplist_sets(p, PA_PROP_MEDIA_ROLE, "production");

	return p;
}

/**
 * Create and connect pulse_context. It is set before connecting so the state
 * callback also sees failures reported during pa_context_connect(). Must be
 * called with the mainloop locked.
 */
static void pulse_connect_context(pa_context_flags_t flags)
{
	pa_proplist *p = pulse_properties();

	pulse_context = pa_context_new_with_proplist(pa_threaded_mainloop_get_api(pulse_mainloop), "OBS", p);
	pa_context_set_state_callback(pulse_context, pulse_context_state_changed, NULL);

	pa_context_connect(pulse_context, NULL, flags, NULL);
	pa_proplist_free(p);
}

/**
 * Initialize the pulse audio context with properties and callback
 */
static void pulse_init_context()
{
	pulse_lock();
	pulse_connect_context(PA_CONTEXT_NOAUTOSPAWN);
	pulse_unlock();
}

/**
 * wait for context to be ready
 */
static int_fast32_t pulse_context_ready()
{
	pulse_lock();

	/* While reconnecting, the server may stay away indefinitely: fail now
	 * instead of blocking the caller. Sources restart once it is back. */
	while (pa_context_get_state(pulse_context) != PA_CONTEXT_READY) {
		if (pulse_reconnecting || !PA_CONTEXT_IS_GOOD(pa_context_get_state(pulse_context))) {
			pulse_unlock();
			return -1;
		}
		pulse_wait();
	}

	pulse_unlock();
	return 0;
}

int_fast32_t pulse_init()
{
	pthread_mutex_lock(&pulse_mutex);

	if (pulse_refs == 0) {
		pulse_mainloop = pa_threaded_mainloop_new();
		pa_threaded_mainloop_start(pulse_mainloop);

		pulse_init_context();
	}

	pulse_refs++;

	pthread_mutex_unlock(&pulse_mutex);

	return 0;
}

void pulse_unref()
{
	pthread_mutex_lock(&pulse_mutex);

	if (--pulse_refs == 0) {
		pulse_lock();
		if (pulse_context != NULL) {
			pa_context_set_state_callback(pulse_context, NULL, NULL);
			pa_context_disconnect(pulse_context);
			pa_context_unref(pulse_context);
			pulse_context = NULL;
		}
		pulse_was_ready = false;
		pulse_reconnecting = false;
		pulse_reconnect_attempts = 0;
		da_free(pulse_reconnected_callbacks);
		pulse_unlock();

		if (pulse_mainloop != NULL) {
			pa_threaded_mainloop_stop(pulse_mainloop);
			pa_threaded_mainloop_free(pulse_mainloop);
			pulse_mainloop = NULL;
		}
	}

	pthread_mutex_unlock(&pulse_mutex);
}

void pulse_lock()
{
	pa_threaded_mainloop_lock(pulse_mainloop);
}

void pulse_unlock()
{
	pa_threaded_mainloop_unlock(pulse_mainloop);
}

void pulse_wait()
{
	pa_threaded_mainloop_wait(pulse_mainloop);
}

void pulse_signal(int wait_for_accept)
{
	pa_threaded_mainloop_signal(pulse_mainloop, wait_for_accept);
}

void pulse_accept()
{
	pa_threaded_mainloop_accept(pulse_mainloop);
}

int_fast32_t pulse_get_source_info_list(pa_source_info_cb_t cb, void *userdata)
{
	if (pulse_context_ready() < 0)
		return -1;

	pulse_lock();

	pa_operation *op = pa_context_get_source_info_list(pulse_context, cb, userdata);
	if (!op) {
		pulse_unlock();
		return -1;
	}
	while (pa_operation_get_state(op) == PA_OPERATION_RUNNING)
		pulse_wait();
	pa_operation_unref(op);

	pulse_unlock();

	return 0;
}

int_fast32_t pulse_get_sink_info_list(pa_sink_info_cb_t cb, void *userdata)
{
	if (pulse_context_ready() < 0)
		return -1;

	pulse_lock();

	pa_operation *op = pa_context_get_sink_info_list(pulse_context, cb, userdata);
	if (!op) {
		pulse_unlock();
		return -1;
	}
	while (pa_operation_get_state(op) == PA_OPERATION_RUNNING)
		pulse_wait();
	pa_operation_unref(op);

	pulse_unlock();

	return 0;
}

int_fast32_t pulse_get_source_info(pa_source_info_cb_t cb, const char *name, void *userdata)
{
	if (pulse_context_ready() < 0)
		return -1;

	pulse_lock();

	pa_operation *op = pa_context_get_source_info_by_name(pulse_context, name, cb, userdata);
	if (!op) {
		pulse_unlock();
		return -1;
	}
	while (pa_operation_get_state(op) == PA_OPERATION_RUNNING)
		pulse_wait();
	pa_operation_unref(op);

	pulse_unlock();

	return 0;
}

int_fast32_t pulse_get_server_info(pa_server_info_cb_t cb, void *userdata)
{
	if (pulse_context_ready() < 0)
		return -1;

	pulse_lock();

	pa_operation *op = pa_context_get_server_info(pulse_context, cb, userdata);
	if (!op) {
		pulse_unlock();
		return -1;
	}
	while (pa_operation_get_state(op) == PA_OPERATION_RUNNING)
		pulse_wait();
	pa_operation_unref(op);

	pulse_unlock();
	return 0;
}

pa_stream *pulse_stream_new(const char *name, const pa_sample_spec *ss, const pa_channel_map *map)
{
	if (pulse_context_ready() < 0)
		return NULL;

	pulse_lock();

	pa_proplist *p = pulse_properties();
	pa_stream *s = pa_stream_new_with_proplist(pulse_context, name, ss, map, p);
	pa_proplist_free(p);

	pulse_unlock();
	return s;
}
