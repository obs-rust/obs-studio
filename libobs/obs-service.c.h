/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

#define get_weak(service) ((obs_weak_service_t *)service->context.control)

const struct obs_service_info *find_service(const char *id)
{
	size_t i;
	for (i = 0; i < obs->service_types.num; i++)
		if (strcmp(obs->service_types.array[i].id, id) == 0)
			return obs->service_types.array + i;

	return NULL;
}

/* The service's lifecycle, references, module lookup and activation moved
 * to obs-service-getters.c (#1920). These are the steps they take on struct
 * obs_service, its context and its reference counts, which stay here with
 * the struct; the Rust port (obs-output's service module) calls the same
 * ones. Hidden. */
obs_module_t *obs_service_module_first(bool disabled)
{
	return disabled ? obs->first_disabled_module : obs->first_module;
}

obs_module_t *obs_service_module_next(obs_module_t *module)
{
	return module->next;
}

size_t obs_service_module_service_count(obs_module_t *module)
{
	return module->services.num;
}

const char *obs_service_module_service_id(obs_module_t *module, size_t i)
{
	return module->services.array[i];
}

enum obs_module_load_state obs_service_module_load_state(obs_module_t *module)
{
	return module->load_state;
}

/* A zeroed service with its context set up, or NULL (freed) if that fails. */
obs_service_t *obs_service_context_new(const char *name, obs_data_t *settings, obs_data_t *hotkey_data, bool private)
{
	struct obs_service *service = bzalloc(sizeof(struct obs_service));

	if (!obs_context_data_init(&service->context, OBS_OBJ_TYPE_SERVICE, settings, name, NULL, hotkey_data,
				   private)) {
		bfree(service);
		return NULL;
	}
	return service;
}

void obs_service_set_info(obs_service_t *service, const struct obs_service_info *info)
{
	service->info = *info;
}

void obs_service_set_plugin_data(obs_service_t *service, void *data)
{
	service->context.data = data;
}

void obs_service_context_insert(obs_service_t *service)
{
	obs_context_init_control(&service->context, service, (obs_destroy_cb)obs_service_destroy);
	obs_context_data_insert(&service->context, &obs->data.services_mutex, &obs->data.first_service);
}

void obs_service_context_remove(obs_service_t *service)
{
	obs_context_data_remove(&service->context);
}

/* The context, the owned id and the service itself. */
void obs_service_context_free(obs_service_t *service)
{
	obs_context_data_free(&service->context);
	if (service->owns_info_id)
		bfree((void *)(uintptr_t)service->info.id);
	bfree(service);
}

void obs_service_set_destroy(obs_service_t *service)
{
	service->destroy = true;
}

bool obs_service_is_destroying(const obs_service_t *service)
{
	return service->destroy;
}

bool obs_service_is_active(const obs_service_t *service)
{
	return service->active;
}

void obs_service_set_active(obs_service_t *service, bool active)
{
	service->active = active;
}

bool obs_service_has_output(const obs_service_t *service)
{
	return service->output != NULL;
}

void obs_service_clear_output(obs_service_t *service)
{
	service->output = NULL;
}

/* Only called with an output. */
void obs_service_detach_from_output(obs_service_t *service)
{
	service->output->service = NULL;
}

obs_weak_service_t *obs_service_get_control(const obs_service_t *service)
{
	return get_weak(service);
}

bool obs_weak_service_strong_release(obs_weak_service_t *weak)
{
	return obs_ref_release(&weak->ref);
}

void obs_weak_service_weak_addref(obs_weak_service_t *weak)
{
	obs_weak_ref_addref(&weak->ref);
}

bool obs_weak_service_weak_release(obs_weak_service_t *weak)
{
	return obs_weak_ref_release(&weak->ref);
}

bool obs_weak_service_weak_get_ref(obs_weak_service_t *weak)
{
	return obs_weak_ref_get_ref(&weak->ref);
}

obs_service_t *obs_weak_service_target(const obs_weak_service_t *weak)
{
	return weak->service;
}

void obs_weak_service_free(obs_weak_service_t *weak)
{
	bfree(weak);
}

signal_handler_t *obs_service_get_signal_handler(const obs_service_t *service);
signal_handler_t *obs_service_get_signal_handler(const obs_service_t *service)
{
	return obs_service_valid(service, "obs_service_get_signal_handler") ? service->context.signals : NULL;
}

proc_handler_t *obs_service_get_proc_handler(const obs_service_t *service);
proc_handler_t *obs_service_get_proc_handler(const obs_service_t *service)
{
	return obs_service_valid(service, "obs_service_get_proc_handler") ? service->context.procs : NULL;
}

/* For the getters in obs-service-getters.c, which no longer see struct
 * obs_service: the plugin's callback table this service was created with.
 * Hidden; the Rust port calls it and obs_service_get_plugin_data() too
 * (#1899). */
const struct obs_service_info *obs_service_get_info_table(const obs_service_t *service)
{
	return &service->info;
}

/* The plugin data that create() returned, for the same getters. Hidden. */
void *obs_service_get_plugin_data(const obs_service_t *service)
{
	return service->context.data;
}

/* The context fields the settings and identity functions in
 * obs-service-getters.c read, for them and their Rust port (#1913). Hidden. */
const char *obs_service_get_context_name(const obs_service_t *service)
{
	return service->context.name;
}

obs_data_t *obs_service_get_context_settings(const obs_service_t *service)
{
	return service->context.settings;
}

/* File-local macros: keep them out of the next source of a unity build. */
#undef get_weak
