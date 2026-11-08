/* Actual-library characterization; canvas must be non-NULL (forming
 * &canvas->view on NULL is undefined, characterized not endorsed). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "obs-internal.h"

static void retained_channel(void **state)
{
	(void)state;
	struct obs_canvas canvas = {0};
	struct obs_source source = {0};
	struct obs_weak_source control = {0};
	source.context.control = (struct obs_weak_object *)&control;
	control.source = &source;
	assert_int_equal(pthread_mutex_init(&canvas.view.channels_mutex, NULL), 0);
	for (uint32_t channel = 0; channel < MAX_CHANNELS; channel++) {
		assert_null(obs_canvas_get_channel(&canvas, channel));
		canvas.view.channels[channel] = &source;
		assert_ptr_equal(obs_canvas_get_channel(&canvas, channel), &source);
		assert_int_equal(control.ref.refs, 1);
		assert_ptr_equal(obs_canvas_get_channel(&canvas, channel), &source);
		assert_int_equal(control.ref.refs, 2);
		/* Drop only the two acquired references; retain the fixture's owner. */
		assert_false(obs_ref_release(&control.ref));
		assert_false(obs_ref_release(&control.ref));
		assert_int_equal(control.ref.refs, 0);
		assert_ptr_equal(canvas.view.channels[channel], &source);
		control.ref.refs = -1;
		assert_null(obs_canvas_get_channel(&canvas, channel));
		assert_int_equal(control.ref.refs, -1);
		control.ref.refs = 0;
		canvas.view.channels[channel] = NULL;
	}
#ifdef NDEBUG
	/* Debug C asserts; release returns NULL before locking or reading a channel. */
	assert_null(obs_canvas_get_channel(&canvas, MAX_CHANNELS));
	assert_null(obs_canvas_get_channel(&canvas, UINT32_MAX));
#endif
	/* The view's channels mutex is released before returning. */
	assert_int_equal(pthread_mutex_trylock(&canvas.view.channels_mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&canvas.view.channels_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&canvas.view.channels_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(retained_channel)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
