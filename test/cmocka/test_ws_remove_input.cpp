/* obs-websocket RemoveInput must remove the scene items of the input when the
 * request returns, also in scenes that are not rendered (#1613).
 *
 * The protocol says that RemoveInput "will immediately remove all associated
 * scene items". RemoveInput calls only obs_source_remove(). libobs removes the
 * scene items of a removed source later, in update_transforms_and_prune_sources(),
 * which runs only when the scene renders or from obs_scene_prune_sources(). So in
 * a scene that is not on program or preview, the items stay until that scene
 * renders. The frontend prunes all scenes after it removes a scene
 * (RemoveSceneAndReleaseNested); RemoveInput did not.
 *
 * This compiles the plugin's input requests (RequestHandler_Inputs.cpp with
 * Request.cpp and the Utils helpers it calls) against real libobs. No video is
 * reset, so no scene renders: every scene here is a scene that is not rendered.
 * The input is a minimal source type that this test registers. RequestHandler's
 * private methods are reached through an explicitly instantiated member
 * pointer; its constructor only stores a session, so it is defined here instead
 * of linking RequestHandler.cpp. obs_startup() needs a display on Linux; the
 * cases skip without one. */

/* The C++ headers come before cmocka.h: its macros break libstdc++. */
#include <cstdio>
#include <cstring>
#include <exception>
#include <string>

#include <obs.h>

#include "requesthandler/RequestHandler.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
// cmocka.h 1.1 (Debian 13) and the Windows cmocka.h have no extern "C" guard.
extern "C" {
#include <cmocka.h>
}

RequestHandler::RequestHandler(SessionPtr session) : _session(session) {}

/* RemoveInput is private. An explicit instantiation may name it. */
using RequestFn = RequestResult (RequestHandler::*)(const Request &);
template<RequestFn F> struct UnlockRemoveInput {
	friend RequestFn remove_input() { return F; }
};
RequestFn remove_input();
template struct UnlockRemoveInput<&RequestHandler::RemoveInput>;

/* The helpers and the other input requests linked in call these. None of them
 * runs here. */
extern "C" {
obs_source_t *obs_frontend_get_current_scene(void)
{
	return nullptr;
}
obs_source_t *obs_frontend_get_current_preview_scene(void)
{
	return nullptr;
}
bool obs_frontend_preview_program_mode_active(void)
{
	return false;
}
void obs_frontend_get_scenes(struct obs_frontend_source_list *) {}
void obs_frontend_get_transitions(struct obs_frontend_source_list *) {}
void obs_frontend_set_current_scene(obs_source_t *) {}
void obs_frontend_set_current_preview_scene(obs_source_t *) {}
obs_source_t *obs_frontend_get_current_transition(void)
{
	return nullptr;
}
char **obs_frontend_get_scene_collections(void)
{
	return nullptr;
}
char **obs_frontend_get_profiles(void)
{
	return nullptr;
}
obs_data_t *obs_frontend_get_scene_transition_override(obs_source_t *)
{
	return nullptr;
}
}
os_cpu_usage_info_t *GetCpuUsageInfo()
{
	return nullptr;
}
std::string Utils::Obs::StringHelper::GetCurrentRecordOutputPath()
{
	return "";
}

/* A source type with no video and no audio: enough to be an input. */
static const char *test_input_name(void *)
{
	return "ws test input";
}
static void *test_input_create(obs_data_t *, obs_source_t *source)
{
	return source;
}
static void test_input_destroy(void *) {}

static struct obs_source_info test_input_info = {};

static bool obs_up;
static obs_scene_t *scene_a;
static obs_scene_t *scene_b;
static obs_source_t *input;

static int setup(void **)
{
	/* obs_startup() needs a display on Linux (the test runs under xvfb-run when
	 * it is installed); the cases skip without one. */
	obs_up = obs_startup("en-US", nullptr, nullptr);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}
	test_input_info.id = "ws_test_input";
	test_input_info.type = OBS_SOURCE_TYPE_INPUT;
	test_input_info.get_name = test_input_name;
	test_input_info.create = test_input_create;
	test_input_info.destroy = test_input_destroy;
	obs_register_source(&test_input_info);
	return 0;
}

static int teardown(void **)
{
	if (obs_up) {
		obs_shutdown();
	}
	return 0;
}

/* One input, in two scenes. Nothing renders them. */
static int case_setup(void **)
{
	if (!obs_up) {
		return 0;
	}
	scene_a = obs_scene_create("ws remove input a");
	scene_b = obs_scene_create("ws remove input b");
	input = obs_source_create("ws_test_input", "ws remove input", nullptr, nullptr);
	if (!scene_a || !scene_b || !input) {
		return -1;
	}
	/* libobs renames an input whose name is in use. RemoveInput then finds
	 * a leftover from the last case instead of this input. */
	if (strcmp(obs_source_get_name(input), "ws remove input") != 0) {
		fprintf(stderr, "input is named '%s': a source of the last case is still alive\n",
			obs_source_get_name(input));
		return -1;
	}
	obs_scene_add(scene_a, input);
	obs_scene_add(scene_b, input);
	return 0;
}

static int case_teardown(void **)
{
	if (!obs_up) {
		return 0;
	}
	for (obs_scene_t **scene : {&scene_a, &scene_b}) {
		if (*scene) {
			obs_source_remove(obs_scene_get_source(*scene));
			obs_scene_release(*scene);
			*scene = nullptr;
		}
	}
	obs_source_remove(input);
	obs_source_release(input);
	input = nullptr;
	/* libobs destroys sources on its destruction thread. Destroying a scene
	 * queues the destruction of its items' sources behind the wait marker,
	 * so wait until a pass processes nothing, as obs_shutdown() does. The
	 * next case can then use the same names. */
	while (obs_wait_for_destroy_queue())
		;
	return 0;
}

static size_t item_count(obs_scene_t *scene)
{
	size_t count = 0;
	auto cb = [](obs_scene_t *, obs_sceneitem_t *, void *param) {
		(*static_cast<size_t *>(param))++;
		return true;
	};
	obs_scene_enum_items(scene, cb, &count);
	return count;
}

/* Runs RemoveInput; an exception would escape the websocket thread and abort
 * OBS, so it fails the case instead. */
static RequestResult call_remove_input(const char *name)
{
	Request request("RemoveInput", json{{"inputName", name}});
	RequestHandler handler;
	try {
		return (handler.*remove_input())(request);
	} catch (const std::exception &e) {
		fail_msg("RemoveInput threw '%s'", e.what());
	}
	return RequestResult::Error(RequestStatus::Unknown);
}

/* Guard: the input is in both scenes before the request. */
static void input_in_both_scenes(void **)
{
	if (!obs_up) {
		skip();
	}
	assert_int_equal(item_count(scene_a), 1);
	assert_int_equal(item_count(scene_b), 1);
}

/* #1613: the scene items are gone when RemoveInput returns. */
static void remove_input_removes_scene_items(void **)
{
	if (!obs_up) {
		skip();
	}
	RequestResult r = call_remove_input("ws remove input");
	assert_int_equal(r.StatusCode, RequestStatus::Success);
	assert_true(obs_source_removed(input));
	assert_int_equal(item_count(scene_a), 0);
	assert_int_equal(item_count(scene_b), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(input_in_both_scenes, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(remove_input_removes_scene_items, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
