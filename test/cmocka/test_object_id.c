/* Characterize obs_obj_get_id() before its later Rust swap (Refs #5088). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-object-id.c.h")
#error "generic object ID query extraction seam is missing"
#endif

/* Static storage: the real-header objects are large and must start zeroed. */
static struct obs_source source, source_before;
static struct obs_output output, output_before;
static struct obs_encoder encoder, encoder_before;
static struct obs_service service, service_before;

static const char empty_id[] = "";

static void null_object(void **state)
{
	(void)state;
	assert_null(obs_obj_get_id(NULL));
}

static void unknown_kinds(void **state)
{
	(void)state;
	const int kinds[] = {OBS_OBJ_TYPE_INVALID, OBS_OBJ_TYPE_CANVAS, OBS_OBJ_TYPE_CANVAS + 1, INT32_MAX};
	for (size_t i = 0; i < sizeof(kinds) / sizeof(kinds[0]); i++) {
		/* Exactly one context: any type-specific read is out of bounds under ASan. */
		struct obs_context_data *context = malloc(sizeof(*context));
		assert_non_null(context);
		memset(context, 0, sizeof(*context));
		context->type = (enum obs_obj_type)kinds[i];
		context->name = "unknown display name";
		struct obs_context_data before;
		memcpy(&before, context, sizeof(before));
		assert_null(obs_obj_get_id(context));
		assert_null(obs_obj_get_id(context));
		assert_memory_equal(context, &before, sizeof(before));
		free(context);
	}
}

/* The display name and the original encoder id are distinct strings, so a
 * read of either instead of info.id fails the pointer comparison. */
static const char *const stored_ids[] = {"concrete_id", empty_id, NULL};

static void source_id(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(stored_ids) / sizeof(stored_ids[0]); i++) {
		memset(&source, 0, sizeof(source));
		source.context.type = OBS_OBJ_TYPE_SOURCE;
		source.context.name = "source display name";
		source.info.id = stored_ids[i];
		memcpy(&source_before, &source, sizeof(source));
		assert_ptr_equal(obs_obj_get_id(&source), stored_ids[i]);
		assert_ptr_equal(obs_obj_get_id(&source), stored_ids[i]);
		assert_memory_equal(&source, &source_before, sizeof(source));
	}
}

static void output_id(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(stored_ids) / sizeof(stored_ids[0]); i++) {
		memset(&output, 0, sizeof(output));
		output.context.type = OBS_OBJ_TYPE_OUTPUT;
		output.context.name = "output display name";
		output.info.id = stored_ids[i];
		memcpy(&output_before, &output, sizeof(output));
		assert_ptr_equal(obs_obj_get_id(&output), stored_ids[i]);
		assert_ptr_equal(obs_obj_get_id(&output), stored_ids[i]);
		assert_memory_equal(&output, &output_before, sizeof(output));
	}
}

static void encoder_id(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(stored_ids) / sizeof(stored_ids[0]); i++) {
		memset(&encoder, 0, sizeof(encoder));
		encoder.context.type = OBS_OBJ_TYPE_ENCODER;
		encoder.context.name = "encoder display name";
		encoder.info.id = stored_ids[i];
		encoder.orig_info.id = "original_encoder_id";
		memcpy(&encoder_before, &encoder, sizeof(encoder));
		assert_ptr_equal(obs_obj_get_id(&encoder), stored_ids[i]);
		assert_ptr_equal(obs_obj_get_id(&encoder), stored_ids[i]);
		assert_memory_equal(&encoder, &encoder_before, sizeof(encoder));
	}
}

static void service_id(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(stored_ids) / sizeof(stored_ids[0]); i++) {
		memset(&service, 0, sizeof(service));
		service.context.type = OBS_OBJ_TYPE_SERVICE;
		service.context.name = "service display name";
		service.info.id = stored_ids[i];
		memcpy(&service_before, &service, sizeof(service));
		assert_ptr_equal(obs_obj_get_id(&service), stored_ids[i]);
		assert_ptr_equal(obs_obj_get_id(&service), stored_ids[i]);
		assert_memory_equal(&service, &service_before, sizeof(service));
	}
}

int main(void)
{
	/* No startup, graphics, audio, threads or capture hardware. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_object), cmocka_unit_test(unknown_kinds), cmocka_unit_test(source_id),
		cmocka_unit_test(output_id),   cmocka_unit_test(encoder_id),    cmocka_unit_test(service_id),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
