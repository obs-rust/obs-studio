/* Characterize public context queries against the actual linked libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void object_queries(void **state)
{
	(void)state;
	assert_int_equal(obs_obj_get_type(NULL), OBS_OBJ_TYPE_INVALID);
	assert_false(obs_obj_is_private(NULL));
	for (int type = OBS_OBJ_TYPE_INVALID; type <= OBS_OBJ_TYPE_CANVAS; type++) {
		for (unsigned privacy = 0; privacy < 2; privacy++) {
			struct obs_context_data context = {0};
			struct obs_context_data before;
			context.type = (enum obs_obj_type)type;
			context.private = privacy != 0;
			context.name = "object query sentinel";
			context.data = &before;
			memcpy(&before, &context, sizeof(context));
			assert_int_equal(obs_obj_get_type(&context), type);
			assert_int_equal(obs_obj_is_private(&context), privacy);
			assert_int_equal(obs_obj_get_type(&context), type);
			assert_int_equal(obs_obj_is_private(&context), privacy);
			assert_memory_equal(&context, &before, sizeof(context));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(object_queries)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
