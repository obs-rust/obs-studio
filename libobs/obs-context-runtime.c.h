#include "libobs/build/libobs.pch.h"

/* ensures that names are never blank (obs-context-names.c) */
static inline char *dup_name(const char *name, bool private)
{
	return obs_context_name_dup(name, private, &obs->data.unnamed_index);
}

static inline bool obs_context_data_init_wrap(struct obs_context_data *context, enum obs_obj_type type,
					      obs_data_t *settings, const char *name, const char *uuid,
					      obs_data_t *hotkey_data, bool private)
{
	assert(context);
	memset(context, 0, sizeof(*context));
	context->private = private;
	context->type = type;

	pthread_mutex_init_value(&context->rename_cache_mutex);
	if (pthread_mutex_init(&context->rename_cache_mutex, NULL) != 0)
		return false;

	context->signals = signal_handler_create();
	if (!context->signals)
		return false;

	context->procs = proc_handler_create();
	if (!context->procs)
		return false;

	if (uuid && strlen(uuid) == UUID_STR_LENGTH)
		context->uuid = bstrdup(uuid);
	/* Only automatically generate UUIDs for sources */
	else if (type == OBS_OBJ_TYPE_SOURCE || type == OBS_OBJ_TYPE_CANVAS)
		context->uuid = os_generate_uuid();

	context->name = dup_name(name, private);
	context->settings = obs_data_newref(settings);
	context->hotkey_data = obs_data_newref(hotkey_data);
	return true;
}

bool obs_context_data_init(struct obs_context_data *context, enum obs_obj_type type, obs_data_t *settings,
			   const char *name, const char *uuid, obs_data_t *hotkey_data, bool private)
{
	if (obs_context_data_init_wrap(context, type, settings, name, uuid, hotkey_data, private)) {
		return true;
	} else {
		obs_context_data_free(context);
		return false;
	}
}

void obs_context_data_free(struct obs_context_data *context)
{
	obs_hotkeys_context_release(context);
	signal_handler_destroy(context->signals);
	proc_handler_destroy(context->procs);
	obs_data_release(context->settings);
	obs_context_data_remove(context);
	pthread_mutex_destroy(&context->rename_cache_mutex);
	bfree(context->name);
	bfree((void *)(uintptr_t)context->uuid);

	for (size_t i = 0; i < context->rename_cache.num; i++)
		bfree(context->rename_cache.array[i]);
	da_free(context->rename_cache);

	memset(context, 0, sizeof(*context));
}

void obs_context_init_control(struct obs_context_data *context, void *object, obs_destroy_cb destroy)
{
	context->control = bzalloc(sizeof(obs_weak_object_t));
	context->control->object = object;
	context->destroy = destroy;
}

void obs_context_data_insert(struct obs_context_data *context, pthread_mutex_t *mutex, void *pfirst)
{
	struct obs_context_data **first = pfirst;

	assert(context);
	assert(mutex);
	assert(first);

	context->mutex = mutex;

	pthread_mutex_lock(mutex);
	context->prev_next = first;
	context->next = *first;
	*first = context;
	if (context->next)
		context->next->prev_next = &context->next;
	pthread_mutex_unlock(mutex);
}

static bool context_name_exists(void *phash, const char *name)
{
	struct obs_context_data *head = phash;
	struct obs_context_data *item = NULL;

	HASH_FIND_STR(head, name, item);
	return item != NULL;
}

/* NULL when the name is free, else the name made unique
 * (obs-context-names.c) */
static inline char *obs_context_deduplicate_name(void *phash, const char *name)
{
	return obs_context_name_deduplicate(name, context_name_exists, phash);
}

void obs_context_data_insert_name(struct obs_context_data *context, pthread_mutex_t *mutex, void *pfirst)
{
	struct obs_context_data **first = pfirst;
	char *new_name;

	assert(context);
	assert(mutex);
	assert(first);

	context->mutex = mutex;

	pthread_mutex_lock(mutex);

	/* Ensure name is not a duplicate. */
	new_name = obs_context_deduplicate_name(*first, context->name);
	if (new_name) {
		blog(LOG_WARNING,
		     "Attempted to insert context with duplicate name \"%s\"!"
		     " Name has been changed to \"%s\"",
		     context->name, new_name);
		/* Since this happens before the context creation finishes,
		 * do not bother to add it to the rename cache. */
		bfree(context->name);
		context->name = new_name;
	}

	HASH_ADD_STR(*first, name, context);

	pthread_mutex_unlock(mutex);
}

void obs_context_data_insert_uuid(struct obs_context_data *context, pthread_mutex_t *mutex, void *pfirst_uuid)
{
	struct obs_context_data **first_uuid = pfirst_uuid;
	struct obs_context_data *item = NULL;

	assert(context);
	assert(mutex);
	assert(first_uuid);

	context->mutex = mutex;

	pthread_mutex_lock(mutex);

	/* Ensure UUID is not a duplicate.
	 * This should only ever happen if a scene collection file has been
	 * manually edited and an entry has been duplicated without removing
	 * or regenerating the UUID. */
	HASH_FIND_UUID(*first_uuid, context->uuid, item);
	if (item) {
		blog(LOG_WARNING, "Attempted to insert context with duplicate UUID \"%s\"!", context->uuid);
		/* It is practically impossible for the new UUID to be a
		 * duplicate, so don't bother checking again. */
		bfree((void *)(uintptr_t)context->uuid);
		context->uuid = os_generate_uuid();
	}

	HASH_ADD_UUID(*first_uuid, uuid, context);
	pthread_mutex_unlock(mutex);
}

void obs_context_data_remove(struct obs_context_data *context)
{
	if (context && context->prev_next) {
		pthread_mutex_lock(context->mutex);
		*context->prev_next = context->next;
		if (context->next)
			context->next->prev_next = context->prev_next;
		context->prev_next = NULL;
		pthread_mutex_unlock(context->mutex);
	}
}

void obs_context_data_remove_name(struct obs_context_data *context, pthread_mutex_t *mutex, void *phead)
{
	struct obs_context_data **head = phead;

	assert(head);

	/* A context whose object failed to initialize was never added (its
	 * hash handle is still zeroed), and deleting it would corrupt or
	 * dereference the table. */
	if (!context || !context->hh.tbl)
		return;

	pthread_mutex_lock(mutex);
	HASH_DELETE(hh, *head, context);
	pthread_mutex_unlock(mutex);
}

void obs_context_data_remove_uuid(struct obs_context_data *context, pthread_mutex_t *mutex, void *puuid_head)
{
	struct obs_context_data **uuid_head = puuid_head;

	assert(uuid_head);

	if (!context || !context->uuid || !uuid_head)
		return;

	pthread_mutex_lock(mutex);
	HASH_DELETE(hh_uuid, *uuid_head, context);
	pthread_mutex_unlock(mutex);
}

void obs_context_wait(struct obs_context_data *context)
{
	/* Not in any list: its object failed to initialize. */
	if (!context->mutex)
		return;

	pthread_mutex_lock(context->mutex);
	pthread_mutex_unlock(context->mutex);
}

void obs_context_data_setname(struct obs_context_data *context, const char *name)
{
	pthread_mutex_lock(&context->rename_cache_mutex);

	if (context->name)
		da_push_back(context->rename_cache, &context->name);
	context->name = dup_name(name, context->private);

	pthread_mutex_unlock(&context->rename_cache_mutex);
}

void obs_context_data_setname_ht(struct obs_context_data *context, const char *name, void *phead)
{
	struct obs_context_data **head = phead;
	char *new_name;

	pthread_mutex_lock(context->mutex);
	pthread_mutex_lock(&context->rename_cache_mutex);

	HASH_DEL(*head, context);
	if (context->name)
		da_push_back(context->rename_cache, &context->name);

	/* Ensure new name is not a duplicate. A NULL name takes the unnamed
	 * name, as an empty one does. */
	new_name = name ? obs_context_deduplicate_name(*head, name) : NULL;
	if (new_name) {
		blog(LOG_WARNING,
		     "Attempted to rename context to duplicate name \"%s\"!"
		     " New name has been changed to \"%s\"",
		     name, new_name);
		context->name = new_name;
	} else {
		context->name = dup_name(name, context->private);
	}

	HASH_ADD_STR(*head, name, context);

	pthread_mutex_unlock(&context->rename_cache_mutex);
	pthread_mutex_unlock(context->mutex);
}

profiler_name_store_t *obs_get_profiler_name_store(void)
{
	return obs->name_store;
}

void *obs_obj_get_data(void *obj)
{
	struct obs_context_data *context = obj;
	if (!context)
		return NULL;

	return context->data;
}
