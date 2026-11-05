#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

/* Exercise the real event/restart/timer path without a sound server. The
 * introspection error is deterministic, unlike the PipeWire race in #3630.
 * Existing live default-follow and reconnect tests remain unchanged. */
#include "plugins/linux-pulseaudio/pulse-input.c.h"

static struct pulse_data *retry_data;
static pulse_reconnected_cb retry_cb;
static void *retry_userdata;
static unsigned retry_calls;
static unsigned weak_refs;
static bool source_alive;

/* ASan retains the plugin registration globals and their callbacks. Keep
 * those unrelated entry points linkable, but fail if this fixture reaches
 * one instead of the event/restart path it is meant to exercise. */
const char *obs_module_text(const char *text)
{
	return text;
}
int_fast32_t pulse_init(void)
{
	fail();
	return -1;
}
void pulse_unref(void)
{
	fail();
}
int_fast32_t pulse_get_source_info_list(pa_source_info_cb_t cb, void *userdata)
{
	(void)cb;
	(void)userdata;
	fail();
	return -1;
}
int_fast32_t pulse_get_sink_info_list(pa_sink_info_cb_t cb, void *userdata)
{
	(void)cb;
	(void)userdata;
	fail();
	return -1;
}
void pulse_add_reconnected_callback(pulse_reconnected_cb cb, void *userdata)
{
	(void)cb;
	(void)userdata;
	fail();
}
void pulse_remove_reconnected_callback(pulse_reconnected_cb cb, void *userdata)
{
	(void)cb;
	(void)userdata;
	fail();
}
void pulse_add_source_event_callback(pulse_source_event_cb cb, void *userdata)
{
	(void)cb;
	(void)userdata;
	fail();
}
void pulse_remove_source_event_callback(pulse_source_event_cb cb, void *userdata)
{
	(void)cb;
	(void)userdata;
	fail();
}
void __wrap_obs_source_set_async_compensation(obs_source_t *source, bool enabled)
{
	(void)source;
	(void)enabled;
	fail();
}

void pulse_lock(void) {}
void pulse_unlock(void) {}
void pulse_signal(int wait_for_accept)
{
	(void)wait_for_accept;
}

int_fast32_t pulse_get_server_info(pa_server_info_cb_t cb, void *userdata)
{
	pa_server_info info = {.server_name = "test", .server_version = "test", .default_sink_name = "new"};
	cb(NULL, &info, userdata);
	return 0;
}

int_fast32_t pulse_get_source_info(pa_source_info_cb_t cb, const char *name, void *userdata)
{
	assert_string_equal(name, "new.monitor");
	cb(NULL, NULL, -1, userdata);
	return 0;
}

void pulse_call_later(pulse_reconnected_cb cb, void *userdata, void (*free_userdata)(void *), uint64_t delay_usec)
{
	(void)free_userdata;
	assert_null(retry_cb);
	assert_int_equal(delay_usec, PULSE_RESTART_RETRY_USEC);
	retry_cb = cb;
	retry_userdata = userdata;
	retry_calls++;
}

pa_stream *pulse_stream_new(const char *name, const pa_sample_spec *spec, const pa_channel_map *map)
{
	(void)name;
	(void)spec;
	(void)map;
	fail(); /* Source-info failure must not create a stream. */
	return NULL;
}
void pulse_stream_free(pa_stream *stream)
{
	(void)stream;
	fail();
}

void __wrap_obs_weak_source_addref(obs_weak_source_t *source)
{
	(void)source;
	weak_refs++;
}
void __wrap_obs_weak_source_release(obs_weak_source_t *source)
{
	(void)source;
	assert_true(weak_refs > 0);
	weak_refs--;
}
obs_source_t *__wrap_obs_weak_source_get_source(obs_weak_source_t *source)
{
	(void)source;
	return source_alive ? (obs_source_t *)retry_data : NULL;
}
void *__wrap_obs_obj_get_data(void *source)
{
	return source;
}
void __wrap_obs_source_release(obs_source_t *source)
{
	(void)source;
}
void __wrap_obs_queue_task(enum obs_task_type type, obs_task_t task, void *param, bool wait)
{
	assert_int_equal(type, OBS_TASK_UI);
	assert_false(wait);
	task(param);
}

static void fire_retry(void)
{
	pulse_reconnected_cb cb = retry_cb;
	void *userdata = retry_userdata;
	assert_non_null(cb);
	retry_cb = NULL;
	retry_userdata = NULL;
	cb(userdata);
}

static void queue_default_change(struct pulse_data *data)
{
	retry_data = data;
	source_alive = true;
	data->source = (obs_source_t *)data;
	data->weak_source = (obs_weak_source_t *)data;
	struct pulse_stream_task *event = bzalloc(sizeof(*event));
	event->weak_source = data->weak_source;
	event->event = PULSE_SERVER_CHANGED;
	weak_refs = 1;
	pulse_stream_event_task(event);
	assert_string_equal(data->device, "new.monitor");
	assert_non_null(retry_cb);
	assert_int_equal(retry_calls, 1);
}

static int retry_setup(void **state)
{
	(void)state;
	retry_cb = NULL;
	retry_userdata = NULL;
	retry_calls = 0;
	weak_refs = 0;
	return 0;
}

static void changed_default_retries_transient_source_info_error(void **state)
{
	(void)state;
	struct pulse_data data = {.device = bstrdup("old.monitor"), .is_default = true};
	queue_default_change(&data);
	/* A missing monitor is retried with the same finite budget as a
     * reconnect; it must not poll forever or leak a timer's weak ref. */
	for (unsigned i = 1; i < PULSE_RESTART_MAX_ATTEMPTS; i++)
		fire_retry();
	assert_null(retry_cb);
	assert_int_equal(retry_calls, PULSE_RESTART_MAX_ATTEMPTS - 1);
	assert_int_equal(weak_refs, 0);
	bfree(data.device);
}

static void destroyed_source_drops_pending_default_retry(void **state)
{
	(void)state;
	struct pulse_data data = {.device = bstrdup("old.monitor"), .is_default = true};
	queue_default_change(&data);
	source_alive = false;
	fire_retry();
	assert_null(retry_cb);
	assert_int_equal(retry_calls, 1);
	assert_int_equal(weak_refs, 0);
	bfree(data.device);
}

static void missing_named_device_still_waits_for_device_event(void **state)
{
	(void)state;
	struct pulse_data data = {.device = bstrdup("new.monitor")};
	retry_data = &data;
	source_alive = true;
	data.source = (obs_source_t *)&data;
	data.weak_source = (obs_weak_source_t *)&data;
	struct pulse_stream_task *event = bzalloc(sizeof(*event));
	event->weak_source = data.weak_source;
	event->event = PULSE_SOURCE_NEW;
	weak_refs = 1;
	pulse_stream_event_task(event);
	assert_null(retry_cb);
	assert_int_equal(retry_calls, 0);
	assert_int_equal(weak_refs, 0);
	bfree(data.device);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(changed_default_retries_transient_source_info_error, retry_setup),
		cmocka_unit_test_setup(destroyed_source_drops_pending_default_retry, retry_setup),
		cmocka_unit_test_setup(missing_named_device_still_waits_for_device_event, retry_setup),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
