/* Public-API characterization before extracting single-hotkey registration.
 * No private layouts or replacement registration implementations are used. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>
#include <string.h>

static const char *type_name(void *unused)
{
	(void)unused;
	return "registration fixture";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	return encoder;
}

static void *service_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	return service;
}

static void type_destroy(void *data)
{
	(void)data;
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static bool encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received;
	return false;
}

static const char *service_protocol(void *data)
{
	(void)data;
	return "RTMP";
}

static void register_types(void)
{
	struct obs_source_info source = {
		.id = "register_input",
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = type_name,
		.create = source_create,
		.destroy = type_destroy,
	};
	struct obs_output_info output = {
		.id = "register_output",
		.flags = OBS_OUTPUT_ENCODED,
		.get_name = type_name,
		.create = output_create,
		.destroy = type_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	struct obs_encoder_info encoder = {
		.id = "register_encoder",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = type_name,
		.create = encoder_create,
		.destroy = type_destroy,
		.encode = encoder_encode,
	};
	struct obs_service_info service = {
		.id = "register_service",
		.get_name = type_name,
		.create = service_create,
		.destroy = type_destroy,
		.get_protocol = service_protocol,
	};
	obs_register_source(&source);
	obs_register_output(&output);
	obs_register_encoder(&encoder);
	obs_register_service(&service);
}

struct observation {
	obs_hotkey_id id;
	obs_hotkey_t *key;
	size_t calls;
	bool pressed;
};

static void callback(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	struct observation *seen = data;
	seen->id = id;
	seen->key = key;
	seen->pressed = pressed;
	seen->calls++;
}

static void unavailable_core(void **state)
{
	(void)state;
	assert_int_equal(obs_hotkey_register_frontend("absent", "absent", callback, NULL), OBS_INVALID_HOTKEY_ID);
	assert_int_equal(obs_hotkey_register_source(NULL, "absent", "absent", callback, NULL), OBS_INVALID_HOTKEY_ID);
	assert_int_equal(obs_hotkey_register_output(NULL, "absent", "absent", callback, NULL), OBS_INVALID_HOTKEY_ID);
	assert_int_equal(obs_hotkey_register_encoder(NULL, "absent", "absent", callback, NULL), OBS_INVALID_HOTKEY_ID);
	assert_int_equal(obs_hotkey_register_service(NULL, "absent", "absent", callback, NULL), OBS_INVALID_HOTKEY_ID);
}

struct lookup {
	obs_hotkey_id id;
	obs_hotkey_t *key;
};

static bool find_key(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct lookup *lookup = data;
	if (id == lookup->id)
		lookup->key = key;
	return true;
}

static obs_hotkey_t *lookup_key(obs_hotkey_id id)
{
	struct lookup lookup = {.id = id};
	obs_enum_hotkeys(find_key, &lookup);
	return lookup.key;
}

static void check_key(obs_hotkey_id id, obs_hotkey_registerer_t type, void *weak, struct observation *seen)
{
	assert_true(id != OBS_INVALID_HOTKEY_ID);
	obs_hotkey_t *key = lookup_key(id);
	assert_non_null(key);
	assert_int_equal(obs_hotkey_get_id(key), id);
	assert_string_equal(obs_hotkey_get_name(key), "registration.name");
	assert_string_equal(obs_hotkey_get_description(key), "registration description");
	assert_int_equal(obs_hotkey_get_registerer_type(key), type);
	assert_ptr_equal(obs_hotkey_get_registerer(key), weak);
	assert_int_equal(obs_hotkey_get_pair_partner_id(key), OBS_INVALID_HOTKEY_PAIR_ID);
	obs_hotkey_enable_callback_rerouting(true);
	obs_hotkey_trigger_routed_callback(id, true);
	assert_int_equal(seen->calls, 1);
	assert_int_equal(seen->id, id);
	assert_ptr_equal(seen->key, key);
	assert_true(seen->pressed);
	obs_hotkey_trigger_routed_callback(id, false);
	assert_int_equal(seen->calls, 2);
	assert_false(seen->pressed);
	obs_hotkey_enable_callback_rerouting(false);
}

static void no_task(void *data)
{
	(void)data;
}

static void frontend_and_admission(void **state)
{
	(void)state;
	struct observation seen = {0};
	obs_hotkey_id id =
		obs_hotkey_register_frontend("registration.name", "registration description", callback, &seen);
	check_key(id, OBS_HOTKEY_REGISTERER_FRONTEND, NULL, &seen);
	assert_int_equal(obs_hotkey_register_source(NULL, "rejected", "rejected", callback, NULL),
			 OBS_INVALID_HOTKEY_ID);
	assert_int_equal(obs_hotkey_register_output(NULL, "rejected", "rejected", callback, NULL),
			 OBS_INVALID_HOTKEY_ID);
	assert_int_equal(obs_hotkey_register_encoder(NULL, "rejected", "rejected", callback, NULL),
			 OBS_INVALID_HOTKEY_ID);
	assert_int_equal(obs_hotkey_register_service(NULL, "rejected", "rejected", callback, NULL),
			 OBS_INVALID_HOTKEY_ID);
	obs_source_t *private_source = obs_source_create_private("register_input", "private", NULL);
	assert_non_null(private_source);
	assert_int_equal(obs_hotkey_register_source(private_source, "rejected", "rejected", callback, NULL),
			 OBS_INVALID_HOTKEY_ID);
	obs_source_release(private_source);
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	/* Rejected calls do not consume IDs. Identical names remain legal. */
	obs_hotkey_id second =
		obs_hotkey_register_frontend("registration.name", "registration description", callback, &seen);
	assert_int_equal(second, id + 1);
	assert_non_null(lookup_key(second));
	obs_hotkey_unregister(id);
	obs_hotkey_unregister(second);
	assert_null(lookup_key(id));
	assert_null(lookup_key(second));
}

static void check_bindings(obs_data_t *saved, obs_hotkey_id id)
{
	assert_non_null(saved);
	obs_data_array_t *array = obs_data_get_array(saved, "registration.name");
	assert_non_null(array);
	assert_int_equal(obs_data_array_count(array), 1);
	obs_data_array_t *direct = obs_hotkey_save(id);
	assert_int_equal(obs_data_array_count(direct), 1);
	obs_data_t *binding = obs_data_array_item(direct, 0);
	assert_string_equal(obs_data_get_string(binding, "key"), "OBS_KEY_A");
	assert_true(obs_data_get_bool(binding, "control"));
	obs_data_release(binding);
	obs_data_array_release(direct);
	obs_data_array_release(array);
	obs_data_release(saved);
}

static void owned_registration(void **state)
{
	(void)state;
	obs_data_t *hotkeys =
		obs_data_create_from_json("{\"registration.name\":[{\"key\":\"OBS_KEY_A\",\"control\":true}]}");
	assert_non_null(hotkeys);
	obs_source_t *source = obs_source_create("register_input", "source", NULL, hotkeys);
	obs_output_t *output = obs_output_create("register_output", "output", NULL, hotkeys);
	obs_encoder_t *encoder = obs_video_encoder_create("register_encoder", "encoder", NULL, hotkeys);
	obs_service_t *service = obs_service_create("register_service", "service", NULL, hotkeys);
	obs_data_release(hotkeys);
	assert_non_null(source);
	assert_non_null(output);
	assert_non_null(encoder);
	assert_non_null(service);
	obs_weak_source_t *ws = obs_source_get_weak_source(source);
	obs_weak_output_t *wo = obs_output_get_weak_output(output);
	obs_weak_encoder_t *we = obs_encoder_get_weak_encoder(encoder);
	obs_weak_service_t *wv = obs_service_get_weak_service(service);
	struct observation seen[4] = {0};
	obs_hotkey_id ids[] = {
		obs_hotkey_register_source(source, "registration.name", "registration description", callback, &seen[0]),
		obs_hotkey_register_output(output, "registration.name", "registration description", callback, &seen[1]),
		obs_hotkey_register_encoder(encoder, "registration.name", "registration description", callback,
					    &seen[2]),
		obs_hotkey_register_service(service, "registration.name", "registration description", callback,
					    &seen[3]),
	};
	check_key(ids[0], OBS_HOTKEY_REGISTERER_SOURCE, ws, &seen[0]);
	check_key(ids[1], OBS_HOTKEY_REGISTERER_OUTPUT, wo, &seen[1]);
	check_key(ids[2], OBS_HOTKEY_REGISTERER_ENCODER, we, &seen[2]);
	check_key(ids[3], OBS_HOTKEY_REGISTERER_SERVICE, wv, &seen[3]);
	for (size_t i = 0; i < 4; i++)
		for (size_t j = i + 1; j < 4; j++)
			assert_true(ids[i] != ids[j]);
	check_bindings(obs_hotkeys_save_source(source), ids[0]);
	check_bindings(obs_hotkeys_save_output(output), ids[1]);
	check_bindings(obs_hotkeys_save_encoder(encoder), ids[2]);
	check_bindings(obs_hotkeys_save_service(service), ids[3]);
	obs_source_release(source);
	obs_output_release(output);
	obs_encoder_release(encoder);
	obs_service_release(service);
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	for (size_t i = 0; i < 4; i++)
		assert_null(lookup_key(ids[i]));
	assert_null(obs_weak_source_get_source(ws));
	assert_null(obs_weak_output_get_output(wo));
	assert_null(obs_weak_encoder_get_encoder(we));
	assert_null(obs_weak_service_get_service(wv));
	obs_weak_source_release(ws);
	obs_weak_output_release(wo);
	obs_weak_encoder_release(we);
	obs_weak_service_release(wv);
}

int main(void)
{
	const struct CMUnitTest absent[] = {cmocka_unit_test(unavailable_core)};
	int rc = cmocka_run_group_tests(absent, NULL, NULL);
	if (rc)
		return rc;
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	register_types();
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(frontend_and_admission),
		cmocka_unit_test(owned_registration),
	};
	rc = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return rc;
}
