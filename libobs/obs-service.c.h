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

/* The service's lifecycle, references, module lookup and activation moved
 * to obs-service-getters.c (#1920). These are the steps they take on struct
 * obs_service, its context and its reference counts, which stay here with
 * the struct; the Rust port (obs-output's service module) calls the same
 * ones. Hidden. */
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

/* File-local macros: keep them out of the next source of a unity build. */
#undef get_weak

/* Expose global roots together; Rust selects the enabled/disabled list.
 * Module storage remains owned and synchronized by libobs. Hidden. */
void obs_service_module_roots(obs_module_t **enabled, obs_module_t **disabled);
void obs_service_module_roots(obs_module_t **enabled, obs_module_t **disabled)
{
	*enabled = obs->first_module;
	*disabled = obs->first_disabled_module;
}
