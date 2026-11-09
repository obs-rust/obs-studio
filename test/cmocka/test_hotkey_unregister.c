/* Public-API characterization before extracting hotkey removal (#4426).
 * The unregister signal sees a live key after hash removal but before owner and
 * pressed-binding release. Characterized, not endorsed: repeated IDs are no-ops.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>

#define INPUT_ID "unregister_input"
#define OUTPUT_ID "unregister_output"
#define ENCODER_ID "unregister_encoder"
#define SERVICE_ID "unregister_service"

static const char *type_name(void *type_data)
{
	(void)type_data;
	return "hotkey context";
}

static void noop_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	(void)data;
	(void)id;
	(void)key;
	(void)pressed;
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
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

static const char *service_protocol(void *data)
{
	(void)data;
	return "RTMP";
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

static void register_types(void)
{
	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = type_name,
		.create = input_create,
		.destroy = type_destroy,
	};
	obs_register_source(&input);

	struct obs_output_info output = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_ENCODED,
		.get_name = type_name,
		.create = output_create,
		.destroy = type_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);

	struct obs_encoder_info encoder = {
		.id = ENCODER_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = type_name,
		.create = encoder_create,
		.destroy = type_destroy,
		.encode = encoder_encode,
	};
	obs_register_encoder(&encoder);

	struct obs_service_info service = {
		.id = SERVICE_ID,
		.get_name = type_name,
		.create = service_create,
		.destroy = type_destroy,
		.get_protocol = service_protocol,
	};
	obs_register_service(&service);
}

struct observation {
	obs_hotkey_id id;
	obs_hotkey_registerer_t type;
	size_t signals;
	size_t presses;
	size_t releases;
	bool has_owner;
};

static bool find_key(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct observation *o = data;
	(void)key;
	assert_int_not_equal(id, o->id);
	return true;
}

static bool find_binding(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	struct observation *o = data;
	(void)idx;
	assert_int_not_equal(obs_hotkey_binding_get_hotkey_id(binding), o->id);
	return true;
}

static void callback(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	struct observation *o = data;
	assert_int_equal(id, o->id);
	assert_int_equal(obs_hotkey_get_id(key), id);
	if (pressed) {
		++o->presses;
	} else {
		assert_int_equal(o->signals, 1);
		assert_null(obs_hotkey_get_registerer(key));
		assert_string_equal(obs_hotkey_get_name(key), "remove");
		++o->releases;
	}
}

static void unregistered(void *data, calldata_t *params)
{
	struct observation *o = data;
	obs_hotkey_t *key = calldata_ptr(params, "key");
	assert_non_null(key);
	assert_int_equal(obs_hotkey_get_id(key), o->id);
	assert_string_equal(obs_hotkey_get_name(key), "remove");
	assert_string_equal(obs_hotkey_get_description(key), "Remove me");
	assert_int_equal(obs_hotkey_get_registerer_type(key), o->type);
	assert_int_equal(obs_hotkey_get_registerer(key) != NULL, o->has_owner);
	assert_int_equal(o->releases, 0);
	obs_enum_hotkeys(find_key, o);
	++o->signals;
}

static void remove_one_locked(void *data)
{
	struct observation *o = data;
	assert_int_not_equal(o->id, OBS_INVALID_HOTKEY_ID);
	signal_handler_t *signals = obs_get_signal_handler();
	signal_handler_connect(signals, "hotkey_unregister", unregistered, o);
	obs_key_combination_t combos[] = {{0, OBS_KEY_F24}, {0, OBS_KEY_F23}};
	obs_hotkey_load_bindings(o->id, combos, 2);
	obs_hotkey_inject_event(combos[0], true);
	assert_int_equal(o->presses, 1);
	obs_hotkey_unregister(OBS_INVALID_HOTKEY_ID);
	obs_hotkey_unregister(o->id + 1000);
	assert_int_equal(o->signals, 0);
	obs_hotkey_unregister(o->id);
	assert_int_equal(o->signals, 1);
	assert_int_equal(o->releases, 1);
	obs_enum_hotkeys(find_key, o);
	obs_enum_hotkey_bindings(find_binding, o);
	obs_hotkey_unregister(o->id);
	assert_int_equal(o->signals, 1);
	obs_hotkey_inject_event(combos[0], false);
	obs_hotkey_inject_event(combos[1], true);
	obs_hotkey_inject_event(combos[1], false);
	assert_int_equal(o->presses, 1);
	assert_int_equal(o->releases, 1);
	signal_handler_disconnect(signals, "hotkey_unregister", unregistered, o);
}

static void remove_one(struct observation *o)
{
	/* Hold the public recursive lock across injection and removal so the input
	 * thread cannot observe the synthetic key between these operations. */
	obs_hotkey_update_atomic(remove_one_locked, o);
}

static void frontend(void **state)
{
	(void)state;
	struct observation o = {.type = OBS_HOTKEY_REGISTERER_FRONTEND};
	o.id = obs_hotkey_register_frontend("remove", "Remove me", callback, &o);
	assert_int_not_equal(o.id, OBS_INVALID_HOTKEY_ID);
	remove_one(&o);
}

static void owners(void **state)
{
	(void)state;
	obs_source_t *source = obs_source_create(INPUT_ID, "source", NULL, NULL);
	obs_output_t *output = obs_output_create(OUTPUT_ID, "output", NULL, NULL);
	obs_encoder_t *encoder = obs_video_encoder_create(ENCODER_ID, "encoder", NULL, NULL);
	obs_service_t *service = obs_service_create(SERVICE_ID, "service", NULL, NULL);
	assert_non_null(source);
	assert_non_null(output);
	assert_non_null(encoder);
	assert_non_null(service);
	struct observation o = {.has_owner = true};
	o.type = OBS_HOTKEY_REGISTERER_SOURCE;
	o.id = obs_hotkey_register_source(source, "remove", "Remove me", callback, &o);
	remove_one(&o);
	o = (struct observation){.has_owner = true, .type = OBS_HOTKEY_REGISTERER_OUTPUT};
	o.id = obs_hotkey_register_output(output, "remove", "Remove me", callback, &o);
	remove_one(&o);
	o = (struct observation){.has_owner = true, .type = OBS_HOTKEY_REGISTERER_ENCODER};
	o.id = obs_hotkey_register_encoder(encoder, "remove", "Remove me", callback, &o);
	remove_one(&o);
	o = (struct observation){.has_owner = true, .type = OBS_HOTKEY_REGISTERER_SERVICE};
	o.id = obs_hotkey_register_service(service, "remove", "Remove me", callback, &o);
	remove_one(&o);
	/* Removing a hotkey releases its weak reference, never its live owner. */
	assert_string_equal(obs_source_get_name(source), "source");
	assert_string_equal(obs_output_get_name(output), "output");
	assert_string_equal(obs_encoder_get_name(encoder), "encoder");
	assert_string_equal(obs_service_get_name(service), "service");
	obs_source_release(source);
	obs_output_release(output);
	obs_encoder_release(encoder);
	obs_service_release(service);
}

static void no_task(void *data)
{
	(void)data;
}

int main(void)
{
	/* Also characterize the public wrapper's no-core early return. */
	obs_hotkey_unregister(OBS_INVALID_HOTKEY_ID);
	obs_hotkey_unregister(0);
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	obs_hotkey_enable_background_press(false);
	register_types();
	const struct CMUnitTest tests[] = {cmocka_unit_test(frontend), cmocka_unit_test(owners)};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	obs_shutdown();
	obs_hotkey_unregister(0);
	return result;
}
