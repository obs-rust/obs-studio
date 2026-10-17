/* obs-websocket GetCurrentProgramScene must answer, not abort OBS, when the
 * program scene is gone (upstream obsproject/obs-websocket#1349).
 *
 * In Studio Mode the frontend keeps the program scene as a weak reference
 * (OBSBasic::programScene), and obs_frontend_get_current_scene() returns
 * obs_weak_source_get_source() of it. Remove that scene (RemoveScene, or the
 * scene list) and the reference dies: the call returns NULL. GetSceneList
 * already reports that as a null currentProgramSceneName. GetCurrentProgramScene
 * passed the NULL name straight into a json string, which throws
 * std::logic_error ("basic_string: construction from null is not valid") on
 * the websocket thread, so OBS aborts. GetCurrentPreviewScene has the same
 * shape.
 *
 * This compiles the plugin's scene requests (RequestHandler_Scenes.cpp with
 * Request.cpp and the Utils helpers it calls) against real libobs. The frontend
 * calls are defined here the way OBSStudioAPI answers them in Studio Mode: from
 * weak references to real scenes. RequestHandler's private methods are reached
 * through explicitly instantiated member pointers; its constructor only stores
 * a session, so it is defined here instead of linking RequestHandler.cpp, which
 * would pull in every request of the plugin. obs_startup() needs a display on
 * Linux; the cases skip without one. */

/* The C++ headers come before cmocka.h: its macros break libstdc++. */
#include <cstdio>
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

/* GetCurrentProgramScene and GetCurrentPreviewScene are private. An explicit
 * instantiation may name them. */
using RequestFn = RequestResult (RequestHandler::*)(const Request &);
template<RequestFn F> struct UnlockProgram {
	friend RequestFn get_current_program_scene() { return F; }
};
RequestFn get_current_program_scene();
template struct UnlockProgram<&RequestHandler::GetCurrentProgramScene>;
template<RequestFn F> struct UnlockPreview {
	friend RequestFn get_current_preview_scene() { return F; }
};
RequestFn get_current_preview_scene();
template struct UnlockPreview<&RequestHandler::GetCurrentPreviewScene>;

/* The frontend in Studio Mode: program and preview are weak references. */
static obs_weak_source_t *program_ref;
static obs_weak_source_t *preview_ref;

extern "C" {
obs_source_t *obs_frontend_get_current_scene(void)
{
	return obs_weak_source_get_source(program_ref);
}
obs_source_t *obs_frontend_get_current_preview_scene(void)
{
	return obs_weak_source_get_source(preview_ref);
}
bool obs_frontend_preview_program_mode_active(void)
{
	return true;
}

/* The helpers and the other scene requests linked in also call these. None of
 * them runs here. */
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

static bool obs_up;
static obs_scene_t *scene;
static std::string scene_uuid;

static int setup(void **)
{
	/* obs_startup() needs a display on Linux (the test runs under xvfb-run when
	 * it is installed); the cases skip without one. */
	obs_up = obs_startup("en-US", nullptr, nullptr);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int teardown(void **)
{
	if (obs_up) {
		obs_shutdown();
	}
	return 0;
}

/* A fresh scene each case, which is both the program and the preview scene. */
static int case_setup(void **)
{
	if (!obs_up) {
		return 0;
	}
	scene = obs_scene_create("ws current scene");
	obs_source_t *source = obs_scene_get_source(scene);
	scene_uuid = obs_source_get_uuid(source);
	program_ref = obs_source_get_weak_source(source);
	preview_ref = obs_source_get_weak_source(source);
	return scene ? 0 : -1;
}

static int case_teardown(void **)
{
	if (!obs_up) {
		return 0;
	}
	if (scene) {
		obs_source_remove(obs_scene_get_source(scene));
		obs_scene_release(scene);
		scene = nullptr;
	}
	obs_weak_source_release(program_ref);
	obs_weak_source_release(preview_ref);
	program_ref = preview_ref = nullptr;
	return 0;
}

/* What RemoveScene does to the scene the frontend points at: once the last
 * reference goes, the frontend's weak references return NULL. */
static void remove_scene(void)
{
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	scene = nullptr;
	obs_source_t *gone = obs_weak_source_get_source(program_ref);
	assert_null(gone);
}

/* Runs a request; an exception would escape the websocket thread and abort OBS,
 * so it fails the case instead. */
static RequestResult call(RequestFn fn, const char *name)
{
	Request request(name, json::object());
	RequestHandler handler;
	try {
		return (handler.*fn)(request);
	} catch (const std::exception &e) {
		fail_msg("%s threw '%s' (OBS aborts on the websocket thread)", name, e.what());
	}
	return RequestResult::Error(RequestStatus::Unknown);
}

static void assert_name_and_uuid(const json &d, const char *legacy_name, const char *legacy_uuid)
{
	assert_string_equal(d["sceneName"].get<std::string>().c_str(), "ws current scene");
	assert_string_equal(d[legacy_name].get<std::string>().c_str(), "ws current scene");
	assert_string_equal(d["sceneUuid"].get<std::string>().c_str(), scene_uuid.c_str());
	assert_string_equal(d[legacy_uuid].get<std::string>().c_str(), scene_uuid.c_str());
}

static void assert_all_null(const json &d, const char *legacy_name, const char *legacy_uuid)
{
	for (const char *field : {"sceneName", "sceneUuid", legacy_name, legacy_uuid}) {
		if (!d.contains(field) || !d[field].is_null()) {
			fail_msg("%s is %s, expected null", field,
				 d.contains(field) ? d[field].dump().c_str() : "missing");
		}
	}
}

/* Guard: a program scene that exists is reported by name and UUID. */
static void program_scene_reported(void **)
{
	if (!obs_up) {
		skip();
	}
	RequestResult r = call(get_current_program_scene(), "GetCurrentProgramScene");
	assert_int_equal(r.StatusCode, RequestStatus::Success);
	assert_name_and_uuid(r.ResponseData, "currentProgramSceneName", "currentProgramSceneUuid");
}

/* obs-websocket#1349: the program scene was removed in Studio Mode. */
static void removed_program_scene_reported_null(void **)
{
	if (!obs_up) {
		skip();
	}
	remove_scene();
	RequestResult r = call(get_current_program_scene(), "GetCurrentProgramScene");
	assert_int_equal(r.StatusCode, RequestStatus::Success);
	assert_all_null(r.ResponseData, "currentProgramSceneName", "currentProgramSceneUuid");
}

/* Guard: a preview scene that exists is reported by name and UUID. */
static void preview_scene_reported(void **)
{
	if (!obs_up) {
		skip();
	}
	RequestResult r = call(get_current_preview_scene(), "GetCurrentPreviewScene");
	assert_int_equal(r.StatusCode, RequestStatus::Success);
	assert_name_and_uuid(r.ResponseData, "currentPreviewSceneName", "currentPreviewSceneUuid");
}

/* The same shape on the preview side: Studio Mode on, no preview scene. */
static void removed_preview_scene_reported_null(void **)
{
	if (!obs_up) {
		skip();
	}
	remove_scene();
	RequestResult r = call(get_current_preview_scene(), "GetCurrentPreviewScene");
	assert_int_equal(r.StatusCode, RequestStatus::Success);
	assert_all_null(r.ResponseData, "currentPreviewSceneName", "currentPreviewSceneUuid");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(program_scene_reported, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(removed_program_scene_reported_null, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(preview_scene_reported, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(removed_preview_scene_reported_null, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
