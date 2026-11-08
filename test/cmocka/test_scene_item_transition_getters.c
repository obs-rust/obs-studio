/* Characterize the public getters against the actual linked libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "obs-scene.h"

static void transition_getters(void **state)
{
	(void)state;
	struct obs_source show_source = {0}, hide_source = {0};
	const uint32_t values[] = {0, 1, 250, UINT32_MAX};
	for (unsigned show = 0; show < 2; show++) {
		assert_null(obs_sceneitem_get_transition(NULL, show != 0));
		assert_int_equal(obs_sceneitem_get_transition_duration(NULL, show != 0), 0);
	}
	for (unsigned mask = 0; mask < 4; mask++) {
		for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
			struct obs_scene_item item = {0}, before;
			item.show_transition = (mask & 1) ? &show_source : NULL;
			item.hide_transition = (mask & 2) ? &hide_source : NULL;
			item.show_transition_duration = values[i];
			item.hide_transition_duration = values[3 - i];
			item.id = 901;
			memcpy(&before, &item, sizeof(item));
			for (unsigned repeat = 0; repeat < 2; repeat++) {
				assert_ptr_equal(obs_sceneitem_get_transition(&item, true), item.show_transition);
				assert_ptr_equal(obs_sceneitem_get_transition(&item, false), item.hide_transition);
				assert_int_equal(obs_sceneitem_get_transition_duration(&item, true),
						 item.show_transition_duration);
				assert_int_equal(obs_sceneitem_get_transition_duration(&item, false),
						 item.hide_transition_duration);
				assert_memory_equal(&item, &before, sizeof(item));
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(transition_getters)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
