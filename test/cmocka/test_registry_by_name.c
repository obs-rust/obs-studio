/* Direct characterization of the three public registry name getters. No media
 * pipeline or external plugin is needed: fallback output/encoder contexts and
 * a registered synthetic service use real registries and destruction paths. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

static const char *service_name(void *data)
{
	(void)data;
	return "Registry service";
}

static void *service_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	return service;
}

static void service_destroy(void *data)
{
	(void)data;
}

static const char *service_protocol(void *data)
{
	(void)data;
	return "RTMP";
}

static int setup(void **state)
{
	(void)state;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	struct obs_service_info info = {
		.id = "registry-service",
		.get_name = service_name,
		.create = service_create,
		.destroy = service_destroy,
		.get_protocol = service_protocol,
	};
	obs_register_service(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void output_lookup(void **state)
{
	(void)state;
	assert_null(obs_get_output_by_name("registry name"));
	assert_null(obs_get_output_by_name(""));
	obs_output_t *older = obs_output_create("registry-missing-output", "registry name", NULL, NULL);
	obs_output_t *newer = obs_output_create("registry-missing-output", "registry name", NULL, NULL);
	assert_non_null(older);
	assert_non_null(newer);
	assert_null(obs_get_output_by_name("Registry name"));
	assert_null(obs_get_output_by_name("registry"));
	assert_null(obs_get_output_by_name(""));
	/* Duplicate names return the newest list entry: characterized, not endorsed. */
	obs_output_t *found = obs_get_output_by_name("registry name");
	assert_ptr_equal(found, newer);
	obs_weak_output_t *weak = obs_output_get_weak_output(newer);
	obs_output_release(newer);
	obs_output_t *promoted = obs_weak_output_get_output(weak);
	assert_ptr_equal(promoted, found);
	obs_output_release(promoted);
	obs_output_t *again = obs_get_output_by_name("registry name");
	assert_ptr_equal(again, found);
	obs_output_release(again);
	obs_output_release(found);
	assert_null(obs_weak_output_get_output(weak));
	obs_weak_output_release(weak);
	found = obs_get_output_by_name("registry name");
	assert_ptr_equal(found, older);
	obs_output_release(found);
	obs_output_release(older);
	assert_null(obs_get_output_by_name("registry name"));
}

static void encoder_lookup(void **state)
{
	(void)state;
	assert_null(obs_get_encoder_by_name("encoder name"));
	assert_null(obs_get_encoder_by_name(""));
	obs_encoder_t *older = obs_video_encoder_create("registry-missing-encoder", "encoder name", NULL, NULL);
	obs_encoder_t *newer = obs_video_encoder_create("registry-missing-encoder", "encoder name", NULL, NULL);
	assert_non_null(older);
	assert_non_null(newer);
	assert_null(obs_get_encoder_by_name("Encoder name"));
	assert_null(obs_get_encoder_by_name("encoder"));
	assert_null(obs_get_encoder_by_name(""));
	obs_encoder_t *found = obs_get_encoder_by_name("encoder name");
	assert_ptr_equal(found, newer);
	obs_weak_encoder_t *weak = obs_encoder_get_weak_encoder(newer);
	obs_encoder_release(newer);
	obs_encoder_t *promoted = obs_weak_encoder_get_encoder(weak);
	assert_ptr_equal(promoted, found);
	obs_encoder_release(promoted);
	obs_encoder_set_name(found, "renamed encoder");
	obs_encoder_t *again = obs_get_encoder_by_name("renamed encoder");
	assert_ptr_equal(again, found);
	obs_encoder_release(again);
	obs_encoder_release(found);
	assert_null(obs_weak_encoder_get_encoder(weak));
	obs_weak_encoder_release(weak);
	assert_null(obs_get_encoder_by_name("renamed encoder"));
	found = obs_get_encoder_by_name("encoder name");
	assert_ptr_equal(found, older);
	obs_encoder_release(found);
	obs_encoder_release(older);
	assert_null(obs_get_encoder_by_name("encoder name"));
}

static void service_lookup(void **state)
{
	(void)state;
	assert_null(obs_get_service_by_name("service name"));
	assert_null(obs_get_service_by_name(""));
	obs_service_t *older = obs_service_create("registry-service", "service name", NULL, NULL);
	obs_service_t *newer = obs_service_create("registry-service", "service name", NULL, NULL);
	obs_service_t *private = obs_service_create_private("registry-service", "service name", NULL);
	assert_non_null(older);
	assert_non_null(newer);
	assert_non_null(private);
	assert_null(obs_get_service_by_name("Service name"));
	assert_null(obs_get_service_by_name("service"));
	assert_null(obs_get_service_by_name(""));
	obs_service_t *found = obs_get_service_by_name("service name");
	assert_ptr_equal(found, newer);
	obs_weak_service_t *weak = obs_service_get_weak_service(newer);
	obs_service_release(newer);
	obs_service_t *promoted = obs_weak_service_get_service(weak);
	assert_ptr_equal(promoted, found);
	obs_service_release(promoted);
	obs_service_t *again = obs_get_service_by_name("service name");
	assert_ptr_equal(again, found);
	obs_service_release(again);
	obs_service_release(found);
	assert_null(obs_weak_service_get_service(weak));
	obs_weak_service_release(weak);
	found = obs_get_service_by_name("service name");
	assert_ptr_equal(found, older);
	obs_service_release(found);
	obs_service_release(older);
	/* The remaining private entry is skipped, even with an exact name match. */
	assert_null(obs_get_service_by_name("service name"));
	obs_service_release(private);
}

static void separate_registries(void **state)
{
	(void)state;
	obs_output_t *output = obs_output_create("registry-missing-output", "shared name", NULL, NULL);
	obs_encoder_t *encoder = obs_video_encoder_create("registry-missing-encoder", "shared name", NULL, NULL);
	obs_service_t *service = obs_service_create("registry-service", "shared name", NULL, NULL);
	assert_non_null(output);
	assert_non_null(encoder);
	assert_non_null(service);
	obs_output_t *found_output = obs_get_output_by_name("shared name");
	obs_encoder_t *found_encoder = obs_get_encoder_by_name("shared name");
	obs_service_t *found_service = obs_get_service_by_name("shared name");
	assert_ptr_equal(found_output, output);
	assert_ptr_equal(found_encoder, encoder);
	assert_ptr_equal(found_service, service);
	obs_output_release(found_output);
	obs_encoder_release(found_encoder);
	obs_service_release(found_service);
	obs_output_release(output);
	obs_encoder_release(encoder);
	obs_service_release(service);
	assert_null(obs_get_output_by_name("shared name"));
	assert_null(obs_get_encoder_by_name("shared name"));
	assert_null(obs_get_service_by_name("shared name"));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(output_lookup),
		cmocka_unit_test(encoder_lookup),
		cmocka_unit_test(service_lookup),
		cmocka_unit_test(separate_registries),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
