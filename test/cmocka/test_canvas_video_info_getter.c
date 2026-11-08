/* Characterize the live getter before extraction or the Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>
#include <cmocka.h>
#include "obs-internal.h"

static void video_info(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_canvas canvas = {0};
	struct obs_core_video_mix mix = {0};
	struct obs_video_info output, before;
	void *token = malloc(1);
	assert_non_null(token);
	memset(&output, 0xa5, sizeof(output));
	memcpy(&before, &output, sizeof(output));
	assert_false(obs_canvas_get_video_info(NULL, &output));
	assert_false(obs_canvas_get_video_info(NULL, NULL));
	assert_memory_equal(&output, &before, sizeof(output));
	canvas.mix = &mix;
	assert_false(obs_canvas_get_video_info(&canvas, &output));
	assert_memory_equal(&output, &before, sizeof(output));
	struct obs_video_info setup = {.graphics_module = GRAPHICS_MODULE_PATH,
				       .fps_num = 30,
				       .fps_den = 1,
				       .base_width = 32,
				       .base_height = 32,
				       .output_width = 32,
				       .output_height = 32,
				       .output_format = VIDEO_FORMAT_NV12,
				       .colorspace = VIDEO_CS_709,
				       .range = VIDEO_RANGE_PARTIAL,
				       .scale_type = OBS_SCALE_BICUBIC};
	assert_int_equal(obs_reset_video(&setup), OBS_VIDEO_SUCCESS);
	/* Probe the non-NULL precondition in a child, after graphics exists. */
	pid_t child = fork();
	assert_true(child >= 0);
	if (child == 0) {
		struct rlimit limit = {0, 0};
		setrlimit(RLIMIT_CORE, &limit);
		signal(SIGSEGV, SIG_DFL);
		obs_canvas_get_video_info(NULL, &output);
		_exit(0);
	}
	int status;
	assert_int_equal(waitpid(child, &status, 0), child);
	assert_true(WIFSIGNALED(status));
	assert_int_equal(WTERMSIG(status), SIGSEGV);
	canvas.mix = NULL;
	assert_false(obs_canvas_get_video_info(&canvas, NULL));
	assert_false(obs_canvas_get_video_info(&canvas, &output));
	assert_memory_equal(&output, &before, sizeof(output));
	canvas.mix = &mix;
	for (unsigned value = 0; value < 256; value++) {
		memset(&mix.ovi, value, sizeof(mix.ovi));
		mix.ovi.graphics_module = token;
		mix.ovi.gpu_conversion = value & 1;
		assert_true(obs_canvas_get_video_info(&canvas, &output));
		assert_memory_equal(&output, &mix.ovi, sizeof(output));
		assert_ptr_equal(output.graphics_module, token);
		assert_ptr_equal(canvas.mix, &mix);
	}
	/* Characterized, not endorsed: with graphics present, canvas must be
	 * non-NULL; a present mix also requires a writable non-NULL output.
	 * A NULL mix->video does not prevent a successful info query. */
	obs_shutdown();
	free(token);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(video_info)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
