/* obs-websocket SetSceneItemTransform must accept the transform
 * GetSceneItemTransform returns (obs-rust/obs-studio#540, upstream
 * obsproject/obs-websocket#1299 and obsproject/obs-studio#13124).
 *
 * GetSceneItemTransform reports a new item's bounds as libobs stores them, 0
 * x 0 with boundsType OBS_BOUNDS_NONE. SetSceneItemTransform required
 * boundsWidth and boundsHeight to be at least 1 whatever the bounds type, so
 * sending that transform back with a changed position failed with 402 and
 * applied nothing.
 *
 * This compiles the plugin's own scene item requests (RequestHandler_SceneItems.cpp
 * with Request.cpp and the Utils helpers it calls) against real libobs and
 * calls SetSceneItemTransform on a real scene item. No video and no display
 * are needed. RequestHandler's private methods are reached through an
 * explicitly instantiated member pointer; its constructor only stores a session, so
 * it is defined here instead of linking RequestHandler.cpp, which would pull
 * in every request of the plugin. */

/* The C++ headers come before cmocka.h: its macros break libstdc++. */
#include <cstdio>
#include <cstring>

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

/* SetSceneItemTransform is private. An explicit instantiation may name it. */
using RequestFn = RequestResult (RequestHandler::*)(const Request &);
template<RequestFn F> struct Unlock {
	friend RequestFn set_scene_item_transform() { return F; }
};
RequestFn set_scene_item_transform();
template struct Unlock<&RequestHandler::SetSceneItemTransform>;

/* The helpers linked in also hold code for other requests, which the linker
 * wants resolved. None of it runs here, so these stand in for the frontend
 * and for the plugin's Qt-based utilities. */
extern "C" {
void obs_frontend_get_scenes(struct obs_frontend_source_list *) {}
void obs_frontend_get_transitions(struct obs_frontend_source_list *) {}
char **obs_frontend_get_scene_collections(void)
{
	return nullptr;
}
char **obs_frontend_get_profiles(void)
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

static const char *test_name(void *)
{
	return "ws transform bounds test source";
}

static void *test_create(obs_data_t *, obs_source_t *)
{
	return bzalloc(1);
}

static void test_destroy(void *data)
{
	bfree(data);
}

static uint32_t test_size(void *)
{
	return 50;
}

static bool obs_up;
static obs_scene_t *scene;
static obs_source_t *source;
static obs_sceneitem_t *item;
static std::string scene_uuid;
static int64_t item_id;

static int setup(void **)
{
	/* obs_startup() needs a display on Linux (the test runs under xvfb-run when
	 * it is installed); the cases skip without one. */
	obs_up = obs_startup("en-US", nullptr, nullptr);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	/* Without a video size, relative coordinates divide by zero. Absolute
	 * ones store the numbers as given, which is what the requests report. */
	obs_data_t *priv = obs_data_create();
	obs_data_set_bool(priv, "AbsoluteCoordinates", true);
	obs_apply_private_data(priv);
	obs_data_release(priv);

	struct obs_source_info info = {};
	info.id = "ws_transform_bounds_test_source";
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = OBS_SOURCE_VIDEO;
	info.get_name = test_name;
	info.create = test_create;
	info.destroy = test_destroy;
	info.get_width = test_size;
	info.get_height = test_size;
	obs_register_source(&info);
	return 0;
}

static int teardown(void **)
{
	if (obs_up) {
		obs_shutdown();
	}
	return 0;
}

/* A fresh item each case: libobs defaults, bounds 0 x 0 and OBS_BOUNDS_NONE. */
static int case_setup(void **)
{
	if (!obs_up) {
		return 0;
	}
	scene = obs_scene_create("ws transform bounds scene");
	source = obs_source_create("ws_transform_bounds_test_source", "box", nullptr, nullptr);
	item = obs_scene_add(scene, source);
	scene_uuid = obs_source_get_uuid(obs_scene_get_source(scene));
	item_id = obs_sceneitem_get_id(item);
	return item && !scene_uuid.empty() ? 0 : -1;
}

static int case_teardown(void **)
{
	if (!obs_up) {
		return 0;
	}
	obs_source_remove(obs_scene_get_source(scene));
	obs_source_release(source);
	obs_scene_release(scene);
	return 0;
}

static RequestResult set_transform(const json &transform)
{
	json data = {{"sceneUuid", scene_uuid}, {"sceneItemId", item_id}, {"sceneItemTransform", transform}};
	Request request("SetSceneItemTransform", data);
	RequestHandler handler;
	return (handler.*set_scene_item_transform())(request);
}

static json get_transform()
{
	return Utils::Obs::ObjectHelper::GetSceneItemTransform(item);
}

/* What a client does: read the transform, change a field, drop the read-only
 * fields, send the rest back. */
static void get_modify_set_applies(void **)
{
	if (!obs_up) {
		skip();
	}
	json t = get_transform();
	assert_string_equal(t["boundsType"].get<std::string>().c_str(), "OBS_BOUNDS_NONE");
	assert_true(t["boundsWidth"].get<float>() == 0.0f);
	assert_true(t["boundsHeight"].get<float>() == 0.0f);

	for (const char *ro : {"width", "height", "sourceWidth", "sourceHeight"}) {
		t.erase(ro);
	}
	t["positionX"] = 100.0;

	RequestResult r = set_transform(t);
	if (r.StatusCode != RequestStatus::Success) {
		fprintf(stderr, "status %d: %s\n", (int)r.StatusCode, r.Comment.c_str());
	}
	assert_int_equal(r.StatusCode, RequestStatus::Success);

	struct obs_transform_info info;
	obs_sceneitem_get_info2(item, &info);
	assert_true(info.pos.x == 100.0f);
}

static void zero_bounds_with_explicit_none(void **)
{
	if (!obs_up) {
		skip();
	}
	RequestResult r =
		set_transform({{"boundsType", "OBS_BOUNDS_NONE"}, {"boundsWidth", 0.0}, {"boundsHeight", 0.0}});
	if (r.StatusCode != RequestStatus::Success) {
		fprintf(stderr, "status %d: %s\n", (int)r.StatusCode, r.Comment.c_str());
	}
	assert_int_equal(r.StatusCode, RequestStatus::Success);
}

/* Guard: bounds in use still need at least 1. */
static void zero_bounds_with_scale_inner_refused(void **)
{
	if (!obs_up) {
		skip();
	}
	RequestResult r =
		set_transform({{"boundsType", "OBS_BOUNDS_SCALE_INNER"}, {"boundsWidth", 0.0}, {"boundsHeight", 0.0}});
	assert_int_equal(r.StatusCode, RequestStatus::RequestFieldOutOfRange);
	assert_int_equal(r.StatusCode, 402);

	struct obs_transform_info info;
	obs_sceneitem_get_info2(item, &info);
	assert_int_equal(info.bounds_type, OBS_BOUNDS_NONE);
}

static void real_bounds_with_scale_inner_applied(void **)
{
	if (!obs_up) {
		skip();
	}
	RequestResult r = set_transform(
		{{"boundsType", "OBS_BOUNDS_SCALE_INNER"}, {"boundsWidth", 100.0}, {"boundsHeight", 80.0}});
	assert_int_equal(r.StatusCode, RequestStatus::Success);

	struct obs_transform_info info;
	obs_sceneitem_get_info2(item, &info);
	assert_int_equal(info.bounds_type, OBS_BOUNDS_SCALE_INNER);
	assert_true(info.bounds.x == 100.0f);
	assert_true(info.bounds.y == 80.0f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(get_modify_set_applies, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(zero_bounds_with_explicit_none, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(zero_bounds_with_scale_inner_refused, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(real_bounds_with_scale_inner_applied, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
