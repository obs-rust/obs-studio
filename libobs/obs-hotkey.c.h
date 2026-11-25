/******************************************************************************
    Copyright (C) 2014-2015 by Ruwen Hahn <palana@stunned.de>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

/* Since ids are just sequential size_t integers, we don't really need a
 * hash function to get an even distribution across buckets.
 * (Realistically this should never wrap, who has 4.29 billion hotkeys?!)  */
#undef HASH_FUNCTION
#define HASH_FUNCTION(s, len, hashv) (hashv) = (unsigned int)(*s % UINT_MAX)

/* Custom definitions to make adding/looking up size_t integers easier */
#define HASH_ADD_HKEY(head, idfield, add) HASH_ADD(hh, head, idfield, sizeof(size_t), add)
#define HASH_FIND_HKEY(head, id, out) HASH_FIND(hh, head, &(id), sizeof(size_t), out)

static inline bool lock(void)
{
	if (!obs)
		return false;

	pthread_mutex_lock(&obs->hotkeys.mutex);
	return true;
}

static inline void unlock(void)
{
	pthread_mutex_unlock(&obs->hotkeys.mutex);
}

void obs_hotkeys_mutex_lock(void)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
}

void obs_hotkeys_mutex_unlock(void)
{
	pthread_mutex_unlock(&obs->hotkeys.mutex);
}

static void hotkey_signal(const char *signal, obs_hotkey_t *hotkey)
{
	calldata_t data;
	calldata_init(&data);
	calldata_set_ptr(&data, "key", hotkey);

	signal_handler_signal(obs->hotkeys.signals, signal, &data);

	calldata_free(&data);
}

static inline void context_add_hotkey(struct obs_context_data *context, obs_hotkey_id id)
{
	da_push_back(context->hotkeys, &id);
}

static inline obs_hotkey_id obs_hotkey_register_internal(obs_hotkey_registerer_t type, void *registerer,
							 struct obs_context_data *context, const char *name,
							 const char *description, obs_hotkey_func func, void *data)
{
	if ((obs->hotkeys.next_id + 1) == OBS_INVALID_HOTKEY_ID)
		blog(LOG_WARNING, "obs-hotkey: Available hotkey ids exhausted");

	obs_hotkey_id result = obs->hotkeys.next_id++;
	obs_hotkey_t *hotkey = bzalloc(sizeof(obs_hotkey_t));

	hotkey->id = result;
	hotkey->name = bstrdup(name);
	hotkey->description = bstrdup(description);
	hotkey->func = func;
	hotkey->data = data;
	hotkey->registerer_type = type;
	hotkey->registerer = registerer;
	hotkey->pair_partner_id = OBS_INVALID_HOTKEY_PAIR_ID;

	HASH_ADD_HKEY(obs->hotkeys.hotkeys, id, hotkey);

	if (context) {
		obs_data_array_t *binding_data = obs_data_get_array(context->hotkey_data, name);
		load_bindings(hotkey, binding_data, false, true);
		obs_data_array_release(binding_data);

		context_add_hotkey(context, result);
	}

	hotkey_signal("hotkey_register", hotkey);

	return result;
}

/* Private call adapter: allocation, saved bindings and signals stay here. */
obs_hotkey_id obs_hotkey_register_call(obs_hotkey_registerer_t type, void *registerer, struct obs_context_data *context,
				       const char *name, const char *description, obs_hotkey_func func, void *data);
obs_hotkey_id obs_hotkey_register_call(obs_hotkey_registerer_t type, void *registerer, struct obs_context_data *context,
				       const char *name, const char *description, obs_hotkey_func func, void *data)
{
	return obs_hotkey_register_internal(type, registerer, context, name, description, func, data);
}

static obs_hotkey_pair_t *create_hotkey_pair(struct obs_context_data *context, obs_hotkey_active_func func0,
					     obs_hotkey_active_func func1, void *data0, void *data1)
{
	if ((obs->hotkeys.next_pair_id + 1) == OBS_INVALID_HOTKEY_PAIR_ID)
		blog(LOG_WARNING, "obs-hotkey: Available hotkey pair ids "
				  "exhausted");

	obs_hotkey_pair_t *pair = bzalloc(sizeof(obs_hotkey_pair_t));

	pair->pair_id = obs->hotkeys.next_pair_id++;
	pair->func[0] = func0;
	pair->func[1] = func1;
	pair->id[0] = OBS_INVALID_HOTKEY_ID;
	pair->id[1] = OBS_INVALID_HOTKEY_ID;
	pair->data[0] = data0;
	pair->data[1] = data1;

	HASH_ADD_HKEY(obs->hotkeys.hotkey_pairs, pair_id, pair);

	if (context)
		da_push_back(context->hotkey_pairs, &pair->pair_id);

	return pair;
}

/* The pair press latch (obs-hotkey-pair.c.h) reaches the pair only through
 * these two. Declared in obs-internal.h; not exported. */
struct obs_hotkey_pair_fields obs_hotkey_pair_fields(obs_hotkey_pair_t *pair)
{
	return (struct obs_hotkey_pair_fields){&pair->pressed0, &pair->pressed1};
}

bool obs_hotkey_pair_call_func(obs_hotkey_pair_t *pair, size_t half, obs_hotkey_t *hotkey, bool pressed)
{
	return pair->func[half](pair->data[half], pair->pair_id, hotkey, pressed);
}

obs_hotkey_pair_id register_hotkey_pair_internal(obs_hotkey_registerer_t type, void *registerer,
						 void *(*weak_ref)(void *), struct obs_context_data *context,
						 const char *name0, const char *description0, const char *name1,
						 const char *description1, obs_hotkey_active_func func0,
						 obs_hotkey_active_func func1, void *data0, void *data1)
{

	if (!lock())
		return OBS_INVALID_HOTKEY_PAIR_ID;

	obs_hotkey_pair_t *pair = create_hotkey_pair(context, func0, func1, data0, data1);

	pair->id[0] = obs_hotkey_register_internal(type, weak_ref(registerer), context, name0, description0,
						   obs_hotkey_pair_first_func, pair);

	pair->id[1] = obs_hotkey_register_internal(type, weak_ref(registerer), context, name1, description1,
						   obs_hotkey_pair_second_func, pair);

	obs_hotkey_t *hotkey_1, *hotkey_2;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, pair->id[0], hotkey_1);
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, pair->id[1], hotkey_2);

	if (hotkey_1)
		hotkey_1->pair_partner_id = pair->id[1];
	if (hotkey_2)
		hotkey_2->pair_partner_id = pair->id[0];

	obs_hotkey_pair_id id = pair->pair_id;

	unlock();
	return id;
}

static inline void enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data)
{
	const size_t num = obs->hotkeys.bindings.num;
	obs_hotkey_binding_t *array = obs->hotkeys.bindings.array;
	for (size_t i = 0; i < num; i++) {
		if (!func(data, i, &array[i]))
			break;
	}
}

struct binding_find_data {
	obs_hotkey_id id;
	size_t *idx;
	bool found;
};

static inline bool binding_finder(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	struct binding_find_data *find = data;
	if (binding->hotkey_id != find->id)
		return true;

	*find->idx = idx;
	find->found = true;
	return false;
}

static inline bool find_binding(obs_hotkey_id id, size_t *idx)
{
	struct binding_find_data data = {id, idx, false};
	enum_bindings(binding_finder, &data);
	return data.found;
}

static inline bool remove_bindings(obs_hotkey_id id)
{
	bool removed = false;
	size_t idx;
	while (find_binding(id, &idx)) {
		obs_hotkey_binding_t *binding = &obs->hotkeys.bindings.array[idx];

		if (binding->pressed)
			obs_hotkey_binding_release(binding);

		da_erase(obs->hotkeys.bindings, idx);
		removed = true;
	}

	return removed;
}

/* The private seam of obs-hotkey-seam.h: what the hotkey save and load functions
 * need from the hotkey registry. Each is one lookup, one field or one call. */

bool obs_hotkey_seam_lock(void)
{
	return lock();
}

void obs_hotkey_seam_unlock(void)
{
	unlock();
}

obs_hotkey_t *obs_hotkey_seam_find(obs_hotkey_id id)
{
	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	return hotkey;
}

obs_hotkey_pair_t *obs_hotkey_seam_find_pair(obs_hotkey_pair_id id)
{
	obs_hotkey_pair_t *pair;
	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	return pair;
}

obs_hotkey_id obs_hotkey_seam_pair_id(const obs_hotkey_pair_t *pair, size_t half)
{
	return pair->id[half];
}

void obs_hotkey_seam_add_binding(obs_hotkey_t *hotkey, obs_key_combination_t combo)
{
	obs_hotkey_binding_t *binding = da_push_back_new(obs->hotkeys.bindings);
	if (!binding)
		return;

	binding->key = combo;
	binding->hotkey_id = hotkey->id;
	binding->hotkey = hotkey;
}

bool obs_hotkey_seam_remove_bindings(obs_hotkey_id id)
{
	return remove_bindings(id);
}

void obs_hotkey_seam_signal(const char *signal, obs_hotkey_t *hotkey)
{
	hotkey_signal(signal, hotkey);
}

void obs_hotkey_seam_enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data)
{
	enum_bindings(func, data);
}

size_t obs_hotkey_seam_context_count(const struct obs_context_data *context)
{
	return context->hotkeys.num;
}

bool obs_hotkey_seam_saved_volume_keys_swapped(void)
{
	return obs->hotkeys.saved_volume_keys_swapped;
}

struct obs_context_data *obs_hotkey_seam_encoder_context(obs_encoder_t *encoder)
{
	return &encoder->context;
}

struct obs_context_data *obs_hotkey_seam_output_context(obs_output_t *output)
{
	return &output->context;
}

struct obs_context_data *obs_hotkey_seam_service_context(obs_service_t *service)
{
	return &service->context;
}

struct obs_context_data *obs_hotkey_seam_source_context(obs_source_t *source)
{
	return &source->context;
}

extern void obs_hotkey_unregister_release_registerer(obs_hotkey_t *hotkey);
extern void obs_hotkey_unregister_one(obs_hotkey_id id);
#define release_registerer obs_hotkey_unregister_release_registerer
#define unregister_hotkey obs_hotkey_unregister_one

static inline void unregister_hotkey_pair(obs_hotkey_pair_id id)
{
	if (id >= obs->hotkeys.next_pair_id)
		return;

	obs_hotkey_pair_t *pair;
	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	if (!pair)
		return;

	unregister_hotkey(pair->id[0]);
	unregister_hotkey(pair->id[1]);

	HASH_DEL(obs->hotkeys.hotkey_pairs, pair);
	bfree(pair);
}

/* Private call seam: retain pair teardown in this translation unit. */
void obs_hotkey_unregister_pair_call(obs_hotkey_pair_id id);
void obs_hotkey_unregister_pair_call(obs_hotkey_pair_id id)
{
	unregister_hotkey_pair(id);
}

static void context_release_hotkeys(struct obs_context_data *context)
{
	if (!context->hotkeys.num)
		goto cleanup;

	for (size_t i = 0; i < context->hotkeys.num; i++)
		unregister_hotkey(context->hotkeys.array[i]);

cleanup:
	da_free(context->hotkeys);
}

static void context_release_hotkey_pairs(struct obs_context_data *context)
{
	if (!context->hotkey_pairs.num)
		goto cleanup;

	for (size_t i = 0; i < context->hotkey_pairs.num; i++)
		unregister_hotkey_pair(context->hotkey_pairs.array[i]);

cleanup:
	da_free(context->hotkey_pairs);
}

void obs_hotkeys_context_release(struct obs_context_data *context)
{
	if (!lock())
		return;

	context_release_hotkeys(context);
	context_release_hotkey_pairs(context);

	obs_data_release(context->hotkey_data);
	unlock();
}

void obs_hotkeys_free(void)
{
	obs_hotkey_t *hotkey, *tmp;
	HASH_ITER (hh, obs->hotkeys.hotkeys, hotkey, tmp) {
		HASH_DEL(obs->hotkeys.hotkeys, hotkey);
		bfree(hotkey->name);
		bfree(hotkey->description);
		release_registerer(hotkey);
		bfree(hotkey);
	}

	obs_hotkey_pair_t *pair, *tmp2;
	HASH_ITER (hh, obs->hotkeys.hotkey_pairs, pair, tmp2) {
		HASH_DEL(obs->hotkeys.hotkey_pairs, pair);
		bfree(pair);
	}

	da_free(obs->hotkeys.bindings);

	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		if (obs->hotkeys.translations[i]) {
			bfree(obs->hotkeys.translations[i]);
			obs->hotkeys.translations[i] = NULL;
		}
	}
}

struct obs_hotkey_binding_fields obs_hotkey_binding_fields(obs_hotkey_binding_t *binding)
{
	return (struct obs_hotkey_binding_fields){
		.key = &binding->key,
		.pressed = &binding->pressed,
		.modifiers_match = &binding->modifiers_match,
	};
}

bool obs_hotkey_key_is_pressed(obs_key_t key)
{
	return obs_hotkeys_platform_is_pressed(obs->hotkeys.platform_context, key);
}

/* The hotkey binding press count (obs-hotkey-binding-press.c): the binding's
 * pressed flag, its hotkey's pressed count and the hotkey, as pointers, so the
 * unit needs no layout of struct obs_hotkey_binding or struct obs_hotkey and
 * writes them in place. */
struct obs_hotkey_binding_count_fields obs_hotkey_binding_count_fields(obs_hotkey_binding_t *binding)
{
	return (struct obs_hotkey_binding_count_fields){
		.pressed = &binding->pressed,
		.count = &binding->hotkey->pressed,
		.hotkey = binding->hotkey,
	};
}

void obs_hotkeys_background_clear_shortcuts(struct obs_hotkeys_background *background)
{
	for (size_t i = 0; i < background->shortcuts.num; i++)
		bfree(background->shortcuts.array[i].trigger);
	da_resize(background->shortcuts, 0);
}

struct obs_query_hotkeys_helper {
	uint32_t modifiers;
	bool no_press;
	bool strict_modifiers;
};

static inline bool query_hotkey(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	UNUSED_PARAMETER(idx);

	struct obs_query_hotkeys_helper *param = (struct obs_query_hotkeys_helper *)data;
	obs_hotkey_binding_query(binding, param->modifiers, param->no_press, param->strict_modifiers);

	return true;
}

static inline void query_hotkeys(void)
{
	struct obs_query_hotkeys_helper param = {
		obs_hotkeys_query_modifiers(),
		obs->hotkeys.thread_disable_press,
		obs->hotkeys.strict_modifiers,
	};
	enum_bindings(query_hotkey, &param);
}

#define NBSP "\xC2\xA0"

void *obs_hotkey_thread(void *arg)
{
	/* arg is the event obs_init_hotkeys waits on (or NULL). */
	os_event_t *ready = arg;

	os_set_thread_name("libobs: hotkey thread");

	const char *hotkey_thread_name =
		profile_store_name(obs_get_profiler_name_store(), "obs_hotkey_thread(%g" NBSP "ms)", 25.);
	profile_register_root(hotkey_thread_name, (uint64_t)25000000);

	/* Do the first profiled pass now so every one-time allocation of this
	 * thread happens before obs_startup() returns (bnum_allocs() must be
	 * stable afterwards, obs-rust/obs-studio#663). */
	profile_start(hotkey_thread_name);
	profile_end(hotkey_thread_name);
	profile_reenable_thread();
	if (ready)
		os_event_signal(ready);

	while (os_event_timedwait(obs->hotkeys.stop_event, 25) == ETIMEDOUT) {
		if (!lock())
			continue;

		profile_start(hotkey_thread_name);
		query_hotkeys();
		profile_end(hotkey_thread_name);

		unlock();

		profile_reenable_thread();
	}
	return NULL;
}

/* This file hashes integer keys (HASH_FUNCTION above); restore util/uthash.h's
 * default for the sources a unity build compiles after it. */
#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef HASH_ADD_HKEY
#undef HASH_FIND_HKEY
#undef NBSP
#undef VOLUME_KEYS_FIXED

#undef release_registerer
#undef unregister_hotkey
