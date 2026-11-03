/* Characterization of the hotkey data that libobs loads and saves per object:
 * obs_hotkeys_load_encoder/_output/_service/_source and
 * obs_hotkeys_save_encoder/_output/_service/_source, before their port (#3545).
 * "Characterized, not endorsed": a load adds to the bindings of the object's
 * hotkeys, it does not replace them.
 *
 * test_hotkey_bindings_codec covers the format of one binding and the source
 * save. Only the source functions had tests until now. This test links the
 * real libobs and goes through the public API, so it also covers the Rust lane.
 * Run under xvfb-run on Linux: obs_startup needs a display there.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/bmem.h>

#define INPUT_ID "hotkey_context_input"
#define OUTPUT_ID "hotkey_context_output"
#define ENCODER_ID "hotkey_context_encoder"
#define SERVICE_ID "hotkey_context_service"

/* One kind of object that holds hotkeys. */
struct kind {
	const char *name;
	void *(*create)(const char *name);
	void (*release)(void *object);
	obs_hotkey_id (*add_hotkey)(void *object, const char *name);
	void (*load)(void *object, obs_data_t *hotkeys);
	obs_data_t *(*save)(void *object);
};

static obs_hotkey_id changed_id;
static size_t changed;

/* ------------------------------------------------------------------------- */

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

/* ------------------------------------------------------------------------- */

static void *source_new(const char *name)
{
	return obs_source_create(INPUT_ID, name, NULL, NULL);
}

static void source_free(void *object)
{
	obs_source_release(object);
}

static obs_hotkey_id source_add(void *object, const char *name)
{
	return obs_hotkey_register_source(object, name, name, noop_hotkey, NULL);
}

static void source_load(void *object, obs_data_t *hotkeys)
{
	obs_hotkeys_load_source(object, hotkeys);
}

static obs_data_t *source_save(void *object)
{
	return obs_hotkeys_save_source(object);
}

static void *output_new(const char *name)
{
	return obs_output_create(OUTPUT_ID, name, NULL, NULL);
}

static void output_free(void *object)
{
	obs_output_release(object);
}

static obs_hotkey_id output_add(void *object, const char *name)
{
	return obs_hotkey_register_output(object, name, name, noop_hotkey, NULL);
}

static void output_load(void *object, obs_data_t *hotkeys)
{
	obs_hotkeys_load_output(object, hotkeys);
}

static obs_data_t *output_save(void *object)
{
	return obs_hotkeys_save_output(object);
}

static void *encoder_new(const char *name)
{
	return obs_video_encoder_create(ENCODER_ID, name, NULL, NULL);
}

static void encoder_free(void *object)
{
	obs_encoder_release(object);
}

static obs_hotkey_id encoder_add(void *object, const char *name)
{
	return obs_hotkey_register_encoder(object, name, name, noop_hotkey, NULL);
}

static void encoder_load(void *object, obs_data_t *hotkeys)
{
	obs_hotkeys_load_encoder(object, hotkeys);
}

static obs_data_t *encoder_save(void *object)
{
	return obs_hotkeys_save_encoder(object);
}

static void *service_new(const char *name)
{
	return obs_service_create(SERVICE_ID, name, NULL, NULL);
}

static void service_free(void *object)
{
	obs_service_release(object);
}

static obs_hotkey_id service_add(void *object, const char *name)
{
	return obs_hotkey_register_service(object, name, name, noop_hotkey, NULL);
}

static void service_load(void *object, obs_data_t *hotkeys)
{
	obs_hotkeys_load_service(object, hotkeys);
}

static obs_data_t *service_save(void *object)
{
	return obs_hotkeys_save_service(object);
}

static const struct kind kinds[] = {
	{"source", source_new, source_free, source_add, source_load, source_save},
	{"output", output_new, output_free, output_add, output_load, output_save},
	{"encoder", encoder_new, encoder_free, encoder_add, encoder_load, encoder_save},
	{"service", service_new, service_free, service_add, service_load, service_save},
};

#define FOR_EACH_KIND(k) for (const struct kind *k = kinds; k < kinds + sizeof(kinds) / sizeof(kinds[0]); k++)

/* ------------------------------------------------------------------------- */

static void load_json(const struct kind *k, void *object, const char *json)
{
	obs_data_t *data = obs_data_create_from_json(json);
	assert_non_null(data);
	k->load(object, data);
	obs_data_release(data);
}

static void assert_saved(const struct kind *k, void *object, const char *expected)
{
	obs_data_t *saved = k->save(object);
	assert_non_null(saved);
	const char *json = obs_data_get_json(saved);
	if (strcmp(json, expected) != 0)
		fprintf(stderr, "%s saved:    %s\n%s expected: %s\n", k->name, json, k->name, expected);
	assert_string_equal(json, expected);
	obs_data_release(saved);
}

static void bindings_changed(void *data, calldata_t *cd)
{
	(void)data;
	obs_hotkey_t *key = calldata_ptr(cd, "key");
	if (key && obs_hotkey_get_id(key) == changed_id)
		changed++;
}

/* ------------------------------------------------------------------------- */

static void no_hotkeys_saves_null(void **state)
{
	(void)state;
	FOR_EACH_KIND(k)
	{
		void *object = k->create("hotkey context none");
		assert_non_null(object);
		assert_null(k->save(object));
		k->release(object);
	}
}

static void hotkeys_without_bindings_save_empty_arrays(void **state)
{
	(void)state;
	FOR_EACH_KIND(k)
	{
		void *object = k->create("hotkey context empty");
		assert_non_null(object);
		k->add_hotkey(object, "ctx.first");
		k->add_hotkey(object, "ctx.second");
		assert_saved(k, object, "{\"ctx.first\":[],\"ctx.second\":[]}");
		k->release(object);
	}
}

static void load_reads_by_hotkey_name(void **state)
{
	(void)state;
	FOR_EACH_KIND(k)
	{
		void *object = k->create("hotkey context load");
		assert_non_null(object);
		k->add_hotkey(object, "ctx.first");
		k->add_hotkey(object, "ctx.second");

		load_json(k, object,
			  "{\"ctx.unknown\":[{\"key\":\"OBS_KEY_U\"}],"
			  "\"ctx.second\":[{\"control\":true,\"key\":\"OBS_KEY_S\"},{\"key\":\"OBS_KEY_T\"}]}");
		assert_saved(k, object,
			     "{\"ctx.first\":[],"
			     "\"ctx.second\":[{\"control\":true,\"key\":\"OBS_KEY_S\"},{\"key\":\"OBS_KEY_T\"}]}");
		k->release(object);
	}
}

/* Unlike obs_hotkey_load(), which replaces the bindings of its hotkey. */
static void load_adds_to_existing_bindings(void **state)
{
	(void)state;
	FOR_EACH_KIND(k)
	{
		void *object = k->create("hotkey context add");
		assert_non_null(object);
		k->add_hotkey(object, "ctx.first");

		load_json(k, object, "{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"}]}");
		load_json(k, object, "{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_B\"}]}");
		assert_saved(
			k, object,
			"{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_B\"}]}");
		k->release(object);
	}
}

static void hotkeys_of_other_objects_are_left_alone(void **state)
{
	(void)state;
	FOR_EACH_KIND(k)
	{
		void *loaded = k->create("hotkey context loaded");
		void *other = k->create("hotkey context other");
		assert_non_null(loaded);
		assert_non_null(other);
		k->add_hotkey(loaded, "ctx.first");
		k->add_hotkey(other, "ctx.first");

		load_json(k, loaded, "{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"}]}");
		assert_saved(k, loaded, "{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"}]}");
		assert_saved(k, other, "{\"ctx.first\":[]}");
		k->release(loaded);
		k->release(other);
	}
}

/* The signal goes out when the array has items, even if each item is rejected.
 * An empty or missing array sends nothing. */
static void load_signals_bindings_changed(void **state)
{
	(void)state;
	signal_handler_t *signals = obs_get_signal_handler();
	signal_handler_connect(signals, "hotkey_bindings_changed", bindings_changed, NULL);

	FOR_EACH_KIND(k)
	{
		void *object = k->create("hotkey context signal");
		assert_non_null(object);
		changed_id = k->add_hotkey(object, "ctx.first");
		changed = 0;

		load_json(k, object, "{\"ctx.other\":[{\"key\":\"OBS_KEY_A\"}]}");
		assert_int_equal(changed, 0);
		load_json(k, object, "{\"ctx.first\":[]}");
		assert_int_equal(changed, 0);
		load_json(k, object, "{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_B\"}]}");
		assert_int_equal(changed, 1);
		load_json(k, object, "{\"ctx.first\":[{\"key\":\"OBS_KEY_MOUSE1\"}]}");
		assert_int_equal(changed, 2);
		assert_saved(k, object, "{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"},{\"key\":\"OBS_KEY_B\"}]}");
		k->release(object);
	}

	signal_handler_disconnect(signals, "hotkey_bindings_changed", bindings_changed, NULL);
}

/* The object data marks a volume key. test_hotkey_volume_keys covers the swap. */
static void save_marks_volume_keys(void **state)
{
	(void)state;
	FOR_EACH_KIND(k)
	{
		void *object = k->create("hotkey context volume");
		assert_non_null(object);
		k->add_hotkey(object, "ctx.first");

		load_json(k, object,
			  "{\"ctx.first\":[{\"key\":\"OBS_KEY_VK_VOLUME_UP\",\"volume_keys_fixed\":true},"
			  "{\"key\":\"OBS_KEY_A\"}]}");
		assert_saved(k, object,
			     "{\"ctx.first\":[{\"key\":\"OBS_KEY_VK_VOLUME_UP\",\"volume_keys_fixed\":true},"
			     "{\"key\":\"OBS_KEY_A\"}]}");
		k->release(object);
	}
}

static void load_ignores_null(void **state)
{
	(void)state;
	obs_data_t *data = obs_data_create_from_json("{\"ctx.first\":[{\"key\":\"OBS_KEY_A\"}]}");
	assert_non_null(data);

	FOR_EACH_KIND(k)
	{
		void *object = k->create("hotkey context null");
		assert_non_null(object);
		k->add_hotkey(object, "ctx.first");

		k->load(NULL, data);
		k->load(object, NULL);
		assert_saved(k, object, "{\"ctx.first\":[]}");
		k->release(object);
	}

	obs_data_release(data);
}

/* ------------------------------------------------------------------------- */

static void no_task(void *param)
{
	(void)param;
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL)) {
		puts("obs_startup failed, skipping");
		return 0;
	}
	register_types();

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(no_hotkeys_saves_null),
		cmocka_unit_test(hotkeys_without_bindings_save_empty_arrays),
		cmocka_unit_test(load_reads_by_hotkey_name),
		cmocka_unit_test(load_adds_to_existing_bindings),
		cmocka_unit_test(hotkeys_of_other_objects_are_left_alone),
		cmocka_unit_test(load_signals_bindings_changed),
		cmocka_unit_test(save_marks_volume_keys),
		cmocka_unit_test(load_ignores_null),
	};
	int rc = cmocka_run_group_tests(tests, NULL, NULL);

	/* Sources are destroyed on the destroy task thread. */
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	obs_shutdown();
	return rc;
}
