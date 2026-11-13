/* Characterize the real object-bound pair registration APIs before extraction.
 * Shared unchanged by hybrid libobs and libobs-c-reference; no source inclusion
 * or replacement libobs symbols. The pair press/latch state machine is outside
 * this unit: routed calls only verify callback and independent data forwarding. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs.h>
#include <string.h>

struct callback_data {
	size_t half;
	size_t calls;
	obs_hotkey_pair_id pair;
	obs_hotkey_t *key;
	bool pressed;
};

static bool record_call(void *data, size_t half, obs_hotkey_pair_id pair, obs_hotkey_t *key, bool pressed)
{
	struct callback_data *call = data;
	assert_non_null(call);
	assert_int_equal(call->half, half);
	assert_int_equal(pair, call->pair);
	assert_ptr_equal(key, call->key);
	call->calls++;
	call->pressed = pressed;
	return false;
}

static bool first_call(void *data, obs_hotkey_pair_id pair, obs_hotkey_t *key, bool pressed)
{
	return record_call(data, 0, pair, key, pressed);
}

static bool second_call(void *data, obs_hotkey_pair_id pair, obs_hotkey_t *key, bool pressed)
{
	return record_call(data, 1, pair, key, pressed);
}

static const char *type_name(void *data)
{
	(void)data;
	return "hotkey pair registration";
}

static void type_destroy(void *data)
{
	(void)data;
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *owner)
{
	(void)settings;
	return owner;
}

static void *output_create(obs_data_t *settings, obs_output_t *owner)
{
	(void)settings;
	return owner;
}

static void *service_create(obs_data_t *settings, obs_service_t *owner)
{
	(void)settings;
	return owner;
}

static void *source_create(obs_data_t *settings, obs_source_t *owner)
{
	(void)settings;
	return owner;
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
	struct obs_encoder_info encoder = {
		.id = "pair_encoder",
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = type_name,
		.create = encoder_create,
		.destroy = type_destroy,
		.encode = encoder_encode,
	};
	obs_register_encoder(&encoder);
	struct obs_output_info output = {
		.id = "pair_output",
		.flags = OBS_OUTPUT_ENCODED,
		.get_name = type_name,
		.create = output_create,
		.destroy = type_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);
	struct obs_service_info service = {
		.id = "pair_service",
		.get_name = type_name,
		.create = service_create,
		.destroy = type_destroy,
		.get_protocol = service_protocol,
	};
	obs_register_service(&service);
	struct obs_source_info source = {
		.id = "pair_source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = type_name,
		.create = source_create,
		.destroy = type_destroy,
	};
	obs_register_source(&source);
}

struct owner_kind {
	obs_hotkey_registerer_t type;
	void *(*create)(obs_data_t *hotkeys);
	void (*release)(void *owner);
	void *(*weak)(void *owner);
	void (*weak_release)(void *weak);
	obs_data_t *(*save)(void *owner);
	obs_hotkey_pair_id (*pair)(void *owner, const char *name0, const char *desc0, const char *name1,
				   const char *desc1, void *data0, void *data1);
};

static void *new_encoder(obs_data_t *hotkeys)
{
	return obs_video_encoder_create("pair_encoder", "owner", NULL, hotkeys);
}

static void release_encoder(void *owner)
{
	obs_encoder_release(owner);
}

static void *weak_encoder(void *owner)
{
	return obs_encoder_get_weak_encoder(owner);
}

static void release_weak_encoder(void *weak)
{
	obs_weak_encoder_release(weak);
}

static obs_data_t *save_encoder(void *owner)
{
	return obs_hotkeys_save_encoder(owner);
}

static obs_hotkey_pair_id pair_encoder(void *owner, const char *name0, const char *desc0, const char *name1,
				       const char *desc1, void *data0, void *data1)
{
	return obs_hotkey_pair_register_encoder(owner, name0, desc0, name1, desc1, first_call, second_call, data0,
						data1);
}

static void *new_output(obs_data_t *hotkeys)
{
	return obs_output_create("pair_output", "owner", NULL, hotkeys);
}

static void release_output(void *owner)
{
	obs_output_release(owner);
}

static void *weak_output(void *owner)
{
	return obs_output_get_weak_output(owner);
}

static void release_weak_output(void *weak)
{
	obs_weak_output_release(weak);
}

static obs_data_t *save_output(void *owner)
{
	return obs_hotkeys_save_output(owner);
}

static obs_hotkey_pair_id pair_output(void *owner, const char *name0, const char *desc0, const char *name1,
				      const char *desc1, void *data0, void *data1)
{
	return obs_hotkey_pair_register_output(owner, name0, desc0, name1, desc1, first_call, second_call, data0,
					       data1);
}

static void *new_service(obs_data_t *hotkeys)
{
	return obs_service_create("pair_service", "owner", NULL, hotkeys);
}

static void release_service(void *owner)
{
	obs_service_release(owner);
}

static void *weak_service(void *owner)
{
	return obs_service_get_weak_service(owner);
}

static void release_weak_service(void *weak)
{
	obs_weak_service_release(weak);
}

static obs_data_t *save_service(void *owner)
{
	return obs_hotkeys_save_service(owner);
}

static obs_hotkey_pair_id pair_service(void *owner, const char *name0, const char *desc0, const char *name1,
				       const char *desc1, void *data0, void *data1)
{
	return obs_hotkey_pair_register_service(owner, name0, desc0, name1, desc1, first_call, second_call, data0,
						data1);
}

static void *new_source(obs_data_t *hotkeys)
{
	return obs_source_create("pair_source", "owner", NULL, hotkeys);
}

static void release_source(void *owner)
{
	obs_source_release(owner);
}

static void *weak_source(void *owner)
{
	return obs_source_get_weak_source(owner);
}

static void release_weak_source(void *weak)
{
	obs_weak_source_release(weak);
}

static obs_data_t *save_source(void *owner)
{
	return obs_hotkeys_save_source(owner);
}

static obs_hotkey_pair_id pair_source(void *owner, const char *name0, const char *desc0, const char *name1,
				      const char *desc1, void *data0, void *data1)
{
	return obs_hotkey_pair_register_source(owner, name0, desc0, name1, desc1, first_call, second_call, data0,
					       data1);
}

static const struct owner_kind kinds[] = {
	{OBS_HOTKEY_REGISTERER_ENCODER, new_encoder, release_encoder, weak_encoder, release_weak_encoder, save_encoder,
	 pair_encoder},
	{OBS_HOTKEY_REGISTERER_OUTPUT, new_output, release_output, weak_output, release_weak_output, save_output,
	 pair_output},
	{OBS_HOTKEY_REGISTERER_SERVICE, new_service, release_service, weak_service, release_weak_service, save_service,
	 pair_service},
	{OBS_HOTKEY_REGISTERER_SOURCE, new_source, release_source, weak_source, release_weak_source, save_source,
	 pair_source},
};

struct key_search {
	obs_hotkey_t *keys[2];
};

static bool find_pair_keys(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct key_search *found = data;
	assert_int_equal(obs_hotkey_get_id(key), id);
	const char *name = obs_hotkey_get_name(key);
	if (name && strcmp(name, "object.on") == 0)
		found->keys[0] = key;
	if (name && strcmp(name, "object.off") == 0)
		found->keys[1] = key;
	return true;
}

static struct key_search pair_keys(void)
{
	struct key_search found = {0};
	obs_enum_hotkeys(find_pair_keys, &found);
	return found;
}

static void null_owners(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(kinds) / sizeof(kinds[0]); i++)
		assert_int_equal(kinds[i].pair(NULL, "object.on", "On", "object.off", "Off", NULL, NULL),
				 OBS_INVALID_HOTKEY_PAIR_ID);
}

static void no_task(void *data)
{
	(void)data;
}

static void check_saved(const struct owner_kind *kind, void *owner, void *other)
{
	obs_data_t *saved = kind->save(owner);
	assert_non_null(saved);
	const char *names[] = {"object.on", "object.off"};
	const char *keys[] = {"OBS_KEY_A", "OBS_KEY_B"};
	for (size_t i = 0; i < 2; i++) {
		obs_data_array_t *bindings = obs_data_get_array(saved, names[i]);
		assert_non_null(bindings);
		assert_int_equal(obs_data_array_count(bindings), 1);
		obs_data_t *binding = obs_data_array_item(bindings, 0);
		assert_string_equal(obs_data_get_string(binding, "key"), keys[i]);
		obs_data_release(binding);
		obs_data_array_release(bindings);
	}
	obs_data_release(saved);
	saved = kind->save(other);
	assert_false(obs_data_has_user_value(saved, names[0]));
	assert_false(obs_data_has_user_value(saved, names[1]));
	obs_data_release(saved);
}

static void object_registration(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	register_types();
	obs_hotkey_enable_callback_rerouting(true);
	/* NULL owners must neither enroll keys nor consume IDs with a live core. */
	null_owners(NULL);
	for (size_t i = 0; i < sizeof(kinds) / sizeof(kinds[0]); i++) {
		const struct owner_kind *kind = &kinds[i];
		obs_data_t *hotkeys = obs_data_create_from_json("{\"object.on\":[{\"key\":\"OBS_KEY_A\"}],"
								"\"object.off\":[{\"key\":\"OBS_KEY_B\"}]}");
		assert_non_null(hotkeys);
		void *owner = kind->create(hotkeys);
		void *other = kind->create(NULL);
		obs_data_release(hotkeys);
		assert_non_null(owner);
		assert_non_null(other);
		void *weak = kind->weak(owner);
		assert_non_null(weak);
		struct callback_data calls[2] = {{.half = 0}, {.half = 1}};
		char name0[] = "object.on", name1[] = "object.off";
		char desc0[] = "Enable object", desc1[] = "Disable object";
		obs_hotkey_pair_id pair = kind->pair(owner, name0, desc0, name1, desc1, &calls[0], &calls[1]);
		assert_true(pair != OBS_INVALID_HOTKEY_PAIR_ID);
		if (i == 0)
			assert_int_equal(pair, 0);
		struct key_search found = pair_keys();
		assert_non_null(found.keys[0]);
		assert_non_null(found.keys[1]);
		assert_ptr_not_equal(found.keys[0], found.keys[1]);
		obs_hotkey_id ids[] = {obs_hotkey_get_id(found.keys[0]), obs_hotkey_get_id(found.keys[1])};
		if (i == 0) {
			assert_int_equal(ids[0], 0);
			assert_int_equal(ids[1], 1);
		}
		/* The C implementation copies metadata and acquires the same weak owner
		 * separately for each half. No strong owner pointer is stored as data. */
		name0[0] = name1[0] = desc0[0] = desc1[0] = 'X';
		for (size_t half = 0; half < 2; half++) {
			assert_int_equal(obs_hotkey_get_registerer_type(found.keys[half]), kind->type);
			assert_ptr_equal(obs_hotkey_get_registerer(found.keys[half]), weak);
			assert_int_equal(obs_hotkey_get_pair_partner_id(found.keys[half]), ids[1 - half]);
			calls[half].pair = pair;
			calls[half].key = found.keys[half];
			obs_hotkey_trigger_routed_callback(ids[half], true);
			assert_int_equal(calls[half].calls, 1);
			assert_true(calls[half].pressed);
			assert_int_equal(calls[1 - half].calls, half == 0 ? 0 : 2);
			obs_hotkey_trigger_routed_callback(ids[half], false);
			assert_int_equal(calls[half].calls, 2);
			assert_false(calls[half].pressed);
		}
		assert_string_equal(obs_hotkey_get_name(found.keys[0]), "object.on");
		assert_string_equal(obs_hotkey_get_name(found.keys[1]), "object.off");
		assert_string_equal(obs_hotkey_get_description(found.keys[0]), "Enable object");
		assert_string_equal(obs_hotkey_get_description(found.keys[1]), "Disable object");
		check_saved(kind, owner, other);
		obs_hotkey_pair_unregister(pair);
		found = pair_keys();
		assert_null(found.keys[0]);
		assert_null(found.keys[1]);
		/* Leave a second pair registered for the real owner's destroy path. */
		pair = kind->pair(owner, "object.on", "On", "object.off", "Off", &calls[0], &calls[1]);
		assert_true(pair != OBS_INVALID_HOTKEY_PAIR_ID);
		kind->release(owner);
		kind->release(other);
		obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
		found = pair_keys();
		assert_null(found.keys[0]);
		assert_null(found.keys[1]);
		kind->weak_release(weak);
		/* Destroy removed the context's pair as well as its two keys. */
		obs_hotkey_pair_unregister(pair);
	}
	/* Unlike single-hotkey source registration, paired registration admits a
	 * private source (characterized, not endorsed). NULL callbacks/data are
	 * accepted at registration; invoking such callbacks is outside the domain. */
	obs_source_t *private_source = obs_source_create_private("pair_source", "private", NULL);
	assert_non_null(private_source);
	obs_hotkey_pair_id private_pair = obs_hotkey_pair_register_source(
		private_source, "object.on", "Private on", "object.off", "Private off", NULL, NULL, NULL, NULL);
	assert_true(private_pair != OBS_INVALID_HOTKEY_PAIR_ID);
	struct key_search private_keys = pair_keys();
	void *private_weak = obs_source_get_weak_source(private_source);
	for (size_t half = 0; half < 2; half++) {
		assert_non_null(private_keys.keys[half]);
		assert_int_equal(obs_hotkey_get_registerer_type(private_keys.keys[half]), OBS_HOTKEY_REGISTERER_SOURCE);
		assert_ptr_equal(obs_hotkey_get_registerer(private_keys.keys[half]), private_weak);
	}
	obs_source_release(private_source);
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	private_keys = pair_keys();
	assert_null(private_keys.keys[0]);
	assert_null(private_keys.keys[1]);
	obs_weak_source_release(private_weak);
	obs_hotkey_enable_callback_rerouting(false);
	obs_shutdown();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_owners),
		cmocka_unit_test(object_registration),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
