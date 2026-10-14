// The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
// with libstdc++.
#include <vector>

#include "utils/Obs.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

/* obs-websocket's RemoveScene refuses to remove the last scene of the main
 * canvas by asking Utils::Obs::NumberHelper::GetCanvasSceneCount() how many
 * scenes it has ("You cannot remove the last scene in the collection." when
 * the answer is below 2). The counter must start at zero: when it does not,
 * a collection with one scene looks like it has more, RemoveScene deletes the
 * last scene, and the next request that needs a current scene
 * (GetCurrentProgramScene, CreateScene) aborts OBS.
 *
 * The helper is built from the plugin's own source against the libobs
 * functions it calls, stubbed below with a fake canvas, so no display or
 * libobs startup is needed. The test is built with
 * -ftrivial-auto-var-init=pattern where the compiler has it, so an
 * uninitialized counter reads as a huge number every run instead of whatever
 * the stack happened to hold. */

struct fake_scene {
	bool group;
};

static std::vector<fake_scene> scenes;
static int canvas_marker;
static obs_canvas_t *const main_canvas = reinterpret_cast<obs_canvas_t *>(&canvas_marker);
static obs_canvas_t *enumerated_canvas;

extern "C" {
obs_canvas_t *obs_get_main_canvas(void)
{
	return main_canvas;
}

void obs_canvas_release(obs_canvas_t *canvas)
{
	UNUSED_PARAMETER(canvas);
}

void obs_canvas_enum_scenes(obs_canvas_t *canvas, bool (*enum_proc)(void *, obs_source_t *), void *param)
{
	enumerated_canvas = canvas;
	for (fake_scene &scene : scenes) {
		if (!enum_proc(param, reinterpret_cast<obs_source_t *>(&scene))) {
			break;
		}
	}
}

bool obs_source_is_group(const obs_source_t *source)
{
	return reinterpret_cast<const fake_scene *>(source)->group;
}

/* Used by the helper's other functions, not by the scene count. */
bool obs_output_active(const obs_output_t *output)
{
	UNUSED_PARAMETER(output);
	return false;
}

video_t *obs_output_video(const obs_output_t *output)
{
	UNUSED_PARAMETER(output);
	return nullptr;
}

int obs_output_get_total_frames(const obs_output_t *output)
{
	UNUSED_PARAMETER(output);
	return 0;
}

uint64_t video_output_get_frame_time(const video_t *video)
{
	UNUSED_PARAMETER(video);
	return 0;
}

void obs_source_enum_filters(obs_source_t *source, obs_source_enum_proc_t callback, void *param)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(param);
}
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	scenes.clear();
	enumerated_canvas = nullptr;
	return 0;
}

static void add_scenes(size_t count, bool group = false)
{
	for (size_t i = 0; i < count; i++) {
		scenes.push_back({group});
	}
}

static void test_no_scenes(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(Utils::Obs::NumberHelper::GetSceneCount(), 0);
}

static void test_one_scene(void **state)
{
	UNUSED_PARAMETER(state);
	add_scenes(1);

	/* RemoveScene must see this as the last scene. */
	assert_int_equal(Utils::Obs::NumberHelper::GetSceneCount(), 1);
}

static void test_two_scenes(void **state)
{
	UNUSED_PARAMETER(state);
	add_scenes(2);
	assert_int_equal(Utils::Obs::NumberHelper::GetSceneCount(), 2);
}

static void test_groups_not_counted(void **state)
{
	UNUSED_PARAMETER(state);
	add_scenes(1);
	add_scenes(3, true);
	assert_int_equal(Utils::Obs::NumberHelper::GetSceneCount(), 1);
}

static void test_canvas_count(void **state)
{
	UNUSED_PARAMETER(state);
	add_scenes(3);
	add_scenes(1, true);
	assert_int_equal(Utils::Obs::NumberHelper::GetCanvasSceneCount(main_canvas), 3);
	assert_ptr_equal(enumerated_canvas, main_canvas);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(test_no_scenes, setup),
		cmocka_unit_test_setup(test_one_scene, setup),
		cmocka_unit_test_setup(test_two_scenes, setup),
		cmocka_unit_test_setup(test_groups_not_counted, setup),
		cmocka_unit_test_setup(test_canvas_count, setup),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
