/* Regression test for obs-rust/obs-studio#2210: gs_effect_set_default() in
 * libobs/graphics/effect.c.h read param->default_val before anything
 * checked param for NULL, so a plugin or script that passed the result of a
 * failed gs_effect_get_param_by_name() crashed there, while every other
 * gs_effect_set_*() setter logs "invalid param" and returns.
 *
 * With graphics up, gs_effect_set_default(NULL) must return and log one
 * error. It runs in a forked child, so a crash fails only its own case.
 * Windows has no fork(), so there the call runs in the test process.
 *
 * gs_effect_set_default() on a real parameter (solid.effect's color, whose
 * default is {1, 1, 1, 1}) must still put the default back after another
 * value was set, and a parameter with no default keeps its value.
 *
 * obs_startup() and obs_reset_video() need a display on Linux, so the test
 * runs under xvfb-run there. It skips only when no display is set at all;
 * with one set, a failed startup fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef _WIN32
#include <errno.h>
#include <signal.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif
#ifdef __linux__
#include <sys/prctl.h>
#endif

#include <obs.h>
#include <graphics/vec4.h>
#include <util/base.h>
#include <util/bmem.h>

static bool obs_up;

/* "invalid param" errors; everything is passed on to the previous
 * handler. */
static log_handler_t prev_handler;
static void *prev_param;
static int param_errors;

static void log_handler(int lvl, const char *format, va_list args, void *param)
{
	(void)param;
	if (lvl == LOG_ERROR && strstr(format, "invalid param"))
		param_errors++;
	if (prev_handler)
		prev_handler(lvl, format, args, prev_param);
}

/* Calls gs_effect_set_default(NULL) and returns how many "invalid param"
 * errors it logged. The caller has entered the graphics context. */
static int set_default_null(void)
{
	log_handler_t handler;
	void *handler_param;
	base_get_log_handler(&handler, &handler_param);
	prev_handler = handler;
	prev_param = handler_param;
	param_errors = 0;
	base_set_log_handler(log_handler, NULL);

	gs_effect_set_default(NULL);

	base_set_log_handler(handler, handler_param);
	return param_errors;
}

static void set_default_null_returns(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	int errors;
#ifdef _WIN32
	obs_enter_graphics();
	errors = set_default_null();
	obs_leave_graphics();
#else
	int fds[2];
	assert_int_equal(pipe(fds), 0);
	fflush(stdout);
	fflush(stderr);
	/* The child inherits the entered context. Entering it in the child
	 * could wait forever on a lock the video thread held at the fork. */
	obs_enter_graphics();
	pid_t pid = fork();
	if (pid != 0)
		obs_leave_graphics();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka traps SIGSEGV and would resume the test runner in the
		 * child; let a crash kill the child instead. */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGILL, SIG_DFL);
		signal(SIGABRT, SIG_DFL);
		signal(SIGALRM, SIG_DFL);
#ifdef __linux__
		/* an expected crash should not leave a core dump behind */
		prctl(PR_SET_DUMPABLE, 0);
#endif
		alarm(30);
		close(fds[0]);
		int child = set_default_null();
		ssize_t n = write(fds[1], &child, sizeof(child));
		_exit(n == (ssize_t)sizeof(child) ? 0 : 1);
	}
	close(fds[1]);
	size_t got = 0;
	while (got < sizeof(errors)) {
		ssize_t n = read(fds[0], (char *)&errors + got, sizeof(errors) - got);
		if (n < 0 && errno == EINTR)
			continue;
		if (n <= 0)
			break;
		got += (size_t)n;
	}
	close(fds[0]);

	int status = 0;
	while (waitpid(pid, &status, 0) < 0)
		assert_int_equal(errno, EINTR);
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "gs_effect_set_default(NULL) died with signal %d\n", WTERMSIG(status));
		fail();
	}
	assert_true(WIFEXITED(status));
	assert_int_equal(WEXITSTATUS(status), 0);
	assert_int_equal(got, sizeof(errors));
#endif

	if (errors != 1)
		fprintf(stderr, "gs_effect_set_default(NULL) logged %d invalid param errors\n", errors);
	assert_int_equal(errors, 1);
}

static bool param_is(gs_eparam_t *param, const struct vec4 *want)
{
	if (gs_effect_get_val_size(param) != sizeof(*want))
		return false;
	void *val = gs_effect_get_val(param);
	bool same = val && memcmp(val, want, sizeof(*want)) == 0;
	bfree(val);
	return same;
}

static void set_default_restores_default(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_enter_graphics();
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	assert_non_null(solid);

	gs_eparam_t *color = gs_effect_get_param_by_name(solid, "color");
	assert_non_null(color);
	struct vec4 white;
	vec4_set(&white, 1.0f, 1.0f, 1.0f, 1.0f);
	assert_int_equal(gs_effect_get_default_val_size(color), sizeof(white));

	struct vec4 grey;
	vec4_set(&grey, 0.25f, 0.5f, 0.75f, 1.0f);
	gs_effect_set_vec4(color, &grey);
	assert_true(param_is(color, &grey));
	gs_effect_set_default(color);
	assert_true(param_is(color, &white));

	/* randomvals1 has no default: its value stays. */
	gs_eparam_t *random = gs_effect_get_param_by_name(solid, "randomvals1");
	assert_non_null(random);
	assert_int_equal(gs_effect_get_default_val_size(random), 0);
	gs_effect_set_vec4(random, &grey);
	gs_effect_set_default(random);
	assert_true(param_is(random, &grey));
	obs_leave_graphics();
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		obs_up = false;
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_reset_video failed with a display set: %d\n", ret);
			return -1;
		}
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(set_default_null_returns),
		cmocka_unit_test(set_default_restores_default),
	};
	return cmocka_run_group_tests_name("test_effect_set_default_null", tests, group_setup, group_teardown);
}
