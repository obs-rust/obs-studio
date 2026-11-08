/* Characterize public context validity query against the actual linked libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static void object_validity_query(void **state)
{
	(void)state;
	assert_true(obs_obj_invalid(NULL));
	int payloads[] = {0, 42};
	void *values[] = {NULL, &payloads[0], &payloads[1]};
	for (int type = OBS_OBJ_TYPE_INVALID; type <= OBS_OBJ_TYPE_CANVAS; type++) {
		for (size_t i = 0; i < 3; i++) {
			struct obs_context_data context = {0};
			struct obs_context_data before;
			context.type = (enum obs_obj_type)type;
			context.private = true;
			context.name = "object data sentinel";
			context.data = values[i];
			memcpy(&before, &context, sizeof(context));
			assert_int_equal(obs_obj_invalid(&context), values[i] == NULL);
			assert_int_equal(obs_obj_invalid(&context), values[i] == NULL);
			assert_memory_equal(&context, &before, sizeof(context));
		}
	}
	assert_int_equal(payloads[0], 0);
	assert_int_equal(payloads[1], 42);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(object_validity_query)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
