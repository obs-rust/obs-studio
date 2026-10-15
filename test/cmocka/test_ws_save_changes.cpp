/* obs-websocket requests that change the scene collection must save it
 * (obs-rust/obs-studio#811).
 *
 * SetInputVolume, SetInputSettings, SetSceneItemTransform and the other
 * requests that change scenes, inputs, filters, scene items or transitions
 * changed only the live state. They never asked the frontend to save, so the
 * change was in memory only: a crash, a SIGKILL or a container hard stop lost
 * it. A change made in the OBS UI saves at once (OBSBasic::SaveProject()).
 *
 * This compiles the plugin's request handlers (RequestHandler.cpp and the
 * RequestHandler_*.cpp files, with the rpc and utils sources) against real
 * libobs and the frontend API library, and sends requests through
 * RequestHandler::ProcessRequest. Without a frontend, obs_frontend_save() does
 * nothing, so the test counts the UI tasks the request queues instead: the
 * save runs on the UI thread through obs_queue_task(OBS_TASK_UI, ...). The
 * plugin's globals (config, event handler, server, vendor API) are stubbed; the
 * requests used here do not call them. obs_startup() needs a display on Linux; the
 * cases skip without one. */

/* The C++ headers come before cmocka.h: its macros break libstdc++. */
#include <cstdio>

#include <obs.h>

#include "obs-websocket.h"
#include "WebSocketApi.h"
#include "requesthandler/RequestHandler.h"
#include "websocketserver/WebSocketServer.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
// cmocka.h has no extern "C" guard of its own on every platform.
extern "C" {
#include <cmocka.h>
}

/* Plugin globals from obs-websocket.cpp, which would pull in the server. */
os_cpu_usage_info_t *GetCpuUsageInfo()
{
	return nullptr;
}
ConfigPtr GetConfig()
{
	return nullptr;
}
EventHandlerPtr GetEventHandler()
{
	return nullptr;
}
WebSocketApiPtr GetWebSocketApi()
{
	return nullptr;
}
WebSocketServerPtr GetWebSocketServer()
{
	return nullptr;
}
bool IsDebugEnabled()
{
	return false;
}
bool IsServerOffBySafeMode()
{
	return false;
}
void ClearServerOffBySafeMode() {}

/* Normally defined by OBS_DECLARE_MODULE() in the plugin. */
extern "C" obs_module_t *obs_current_module(void)
{
	return nullptr;
}

/* Referenced by BroadcastCustomEvent and CallVendorRequest, which the test does
 * not send. */
void WebSocketServer::BroadcastEvent(uint64_t, const std::string &, const json &, uint8_t) {}
enum WebSocketApi::RequestReturnCode WebSocketApi::PerformVendorRequest(std::string, std::string, obs_data_t *,
									obs_data_t *)
{
	return NoVendor;
}

static int ui_tasks;

/* Counts the tasks queued for the UI thread and does not run them. */
static void count_ui_task(obs_task_t task, void *param, bool wait)
{
	UNUSED_PARAMETER(task);
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(wait);
	ui_tasks++;
}

static const char *dummy_get_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "ws save dummy";
}

static void *dummy_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void dummy_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static obs_source_t *input;
static bool have_obs;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_startup("en-US", nullptr, nullptr)) {
		fprintf(stderr, "obs_startup failed (no display?); skipping\n");
		return 0;
	}
	have_obs = true;

	struct obs_source_info info = {};
	info.id = "ws_save_dummy";
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = OBS_SOURCE_AUDIO;
	info.get_name = dummy_get_name;
	info.create = dummy_create;
	info.destroy = dummy_destroy;
	obs_register_source(&info);

	input = obs_source_create("ws_save_dummy", "Mic", nullptr, nullptr);
	obs_set_ui_task_handler(count_ui_task);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_obs) {
		return 0;
	}
	obs_set_ui_task_handler(nullptr);
	obs_source_release(input);
	obs_shutdown();
	return 0;
}

static RequestResult send(const char *type, const json &data)
{
	RequestHandler handler(nullptr);
	ui_tasks = 0;
	return handler.ProcessRequest(Request(type, data));
}

static void set_input_volume_queues_save(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_obs) {
		skip();
	}

	RequestResult r = send("SetInputVolume", {{"inputName", "Mic"}, {"inputVolumeDb", -6.0}});

	assert_int_equal(r.StatusCode, RequestStatus::Success);
	if (ui_tasks != 1) {
		fprintf(stderr, "SetInputVolume queued %d UI tasks, expected 1 (the save)\n", ui_tasks);
	}
	assert_int_equal(ui_tasks, 1);
}

static void get_input_volume_does_not_save(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_obs) {
		skip();
	}

	RequestResult r = send("GetInputVolume", {{"inputName", "Mic"}});

	assert_int_equal(r.StatusCode, RequestStatus::Success);
	assert_int_equal(ui_tasks, 0);
}

static void failed_set_does_not_save(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_obs) {
		skip();
	}

	RequestResult r = send("SetInputVolume", {{"inputName", "Missing"}, {"inputVolumeDb", -6.0}});

	assert_int_not_equal(r.StatusCode, RequestStatus::Success);
	assert_int_equal(ui_tasks, 0);
}

int main()
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(set_input_volume_queues_save),
		cmocka_unit_test(get_input_volume_does_not_save),
		cmocka_unit_test(failed_set_does_not_save),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
