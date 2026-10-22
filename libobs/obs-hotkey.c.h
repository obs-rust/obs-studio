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
#define HASH_FUNCTION(s, len, hashv) (hashv) = *s % UINT_MAX

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

obs_hotkey_id obs_hotkey_get_id(const obs_hotkey_t *key)
{
	return key->id;
}

const char *obs_hotkey_get_name(const obs_hotkey_t *key)
{
	return key->name;
}

const char *obs_hotkey_get_description(const obs_hotkey_t *key)
{
	return key->description;
}

obs_hotkey_registerer_t obs_hotkey_get_registerer_type(const obs_hotkey_t *key)
{
	return key->registerer_type;
}

void *obs_hotkey_get_registerer(const obs_hotkey_t *key)
{
	return key->registerer;
}

obs_hotkey_id obs_hotkey_get_pair_partner_id(const obs_hotkey_t *key)
{
	return key->pair_partner_id;
}

obs_key_combination_t obs_hotkey_binding_get_key_combination(obs_hotkey_binding_t *binding)
{
	return binding->key;
}

obs_hotkey_id obs_hotkey_binding_get_hotkey_id(obs_hotkey_binding_t *binding)
{
	return binding->hotkey_id;
}

obs_hotkey_t *obs_hotkey_binding_get_hotkey(obs_hotkey_binding_t *binding)
{
	return binding->hotkey;
}

void obs_hotkey_set_name(obs_hotkey_id id, const char *name)
{
	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		return;

	bfree(hotkey->name);
	hotkey->name = bstrdup(name);
}

void obs_hotkey_set_description(obs_hotkey_id id, const char *desc)
{
	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		return;

	bfree(hotkey->description);
	hotkey->description = bstrdup(desc);
}

void obs_hotkey_pair_set_names(obs_hotkey_pair_id id, const char *name0, const char *name1)
{
	obs_hotkey_pair_t *pair;

	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	if (!pair)
		return;

	obs_hotkey_set_name(pair->id[0], name0);
	obs_hotkey_set_name(pair->id[1], name1);
}

void obs_hotkey_pair_set_descriptions(obs_hotkey_pair_id id, const char *desc0, const char *desc1)
{
	obs_hotkey_pair_t *pair;

	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	if (!pair)
		return;

	obs_hotkey_set_description(pair->id[0], desc0);
	obs_hotkey_set_description(pair->id[1], desc1);
}

static void hotkey_signal(const char *signal, obs_hotkey_t *hotkey)
{
	calldata_t data;
	calldata_init(&data);
	calldata_set_ptr(&data, "key", hotkey);

	signal_handler_signal(obs->hotkeys.signals, signal, &data);

	calldata_free(&data);
}

static inline void load_bindings(obs_hotkey_t *hotkey, obs_data_array_t *data, bool removed);

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
		load_bindings(hotkey, binding_data, false);
		obs_data_array_release(binding_data);

		context_add_hotkey(context, result);
	}

	hotkey_signal("hotkey_register", hotkey);

	return result;
}

obs_hotkey_id obs_hotkey_register_frontend(const char *name, const char *description, obs_hotkey_func func, void *data)
{
	if (!lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id =
		obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_FRONTEND, NULL, NULL, name, description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_encoder(obs_encoder_t *encoder, const char *name, const char *description,
					  obs_hotkey_func func, void *data)
{
	if (!encoder || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_ENCODER,
							obs_encoder_get_weak_encoder(encoder), &encoder->context, name,
							description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_output(obs_output_t *output, const char *name, const char *description,
					 obs_hotkey_func func, void *data)
{
	if (!output || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_OUTPUT,
							obs_output_get_weak_output(output), &output->context, name,
							description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_service(obs_service_t *service, const char *name, const char *description,
					  obs_hotkey_func func, void *data)
{
	if (!service || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_SERVICE,
							obs_service_get_weak_service(service), &service->context, name,
							description, func, data);

	unlock();
	return id;
}

obs_hotkey_id obs_hotkey_register_source(obs_source_t *source, const char *name, const char *description,
					 obs_hotkey_func func, void *data)
{
	if (!source || source->context.private || !lock())
		return OBS_INVALID_HOTKEY_ID;

	obs_hotkey_id id = obs_hotkey_register_internal(OBS_HOTKEY_REGISTERER_SOURCE,
							obs_source_get_weak_source(source), &source->context, name,
							description, func, data);

	unlock();
	return id;
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

static void obs_hotkey_pair_first_func(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);

	obs_hotkey_pair_t *pair = data;
	if (pair->pressed1)
		return;

	if (pair->pressed0 && !pressed)
		pair->pressed0 = false;
	else if (pair->func[0](pair->data[0], pair->pair_id, hotkey, pressed))
		pair->pressed0 = pressed;
}

static void obs_hotkey_pair_second_func(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);

	obs_hotkey_pair_t *pair = data;
	if (pair->pressed0)
		return;

	if (pair->pressed1 && !pressed)
		pair->pressed1 = false;
	else if (pair->func[1](pair->data[1], pair->pair_id, hotkey, pressed))
		pair->pressed1 = pressed;
}

static obs_hotkey_pair_id register_hotkey_pair_internal(obs_hotkey_registerer_t type, void *registerer,
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

static inline void *obs_id_(void *id_)
{
	return id_;
}

obs_hotkey_pair_id obs_hotkey_pair_register_frontend(const char *name0, const char *description0, const char *name1,
						     const char *description1, obs_hotkey_active_func func0,
						     obs_hotkey_active_func func1, void *data0, void *data1)
{
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_FRONTEND, NULL, obs_id_, NULL, name0, description0,
					     name1, description1, func0, func1, data0, data1);
}

static inline void *weak_encoder_ref(void *ref)
{
	return obs_encoder_get_weak_encoder(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_encoder(obs_encoder_t *encoder, const char *name0, const char *description0,
						    const char *name1, const char *description1,
						    obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						    void *data0, void *data1)
{
	if (!encoder)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_ENCODER, encoder, weak_encoder_ref,
					     &encoder->context, name0, description0, name1, description1, func0, func1,
					     data0, data1);
}

static inline void *weak_output_ref(void *ref)
{
	return obs_output_get_weak_output(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_output(obs_output_t *output, const char *name0, const char *description0,
						   const char *name1, const char *description1,
						   obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						   void *data0, void *data1)
{
	if (!output)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_OUTPUT, output, weak_output_ref, &output->context,
					     name0, description0, name1, description1, func0, func1, data0, data1);
}

static inline void *weak_service_ref(void *ref)
{
	return obs_service_get_weak_service(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_service(obs_service_t *service, const char *name0, const char *description0,
						    const char *name1, const char *description1,
						    obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						    void *data0, void *data1)
{
	if (!service)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_SERVICE, service, weak_service_ref,
					     &service->context, name0, description0, name1, description1, func0, func1,
					     data0, data1);
}

static inline void *weak_source_ref(void *ref)
{
	return obs_source_get_weak_source(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_source(obs_source_t *source, const char *name0, const char *description0,
						   const char *name1, const char *description1,
						   obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						   void *data0, void *data1)
{
	if (!source)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_SOURCE, source, weak_source_ref, &source->context,
					     name0, description0, name1, description1, func0, func1, data0, data1);
}

typedef bool (*obs_hotkey_binding_internal_enum_func)(void *data, size_t idx, obs_hotkey_binding_t *binding);

static inline void enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data)
{
	const size_t num = obs->hotkeys.bindings.num;
	obs_hotkey_binding_t *array = obs->hotkeys.bindings.array;
	for (size_t i = 0; i < num; i++) {
		if (!func(data, i, &array[i]))
			break;
	}
}

typedef bool (*obs_hotkey_internal_enum_func)(void *data, obs_hotkey_t *hotkey);

static inline void enum_context_hotkeys(struct obs_context_data *context, obs_hotkey_internal_enum_func func,
					void *data)
{
	const size_t num = context->hotkeys.num;
	const obs_hotkey_id *array = context->hotkeys.array;
	obs_hotkey_t *hotkey;

	for (size_t i = 0; i < num; i++) {
		HASH_FIND_HKEY(obs->hotkeys.hotkeys, array[i], hotkey);
		if (!hotkey)
			continue;
		if (!func(data, hotkey))
			break;
	}
}

static inline void create_binding(obs_hotkey_t *hotkey, obs_key_combination_t combo)
{
	if (!obs_hotkey_combination_bindable(combo))
		return;

	obs_hotkey_binding_t *binding = da_push_back_new(obs->hotkeys.bindings);
	if (!binding)
		return;

	binding->key = combo;
	binding->hotkey_id = hotkey->id;
	binding->hotkey = hotkey;
}

static inline void load_binding(obs_hotkey_t *hotkey, obs_data_t *data)
{
	if (!hotkey || !data)
		return;

	obs_key_combination_t combo;
	if (!obs_hotkey_combination_from_data(data, &combo))
		return;

	create_binding(hotkey, combo);
}

/* removed: the caller's remove_bindings() removed bindings of this hotkey
 * first, so an empty or missing array still changed them (as in
 * obs_hotkey_load_bindings()). */
static inline void load_bindings(obs_hotkey_t *hotkey, obs_data_array_t *data, bool removed)
{
	const size_t count = obs_data_array_count(data);
	for (size_t i = 0; i < count; i++) {
		obs_data_t *item = obs_data_array_item(data, i);
		load_binding(hotkey, item);
		obs_data_release(item);
	}

	if (count || removed)
		hotkey_signal("hotkey_bindings_changed", hotkey);
}

static inline bool remove_bindings(obs_hotkey_id id);

void obs_hotkey_load_bindings(obs_hotkey_id id, obs_key_combination_t *combinations, size_t num)
{
	if (!lock())
		return;

	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (hotkey) {
		bool changed = remove_bindings(id);
		for (size_t i = 0; i < num; i++)
			create_binding(hotkey, combinations[i]);

		if (num || changed)
			hotkey_signal("hotkey_bindings_changed", hotkey);
	}

	unlock();
}

void obs_hotkey_load(obs_hotkey_id id, obs_data_array_t *data)
{
	if (!lock())
		return;

	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (hotkey) {
		bool removed = remove_bindings(id);
		load_bindings(hotkey, data, removed);
	}
	unlock();
}

static inline bool enum_load_bindings(void *data, obs_hotkey_t *hotkey)
{
	obs_data_array_t *hotkey_data = obs_data_get_array(data, hotkey->name);
	if (!hotkey_data)
		return true;

	load_bindings(hotkey, hotkey_data, false);
	obs_data_array_release(hotkey_data);
	return true;
}

void obs_hotkeys_load_encoder(obs_encoder_t *encoder, obs_data_t *hotkeys)
{
	if (!encoder || !hotkeys)
		return;
	if (!lock())
		return;

	enum_context_hotkeys(&encoder->context, enum_load_bindings, hotkeys);
	unlock();
}

void obs_hotkeys_load_output(obs_output_t *output, obs_data_t *hotkeys)
{
	if (!output || !hotkeys)
		return;
	if (!lock())
		return;

	enum_context_hotkeys(&output->context, enum_load_bindings, hotkeys);
	unlock();
}

void obs_hotkeys_load_service(obs_service_t *service, obs_data_t *hotkeys)
{
	if (!service || !hotkeys)
		return;
	if (!lock())
		return;

	enum_context_hotkeys(&service->context, enum_load_bindings, hotkeys);
	unlock();
}

void obs_hotkeys_load_source(obs_source_t *source, obs_data_t *hotkeys)
{
	if (!source || !hotkeys)
		return;
	if (!lock())
		return;

	enum_context_hotkeys(&source->context, enum_load_bindings, hotkeys);
	unlock();
}

void obs_hotkey_pair_load(obs_hotkey_pair_id id, obs_data_array_t *data0, obs_data_array_t *data1)
{
	if ((!data0 && !data1) || !lock())
		return;

	obs_hotkey_pair_t *pair;
	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	if (!pair)
		goto unlock;

	obs_hotkey_t *p1, *p2;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, pair->id[0], p1);
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, pair->id[1], p2);

	if (p1) {
		bool removed = remove_bindings(pair->id[0]);
		load_bindings(p1, data0, removed);
	}
	if (p2) {
		bool removed = remove_bindings(pair->id[1]);
		load_bindings(p2, data1, removed);
	}

unlock:
	unlock();
}

struct save_bindings_helper_t {
	obs_data_array_t *array;
	obs_hotkey_t *hotkey;
};

static inline bool save_bindings_helper(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	UNUSED_PARAMETER(idx);
	struct save_bindings_helper_t *h = data;

	if (h->hotkey->id != binding->hotkey_id)
		return true;

	obs_data_t *hotkey = obs_data_create();

	obs_hotkey_combination_to_data(binding->key, hotkey);

	obs_data_array_push_back(h->array, hotkey);

	obs_data_release(hotkey);

	return true;
}

static inline obs_data_array_t *save_hotkey(obs_hotkey_t *hotkey)
{
	obs_data_array_t *data = obs_data_array_create();

	struct save_bindings_helper_t arg = {data, hotkey};
	enum_bindings(save_bindings_helper, &arg);

	return data;
}

obs_data_array_t *obs_hotkey_save(obs_hotkey_id id)
{
	obs_data_array_t *result = NULL;

	if (!lock())
		return result;

	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (hotkey)
		result = save_hotkey(hotkey);

	unlock();

	return result;
}

void obs_hotkey_pair_save(obs_hotkey_pair_id id, obs_data_array_t **p_data0, obs_data_array_t **p_data1)
{
	if ((!p_data0 && !p_data1) || !lock())
		return;

	obs_hotkey_pair_t *pair;
	HASH_FIND_HKEY(obs->hotkeys.hotkey_pairs, id, pair);
	if (!pair)
		goto unlock;

	obs_hotkey_t *hotkey;
	if (p_data0) {
		HASH_FIND_HKEY(obs->hotkeys.hotkeys, pair->id[0], hotkey);
		if (hotkey)
			*p_data0 = save_hotkey(hotkey);
	}
	if (p_data1) {
		HASH_FIND_HKEY(obs->hotkeys.hotkeys, pair->id[1], hotkey);
		if (hotkey)
			*p_data1 = save_hotkey(hotkey);
	}

unlock:
	unlock();
}

static inline bool enum_save_hotkey(void *data, obs_hotkey_t *hotkey)
{
	obs_data_array_t *hotkey_data = save_hotkey(hotkey);
	obs_data_set_array(data, hotkey->name, hotkey_data);
	obs_data_array_release(hotkey_data);
	return true;
}

static inline obs_data_t *save_context_hotkeys(struct obs_context_data *context)
{
	if (!context->hotkeys.num)
		return NULL;

	obs_data_t *result = obs_data_create();
	enum_context_hotkeys(context, enum_save_hotkey, result);
	return result;
}

obs_data_t *obs_hotkeys_save_encoder(obs_encoder_t *encoder)
{
	obs_data_t *result = NULL;

	if (!lock())
		return result;

	result = save_context_hotkeys(&encoder->context);
	unlock();

	return result;
}

obs_data_t *obs_hotkeys_save_output(obs_output_t *output)
{
	obs_data_t *result = NULL;

	if (!lock())
		return result;

	result = save_context_hotkeys(&output->context);
	unlock();

	return result;
}

obs_data_t *obs_hotkeys_save_service(obs_service_t *service)
{
	obs_data_t *result = NULL;

	if (!lock())
		return result;

	result = save_context_hotkeys(&service->context);
	unlock();

	return result;
}

obs_data_t *obs_hotkeys_save_source(obs_source_t *source)
{
	obs_data_t *result = NULL;

	if (!lock())
		return result;

	result = save_context_hotkeys(&source->context);
	unlock();

	return result;
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

static inline void release_pressed_binding(obs_hotkey_binding_t *binding);

static inline bool remove_bindings(obs_hotkey_id id)
{
	bool removed = false;
	size_t idx;
	while (find_binding(id, &idx)) {
		obs_hotkey_binding_t *binding = &obs->hotkeys.bindings.array[idx];

		if (binding->pressed)
			release_pressed_binding(binding);

		da_erase(obs->hotkeys.bindings, idx);
		removed = true;
	}

	return removed;
}

static void release_registerer(obs_hotkey_t *hotkey)
{
	switch (hotkey->registerer_type) {
	case OBS_HOTKEY_REGISTERER_FRONTEND:
		break;

	case OBS_HOTKEY_REGISTERER_ENCODER:
		obs_weak_encoder_release(hotkey->registerer);
		break;

	case OBS_HOTKEY_REGISTERER_OUTPUT:
		obs_weak_output_release(hotkey->registerer);
		break;

	case OBS_HOTKEY_REGISTERER_SERVICE:
		obs_weak_service_release(hotkey->registerer);
		break;

	case OBS_HOTKEY_REGISTERER_SOURCE:
		obs_weak_source_release(hotkey->registerer);
		break;
	}

	hotkey->registerer = NULL;
}

static inline void unregister_hotkey(obs_hotkey_id id)
{
	if (id >= obs->hotkeys.next_id)
		return;

	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		return;

	HASH_DEL(obs->hotkeys.hotkeys, hotkey);

	hotkey_signal("hotkey_unregister", hotkey);

	release_registerer(hotkey);

	if (hotkey->registerer_type == OBS_HOTKEY_REGISTERER_SOURCE)
		obs_weak_source_release(hotkey->registerer);

	bfree(hotkey->name);
	bfree(hotkey->description);
	bfree(hotkey);

	remove_bindings(id);
}

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

void obs_hotkey_unregister(obs_hotkey_id id)
{
	if (!lock())
		return;

	unregister_hotkey(id);
	unlock();
}

void obs_hotkey_pair_unregister(obs_hotkey_pair_id id)
{
	if (!lock())
		return;

	unregister_hotkey_pair(id);
	unlock();
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

void obs_enum_hotkeys(obs_hotkey_enum_func func, void *data)
{
	if (!lock())
		return;

	obs_hotkey_t *hk, *tmp;
	HASH_ITER (hh, obs->hotkeys.hotkeys, hk, tmp) {
		if (!func(data, hk->id, hk))
			break;
	}

	unlock();
}

void obs_enum_hotkey_bindings(obs_hotkey_binding_enum_func func, void *data)
{
	if (!lock())
		return;

	enum_bindings(func, data);
	unlock();
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

static inline void press_released_binding(obs_hotkey_binding_t *binding)
{
	binding->pressed = true;

	obs_hotkey_t *hotkey = binding->hotkey;
	if (hotkey->pressed++)
		return;

	if (!obs->hotkeys.reroute_hotkeys)
		hotkey->func(hotkey->data, hotkey->id, hotkey, true);
	else if (obs->hotkeys.router_func)
		obs->hotkeys.router_func(obs->hotkeys.router_func_data, hotkey->id, true);
}

static inline void release_pressed_binding(obs_hotkey_binding_t *binding)
{
	binding->pressed = false;

	obs_hotkey_t *hotkey = binding->hotkey;
	if (--hotkey->pressed)
		return;

	if (!obs->hotkeys.reroute_hotkeys)
		hotkey->func(hotkey->data, hotkey->id, hotkey, false);
	else if (obs->hotkeys.router_func)
		obs->hotkeys.router_func(obs->hotkeys.router_func_data, hotkey->id, false);
}

void obs_hotkey_binding_press(obs_hotkey_binding_t *binding)
{
	press_released_binding(binding);
}

void obs_hotkey_binding_release(obs_hotkey_binding_t *binding)
{
	release_pressed_binding(binding);
}

struct obs_hotkey_internal_inject {
	obs_key_combination_t hotkey;
	bool pressed;
	bool strict_modifiers;
};

static inline bool inject_hotkey(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	UNUSED_PARAMETER(idx);
	struct obs_hotkey_internal_inject *event = data;
	obs_hotkey_binding_inject(binding, event->hotkey, event->pressed, event->strict_modifiers);
	return true;
}

void obs_hotkey_inject_event(obs_key_combination_t hotkey, bool pressed)
{
	if (!lock())
		return;

	struct obs_hotkey_internal_inject event = {
		{hotkey.modifiers, hotkey.key},
		pressed,
		obs->hotkeys.strict_modifiers,
	};
	enum_bindings(inject_hotkey, &event);
	unlock();
}

void obs_hotkey_enable_background_press(bool enable)
{
	if (!lock())
		return;

	bool changed = obs->hotkeys.thread_disable_press == enable;
	obs->hotkeys.thread_disable_press = !enable;
	unlock();

	if (!changed)
		return;

	/* Platforms that have the desktop hold the keys for OBS (the
	 * GlobalShortcuts portal on Wayland) give them back while presses
	 * are disabled. */
	calldata_t data;
	uint8_t stack[128];
	calldata_init_fixed(&data, stack, sizeof(stack));
	calldata_set_bool(&data, "enabled", enable);
	signal_handler_signal(obs->hotkeys.signals, "hotkey_background_press_changed", &data);
}

void obs_hotkeys_background_clear_shortcuts(struct obs_hotkeys_background *background)
{
	for (size_t i = 0; i < background->shortcuts.num; i++)
		bfree(background->shortcuts.array[i].trigger);
	da_resize(background->shortcuts, 0);
}

void obs_hotkeys_background_status_changed(void)
{
	if (obs && obs->hotkeys.signals)
		signal_handler_signal(obs->hotkeys.signals, "hotkey_background_status_changed", NULL);
}

enum obs_hotkeys_background_state obs_hotkeys_get_background_state(uint32_t *flags)
{
	if (flags)
		*flags = 0;
	if (!lock())
		return OBS_HOTKEYS_BACKGROUND_NATIVE;

	enum obs_hotkeys_background_state state = obs->hotkeys.background.state;
	if (flags)
		*flags = obs->hotkeys.background.flags;
	unlock();
	return state;
}

char *obs_hotkeys_get_background_error(void)
{
	if (!lock())
		return NULL;

	char *error = obs->hotkeys.background.error ? bstrdup(obs->hotkeys.background.error) : NULL;
	unlock();
	return error;
}

const char *obs_hotkeys_get_background_app_id(void)
{
	if (!lock())
		return NULL;

	const char *app_id = obs->hotkeys.background.app_id;
	unlock();
	return app_id;
}

char *obs_hotkey_combination_get_background_id(obs_key_combination_t combo)
{
	if (!lock())
		return NULL;

	struct dstr id = {0};
	bool (*shortcut_id)(obs_key_combination_t, struct dstr *) = obs->hotkeys.background.shortcut_id;
	unlock();

	if (!shortcut_id || !shortcut_id(combo, &id)) {
		dstr_free(&id);
		return NULL;
	}
	return id.array;
}

char *obs_hotkey_combination_get_background_trigger(obs_key_combination_t combo)
{
	if (!lock())
		return NULL;

	struct dstr trigger = {0};
	struct obs_hotkeys_background *background = &obs->hotkeys.background;
	bool found = background->shortcut_trigger &&
		     background->shortcut_trigger(background->wake_data, combo, &trigger);
	unlock();

	if (!found) {
		dstr_free(&trigger);
		return NULL;
	}
	return trigger.array;
}

bool obs_hotkey_combination_is_background(obs_key_combination_t combo)
{
	if (obs_hotkeys_get_background_state(NULL) == OBS_HOTKEYS_BACKGROUND_NATIVE)
		return true;

	char *id = obs_hotkey_combination_get_background_id(combo);
	bool background = id != NULL;
	bfree(id);
	return background;
}

void obs_hotkeys_enum_background_shortcuts(obs_hotkey_background_enum_func func, void *data)
{
	if (!func || !lock())
		return;

	struct obs_hotkeys_background *background = &obs->hotkeys.background;
	for (size_t i = 0; i < background->shortcuts.num; i++) {
		struct obs_hotkey_background_shortcut *shortcut = &background->shortcuts.array[i];
		if (!func(data, shortcut->combo, shortcut->trigger, shortcut->bound))
			break;
	}
	unlock();
}

/* Lets the platform client act on a retry or probe change, then unlocks.
 * Called under the mutex, so the client cannot be stopped meanwhile. */
static void wake_background_client(void)
{
	if (obs->hotkeys.background.wake)
		obs->hotkeys.background.wake(obs->hotkeys.background.wake_data);
	unlock();
}

void obs_hotkeys_retry_background(void)
{
	if (!lock())
		return;

	obs->hotkeys.background.retry = true;
	wake_background_client();
}

void obs_hotkeys_configure_background(void)
{
	if (!lock())
		return;

	obs->hotkeys.background.configure = true;
	obs->hotkeys.background.flags &= ~OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED;
	wake_background_client();
}

void obs_hotkeys_set_background_probe(bool enable)
{
	if (!lock())
		return;

	if (obs->hotkeys.background.probe == enable) {
		unlock();
		return;
	}
	obs->hotkeys.background.probe = enable;
	/* A request to open the desktop's editor belongs to this probe. */
	if (!enable)
		obs->hotkeys.background.configure = false;
	wake_background_client();
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

void obs_hotkey_trigger_routed_callback(obs_hotkey_id id, bool pressed)
{
	if (!lock())
		return;

	if (!obs->hotkeys.reroute_hotkeys)
		goto unlock;

	obs_hotkey_t *hotkey;
	HASH_FIND_HKEY(obs->hotkeys.hotkeys, id, hotkey);
	if (!hotkey)
		goto unlock;

	hotkey->func(hotkey->data, id, hotkey, pressed);

unlock:
	unlock();
}

void obs_hotkey_set_callback_routing_func(obs_hotkey_callback_router_func func, void *data)
{
	if (!lock())
		return;

	obs->hotkeys.router_func = func;
	obs->hotkeys.router_func_data = data;
	unlock();
}

void obs_hotkey_enable_callback_rerouting(bool enable)
{
	if (!lock())
		return;

	obs->hotkeys.reroute_hotkeys = enable;
	unlock();
}

/* The key name store (obs->hotkeys.translations[]): obs_hotkeys_set_translations_s()
 * in obs-hotkey-translations.c.h calls it for each name it sets. Declared in
 * obs-internal.h; not exported. */
void obs_hotkeys_store_key_translation(obs_key_t key, const char *translation)
{
	bfree(obs->hotkeys.translations[key]);
	obs->hotkeys.translations[key] = NULL;

	if (translation)
		obs->hotkeys.translations[key] = bstrdup(translation);
}

const char *obs_get_hotkey_translation(obs_key_t key, const char *def)
{
	if (key == OBS_KEY_NONE) {
		return NULL;
	}

	return obs->hotkeys.translations[key] ? obs->hotkeys.translations[key] : def;
}

void obs_hotkey_update_atomic(obs_hotkey_atomic_update_func func, void *data)
{
	if (!lock())
		return;

	func(data);

	unlock();
}

void obs_hotkeys_set_audio_hotkeys_translations(const char *mute, const char *unmute, const char *push_to_mute,
						const char *push_to_talk, const char *monitor_on,
						const char *monitor_off)
{
#define SET_T(n)               \
	bfree(obs->hotkeys.n); \
	obs->hotkeys.n = bstrdup(n)
	SET_T(mute);
	SET_T(unmute);
	SET_T(push_to_mute);
	SET_T(push_to_talk);
	SET_T(monitor_on);
	SET_T(monitor_off);
#undef SET_T
}

void obs_hotkeys_set_sceneitem_hotkeys_translations(const char *show, const char *hide)
{
#define SET_T(n)                           \
	bfree(obs->hotkeys.sceneitem_##n); \
	obs->hotkeys.sceneitem_##n = bstrdup(n)
	SET_T(show);
	SET_T(hide);
#undef SET_T
}

/* This file hashes integer keys (HASH_FUNCTION above); restore util/uthash.h's
 * default for the sources a unity build compiles after it. */
#undef HASH_FUNCTION
#define HASH_FUNCTION HASH_SFH

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef HASH_ADD_HKEY
#undef HASH_FIND_HKEY
#undef NBSP
