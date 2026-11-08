/* Characterize the public exports, using the real main canvas and private mix layout. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

/* These deprecated exports remain part of the compatibility contract. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

static void texture_activity(void **state)
{
	(void)state;
	assert_false(obs_nv12_tex_active());
	assert_false(obs_p010_tex_active());
	assert_true(obs_startup("en-US", NULL, NULL));
	obs_canvas_t *canvas = obs_get_main_canvas();
	assert_non_null(canvas);
	struct obs_core_video_mix *saved = canvas->mix;
	assert_null(saved);
	assert_false(obs_nv12_tex_active());
	assert_false(obs_p010_tex_active());
	struct obs_core_video_mix mix = {0}, before;
	canvas->mix = &mix;
	for (unsigned n = 0; n < 2; n++) {
		for (unsigned p = 0; p < 2; p++) {
			mix.using_nv12_tex = n != 0;
			mix.using_p010_tex = p != 0;
			memcpy(&before, &mix, sizeof(mix));
			assert_int_equal(obs_nv12_tex_active(), n);
			assert_int_equal(obs_p010_tex_active(), p);
			assert_int_equal(obs_nv12_tex_active(), n);
			assert_int_equal(obs_p010_tex_active(), p);
			assert_memory_equal(&mix, &before, sizeof(mix));
			assert_ptr_equal(canvas->mix, &mix);
		}
	}
	canvas->mix = saved;
	obs_canvas_release(canvas);
	obs_shutdown();
	assert_false(obs_nv12_tex_active());
	assert_false(obs_p010_tex_active());
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(texture_activity)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
