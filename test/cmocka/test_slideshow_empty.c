#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs-module.h>
#include <util/bmem.h>
#include <string.h>
#ifndef _WIN32
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

extern struct obs_source_info slideshow_info;
extern struct obs_source_info cut_transition;

/* Module-local services for the image-source fixture. No decoder or graphics
 * session is needed to test the slideshow's active-child lifetime. */
const char *obs_module_text(const char *text)
{
	return text;
}

uint64_t image_source_get_memory_usage(void *data)
{
	UNUSED_PARAMETER(data);
	return 0;
}

void image_source_preload_image(void *data)
{
	UNUSED_PARAMETER(data);
}

void image_source_ready_slide(void *data)
{
	UNUSED_PARAMETER(data);
}

static const char *image_name(void *data)
{
	UNUSED_PARAMETER(data);
	return "Fixture image";
}

static void *image_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void image_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static uint32_t image_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 16;
}

static const struct obs_source_info image_info = {
	.id = "image_source",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = image_name,
	.create = image_create,
	.destroy = image_destroy,
	.get_width = image_size,
	.get_height = image_size,
};

struct fixture {
	obs_source_t *source;
	obs_source_t *transition;
	obs_data_t *settings;
	size_t ended;
	size_t reentered;
	size_t slides;
	int last_index;
	char last_path[64];
	int first_index;
	char first_path[64];
};

static void get_transition(obs_source_t *parent, obs_source_t *child, void *param)
{
	UNUSED_PARAMETER(parent);
	struct fixture *fixture = param;
	assert_null(fixture->transition);
	assert_int_equal(obs_source_get_type(child), OBS_SOURCE_TYPE_TRANSITION);
	fixture->transition = obs_source_get_ref(child);
}

static void media_ended(void *param, calldata_t *data)
{
	UNUSED_PARAMETER(data);
	struct fixture *fixture = param;
	fixture->ended++;
}

static void check_transition(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct fixture *fixture = param;
	assert_ptr_equal(parent, fixture->source);
	assert_ptr_equal(child, fixture->transition);
	fixture->reentered++;
}

static void media_ended_reenter(void *param, calldata_t *data)
{
	UNUSED_PARAMETER(data);
	struct fixture *fixture = param;
	/* Signal subscribers may synchronously read the slideshow's children. */
	obs_source_enum_active_sources(fixture->source, check_transition, fixture);
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_ENDED);
}

static void slide_changed_reenter(void *param, calldata_t *data)
{
	UNUSED_PARAMETER(data);
	struct fixture *fixture = param;
	/* Signal subscribers may synchronously read the slideshow's children. */
	obs_source_enum_active_sources(fixture->source, check_transition, fixture);
}

static void slide_changed_record(void *param, calldata_t *data)
{
	struct fixture *fixture = param;
	long long index = 0;
	const char *path = NULL;
	assert_true(calldata_get_int(data, "index", &index));
	assert_true(calldata_get_string(data, "path", &path));
	assert_non_null(path);
	assert_true(strlen(path) < sizeof(fixture->last_path));

	if (!fixture->slides) {
		fixture->first_index = (int)index;
		strcpy(fixture->first_path, path);
	}
	fixture->slides++;
	fixture->last_index = (int)index;
	strcpy(fixture->last_path, path);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(obs_startup("en-US", NULL, NULL));
	obs_register_source(&image_info);
	obs_register_source(&cut_transition);
	obs_register_source(&slideshow_info);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	obs_shutdown();
	return 0;
}

static int setup(void **state)
{
	struct fixture *fixture = bzalloc(sizeof(*fixture));
	*state = fixture;
	fixture->settings = obs_data_create();
	slideshow_info.get_defaults(fixture->settings);
	obs_data_set_string(fixture->settings, "transition", "cut");
	obs_data_set_string(fixture->settings, "use_custom_size", "16x16");
	obs_data_set_string(fixture->settings, "playback_behavior", "pause_unpause");
	obs_data_set_string(fixture->settings, "slide_mode", "mode_auto");

	obs_data_array_t *files = obs_data_array_create();
	obs_data_t *file = obs_data_create();
	obs_data_set_string(file, "value", "fixture.png");
	obs_data_array_push_back(files, file);
	obs_data_set_array(fixture->settings, "files", files);
	obs_data_release(file);
	obs_data_array_release(files);

	fixture->source = obs_source_create_private("slideshow", NULL, fixture->settings);
	assert_non_null(fixture->source);
	assert_non_null(obs_obj_get_data(fixture->source));

	/* Drive the registered update/tick callbacks synchronously, as the video
	 * thread would, without starting that thread or waiting for a frame. */
	slideshow_info.update(obs_obj_get_data(fixture->source), fixture->settings);
	obs_source_enum_active_sources(fixture->source, get_transition, fixture);
	assert_non_null(fixture->transition);
	obs_source_t *active = obs_transition_get_active_source(fixture->transition);
	assert_non_null(active);
	/* Complete the initial cut through the public transition API. */
	obs_transition_set(fixture->transition, active);
	obs_source_release(active);
	signal_handler_connect(obs_source_get_signal_handler(fixture->source), "media_ended", media_ended, fixture);
	return 0;
}

static int teardown(void **state)
{
	struct fixture *fixture = *state;
	signal_handler_disconnect(obs_source_get_signal_handler(fixture->source), "media_ended", media_ended, fixture);
	signal_handler_disconnect(obs_source_get_signal_handler(fixture->source), "media_ended", media_ended_reenter,
				  fixture);
	signal_handler_disconnect(obs_source_get_signal_handler(fixture->source), "slide_changed",
				  slide_changed_reenter, fixture);
	signal_handler_disconnect(obs_source_get_signal_handler(fixture->source), "slide_changed", slide_changed_record,
				  fixture);
	obs_source_release(fixture->transition);
	obs_source_release(fixture->source);
	obs_data_release(fixture->settings);
	obs_wait_for_destroy_queue();
	bfree(fixture);
	return 0;
}

static void clear_files(struct fixture *fixture)
{
	obs_data_array_t *files = obs_data_array_create();
	obs_data_set_array(fixture->settings, "files", files);
	obs_data_array_release(files);
	void *data = obs_obj_get_data(fixture->source);
	slideshow_info.update(data, fixture->settings);
	slideshow_info.video_tick(data, 0.0f);

	obs_source_t *active = obs_transition_get_active_source(fixture->transition);
	bool cleared = active == NULL;
	obs_source_release(active);
	assert_true(cleared);
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_ENDED);
	assert_int_equal(fixture->ended, 1);

	/* Empty ticks must not repeatedly report the end of the same list. */
	slideshow_info.video_tick(data, 0.0f);
	assert_int_equal(fixture->ended, 1);
}

static void automatic_empty_clears(void **state)
{
	clear_files(*state);
}

static void manual_empty_clears(void **state)
{
	struct fixture *fixture = *state;
	obs_data_set_string(fixture->settings, "slide_mode", "mode_manual");
	slideshow_info.update(obs_obj_get_data(fixture->source), fixture->settings);
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_PAUSED);
	clear_files(fixture);
}

static void manual_empty_callback_reenters(void **state)
{
	struct fixture *fixture = *state;
	signal_handler_connect(obs_source_get_signal_handler(fixture->source), "media_ended", media_ended_reenter,
			       fixture);
	manual_empty_clears(state);
	assert_int_equal(fixture->reentered, 1);
}

static void manual_one_image_tick_retains_child(void **state)
{
	struct fixture *fixture = *state;
	void *data = obs_obj_get_data(fixture->source);
	obs_data_set_string(fixture->settings, "slide_mode", "mode_manual");
	slideshow_info.update(data, fixture->settings);
	obs_source_t *before = obs_transition_get_active_source(fixture->transition);
	assert_non_null(before);
	/* Complete the update's cut before exercising the populated boundary. */
	obs_transition_set(fixture->transition, before);
	slideshow_info.video_tick(data, 0.0f);
	obs_source_t *after = obs_transition_get_active_source(fixture->transition);
	bool same = before == after;
	obs_source_release(after);
	obs_source_release(before);
	assert_true(same);
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_PAUSED);
	assert_int_equal(fixture->ended, 0);
}

static void paused_empty_clears(void **state)
{
	struct fixture *fixture = *state;
	slideshow_info.media_play_pause(obs_obj_get_data(fixture->source), true);
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_PAUSED);
	clear_files(fixture);
}

static void deactivated_empty_clears(void **state)
{
	struct fixture *fixture = *state;
	slideshow_info.deactivate(obs_obj_get_data(fixture->source));
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_PAUSED);
	clear_files(fixture);
}

static void update_settings(struct fixture *fixture)
{
	slideshow_info.update(obs_obj_get_data(fixture->source), fixture->settings);
}

static void empty_files(struct fixture *fixture)
{
	obs_data_array_t *files = obs_data_array_create();
	obs_data_set_array(fixture->settings, "files", files);
	obs_data_array_release(files);
}

static bool transition_cleared(struct fixture *fixture)
{
	obs_source_t *active = obs_transition_get_active_source(fixture->transition);
	bool cleared = active == NULL;
	obs_source_release(active);
	return cleared;
}

/* Manual slideshow with stop_restart: activation arms a cut restart, and the
 * next tick must still clear the empty list with the mutex released. */
static void manual_activation_empty_clears(void **state)
{
	struct fixture *fixture = *state;
	void *data = obs_obj_get_data(fixture->source);
	signal_handler_connect(obs_source_get_signal_handler(fixture->source), "media_ended", media_ended_reenter,
			       fixture);
	obs_data_set_string(fixture->settings, "slide_mode", "mode_manual");
	obs_data_set_string(fixture->settings, "playback_behavior", "stop_restart");
	empty_files(fixture);
	update_settings(fixture);
	slideshow_info.activate(data);
	slideshow_info.video_tick(data, 0.0f);

	assert_true(transition_cleared(fixture));
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_ENDED);
	assert_int_equal(fixture->ended, 1);
	assert_int_equal(fixture->reentered, 1);

	/* The activation restart is consumed; later empty ticks stay quiet. */
	slideshow_info.video_tick(data, 0.0f);
	assert_int_equal(fixture->ended, 1);
}

/* The restart-on-activate transition of a populated list signals
 * slide_changed; subscribers must be able to enumerate the source. */
static void activation_restart_reenters(void **state)
{
	struct fixture *fixture = *state;
	void *data = obs_obj_get_data(fixture->source);
	obs_data_set_string(fixture->settings, "playback_behavior", "stop_restart");
	update_settings(fixture);
	signal_handler_connect(obs_source_get_signal_handler(fixture->source), "slide_changed", slide_changed_reenter,
			       fixture);
	slideshow_info.activate(data);
	slideshow_info.video_tick(data, 0.0f);

	assert_int_equal(fixture->reentered, 1);
	assert_false(transition_cleared(fixture));
	assert_int_equal(fixture->ended, 0);
}

/* A non-looping slideshow reaching the end of its list re-transitions on the
 * tick: it either hides (media_ended) or re-announces the last slide. */
static void list_end_hide_reenters(void **state)
{
	struct fixture *fixture = *state;
	void *data = obs_obj_get_data(fixture->source);
	signal_handler_connect(obs_source_get_signal_handler(fixture->source), "media_ended", media_ended_reenter,
			       fixture);
	obs_data_set_bool(fixture->settings, "loop", false);
	obs_data_set_bool(fixture->settings, "hide", true);
	update_settings(fixture);
	slideshow_info.video_tick(data, 100.0f);

	assert_true(transition_cleared(fixture));
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_ENDED);
	assert_int_equal(fixture->ended, 1);
	assert_int_equal(fixture->reentered, 1);
}

static void list_end_keep_reenters(void **state)
{
	struct fixture *fixture = *state;
	void *data = obs_obj_get_data(fixture->source);
	signal_handler_connect(obs_source_get_signal_handler(fixture->source), "slide_changed", slide_changed_reenter,
			       fixture);
	obs_data_set_bool(fixture->settings, "loop", false);
	obs_data_set_bool(fixture->settings, "hide", false);
	update_settings(fixture);
	/* The update announces slide 0 itself; count only the tick's transition. */
	size_t before = fixture->reentered;
	slideshow_info.video_tick(data, 100.0f);

	assert_int_equal(fixture->reentered, before + 1);
	assert_false(transition_cleared(fixture));
	assert_int_equal(fixture->ended, 0);
}

/* Re-activating an empty stop_restart slideshow whose child is already cleared
 * ends it again: the restart runs before, and independently of, the empty-list
 * fade. Both orderings keep the first activation's end and drop the second. */
static void empty_reactivation_ends_again(void **state)
{
	struct fixture *fixture = *state;
	void *data = obs_obj_get_data(fixture->source);
	obs_data_set_string(fixture->settings, "slide_mode", "mode_manual");
	obs_data_set_string(fixture->settings, "playback_behavior", "stop_restart");
	empty_files(fixture);
	update_settings(fixture);

	slideshow_info.activate(data);
	slideshow_info.video_tick(data, 0.0f);
	assert_true(transition_cleared(fixture));
	assert_int_equal(fixture->ended, 1);

	slideshow_info.activate(data);
	slideshow_info.video_tick(data, 0.0f);
	assert_true(transition_cleared(fixture));
	assert_int_equal(obs_source_media_get_state(fixture->source), OBS_MEDIA_STATE_ENDED);
	assert_int_equal(fixture->ended, 2);

	/* The second restart is consumed too. */
	slideshow_info.video_tick(data, 0.0f);
	assert_int_equal(fixture->ended, 2);
}

/* The slide_changed payload is the slide that was transitioned to: randomized
 * two-file lists always move to the other slide, so the update announces
 * b.png (index 1) and the activation restart announces a.png (index 0). */
static void restart_reports_slide_payload(void **state)
{
	struct fixture *fixture = *state;
	void *data = obs_obj_get_data(fixture->source);
	signal_handler_connect(obs_source_get_signal_handler(fixture->source), "slide_changed", slide_changed_record,
			       fixture);

	obs_data_array_t *files = obs_data_array_create();
	const char *paths[] = {"a.png", "b.png"};
	for (size_t i = 0; i < 2; i++) {
		obs_data_t *file = obs_data_create();
		obs_data_set_string(file, "value", paths[i]);
		obs_data_array_push_back(files, file);
		obs_data_release(file);
	}
	obs_data_set_array(fixture->settings, "files", files);
	obs_data_array_release(files);
	obs_data_set_bool(fixture->settings, "randomize", true);
	obs_data_set_string(fixture->settings, "playback_behavior", "stop_restart");
	update_settings(fixture);

	assert_int_equal(fixture->slides, 1);
	assert_int_equal(fixture->first_index, 1);
	assert_string_equal(fixture->first_path, "b.png");

	slideshow_info.activate(data);
	slideshow_info.video_tick(data, 0.0f);

	assert_int_equal(fixture->slides, 2);
	assert_int_equal(fixture->last_index, 0);
	assert_string_equal(fixture->last_path, "a.png");
}

#define CASE_TIMEOUT_SECONDS 5

#define CASE(name) cmocka_unit_test_setup_teardown(name, setup, teardown)

static const struct CMUnitTest tests[] = {
	CASE(automatic_empty_clears),
	CASE(manual_empty_clears),
	CASE(manual_empty_callback_reenters),
	CASE(manual_one_image_tick_retains_child),
	CASE(paused_empty_clears),
	CASE(deactivated_empty_clears),
	CASE(manual_activation_empty_clears),
	CASE(activation_restart_reenters),
	CASE(list_end_hide_reenters),
	CASE(list_end_keep_reenters),
	CASE(empty_reactivation_ends_again),
	CASE(restart_reports_slide_payload),
};

#define TEST_COUNT (sizeof(tests) / sizeof(tests[0]))

#ifdef _WIN32
/* No fork on Windows: run the cases in this process. */
static int run_all(const char *only)
{
	if (only)
		cmocka_set_test_filter(only);
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
#else
/* A locking regression in the slideshow shows up as a hang. Each case runs in
 * a child process with its own libobs, bounded by an alarm, so a hang fails
 * that one case by name and the remaining cases still run (compare
 * test_group_recursion.c). */
static int run_all(const char *only)
{
	int failed = 0;

	for (size_t i = 0; i < TEST_COUNT; i++) {
		const char *name = tests[i].name;
		if (only && strcmp(only, name) != 0)
			continue;

		fflush(stdout);
		fflush(stderr);
		pid_t pid = fork();
		if (pid < 0) {
			fprintf(stderr, "%s: fork failed\n", name);
			failed++;
			continue;
		}
		if (pid == 0) {
			alarm(CASE_TIMEOUT_SECONDS);
			cmocka_set_test_filter(name);
			int ret = cmocka_run_group_tests(tests, group_setup, group_teardown);
			fflush(stdout);
			fflush(stderr);
			_exit(ret ? 1 : 0);
		}

		int status = 0;
		if (waitpid(pid, &status, 0) != pid) {
			fprintf(stderr, "[  FAILED  ] %s: waitpid failed\n", name);
			failed++;
		} else if (WIFSIGNALED(status)) {
			fprintf(stderr, "[  FAILED  ] %s: the case died from signal %d (%s)\n", name, WTERMSIG(status),
				WTERMSIG(status) == SIGALRM ? "hang" : "crash");
			failed++;
		} else if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
			failed++;
		}
	}

	fprintf(stderr, "%d of %zu case(s) failed\n", failed, TEST_COUNT);
	return failed ? 1 : 0;
}
#endif

int main(int argc, char **argv)
{
	/* An optional case name runs just that case. */
	return run_all(argc > 1 ? argv[1] : NULL);
}
