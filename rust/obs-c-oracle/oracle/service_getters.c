/* Test-only oracle: the original libobs/obs-service-getters.c.h, unmodified,
 * with its exports renamed to oracle_* so it can link next to the Rust
 * shims (#1899, #1913, #1920). find_service() and the context accessors are
 * test doubles that both sides call. obs_service_get_info_table() and
 * obs_service_get_plugin_data() are copied from obs-service.c,
 * and oracle_service_new() builds the struct obs_service both sides read,
 * so the parity tests drive the C and the Rust with the same service. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#define obs_service_get_supported_resolutions oracle_obs_service_get_supported_resolutions
#define obs_service_get_max_fps oracle_obs_service_get_max_fps
#define obs_service_get_max_bitrate oracle_obs_service_get_max_bitrate
#define obs_service_get_supported_video_codecs oracle_obs_service_get_supported_video_codecs
#define obs_service_get_supported_audio_codecs oracle_obs_service_get_supported_audio_codecs
#define obs_service_get_protocol oracle_obs_service_get_protocol
#define obs_service_get_preferred_output_type oracle_obs_service_get_preferred_output_type
#define obs_service_get_connect_info oracle_obs_service_get_connect_info
#define obs_service_can_try_to_connect oracle_obs_service_can_try_to_connect
#define obs_service_get_display_name oracle_obs_service_get_display_name
#define obs_service_get_name oracle_obs_service_get_name
#define obs_service_defaults oracle_obs_service_defaults
#define obs_get_service_properties oracle_obs_get_service_properties
#define obs_service_properties oracle_obs_service_properties
#define obs_service_get_type oracle_obs_service_get_type
#define obs_service_update oracle_obs_service_update
#define obs_service_get_settings oracle_obs_service_get_settings
#define obs_service_apply_encoder_settings oracle_obs_service_apply_encoder_settings
#define obs_service_get_type_data oracle_obs_service_get_type_data
#define obs_service_get_id oracle_obs_service_get_id
#define obs_service_get_module oracle_obs_service_get_module
#define obs_service_load_state oracle_obs_service_load_state
#define obs_service_create oracle_obs_service_create
#define obs_service_create_private oracle_obs_service_create_private
#define obs_service_destroy oracle_obs_service_destroy
#define obs_service_activate oracle_obs_service_activate
#define obs_service_deactivate oracle_obs_service_deactivate
#define obs_service_initialize oracle_obs_service_initialize
#define obs_service_release oracle_obs_service_release
#define obs_weak_service_addref oracle_obs_weak_service_addref
#define obs_weak_service_release oracle_obs_weak_service_release
#define obs_service_get_ref oracle_obs_service_get_ref
#define obs_service_get_weak_service oracle_obs_service_get_weak_service
#define obs_weak_service_get_service oracle_obs_weak_service_get_service
#define obs_weak_service_references_service oracle_obs_weak_service_references_service

#include "obs-service-getters.c.h"

#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* obs-service.c */
const struct obs_service_info *obs_service_get_info_table(const obs_service_t *service)
{
	return &service->info;
}

void *obs_service_get_plugin_data(const obs_service_t *service)
{
	return service->context.data;
}

const char *obs_service_get_context_name(const obs_service_t *service)
{
	return service->context.name;
}

obs_data_t *obs_service_get_context_settings(const obs_service_t *service)
{
	return service->context.settings;
}

/* The steps on struct obs_service, its context and its reference counts
 * that the lifecycle (#1920) takes, as test doubles both sides call. Each
 * reports to oracle_service_trace() (the test's hook) with the service's
 * name, so a C run and a Rust run give comparable traces. The reference
 * counts are libobs' own inline obs_ref_* / obs_weak_ref_*. */
static void (*oracle_trace_hook)(const char *event, const char *name);

void oracle_service_set_trace(void (*hook)(const char *event, const char *name));
void oracle_service_set_trace(void (*hook)(const char *event, const char *name))
{
	oracle_trace_hook = hook;
}

static void trace_ev(const char *event, const obs_service_t *service)
{
	if (oracle_trace_hook)
		oracle_trace_hook(event, service ? service->context.name : NULL);
}

void oracle_module_lookup_set_roots(obs_module_t *enabled, obs_module_t *disabled);
void oracle_service_set_modules(obs_module_t *first, obs_module_t *first_disabled);
void oracle_service_set_modules(obs_module_t *first, obs_module_t *first_disabled)
{
	oracle_module_lookup_set_roots(first, first_disabled);
}

/* A module that registered `num` service ids; `next` links the list. */
obs_module_t *oracle_module_new(const char **ids, size_t num, int load_state, obs_module_t *next);
obs_module_t *oracle_module_new(const char **ids, size_t num, int load_state, obs_module_t *next)
{
	struct obs_module *module = calloc(1, sizeof(*module));
	module->services.array = (char **)(uintptr_t)ids;
	module->services.num = num;
	module->load_state = (enum obs_module_load_state)load_state;
	module->next = next;
	return module;
}

void oracle_module_free(obs_module_t *module);
void oracle_module_free(obs_module_t *module)
{
	free(module);
}

static bool oracle_context_fails;

void oracle_service_set_context_fails(bool fails);
void oracle_service_set_context_fails(bool fails)
{
	oracle_context_fails = fails;
}

/* No libobs context: the name is the caller's string, the settings the
 * caller's object. */
obs_service_t *obs_service_context_new(const char *name, obs_data_t *settings, obs_data_t *hotkey_data, bool private)
{
	(void)hotkey_data;
	if (oracle_trace_hook)
		oracle_trace_hook(private ? "context_new private" : "context_new", name);
	if (oracle_context_fails)
		return NULL;
	struct obs_service *service = calloc(1, sizeof(*service));
	service->context.name = (char *)(uintptr_t)name;
	service->context.settings = settings;
	return service;
}

void obs_service_set_info(obs_service_t *service, const struct obs_service_info *info)
{
	trace_ev("set_info", service);
	service->info = *info;
}

void obs_service_set_plugin_data(obs_service_t *service, void *data)
{
	trace_ev(data ? "set_data" : "set_data NULL", service);
	service->context.data = data;
}

/* obs_context_init_control()'s control object, then the insert. */
void obs_service_context_insert(obs_service_t *service)
{
	trace_ev("insert", service);
	struct obs_weak_service *control = calloc(1, sizeof(*control));
	control->service = service;
	service->context.control = (obs_weak_object_t *)control;
}

void obs_service_context_remove(obs_service_t *service)
{
	trace_ev("remove", service);
}

/* Traced, not freed: the test frees the service afterwards. */
void obs_service_context_free(obs_service_t *service)
{
	trace_ev("free", service);
}

/* State accessors now resolve to the real Rust seam (#3840).
 * Independent C state parity lives in oracle/service_state.c. */
bool obs_weak_service_strong_release(obs_weak_service_t *weak)
{
	trace_ev("ref_release", weak->service);
	return obs_ref_release(&weak->ref);
}

void obs_weak_service_weak_addref(obs_weak_service_t *weak)
{
	trace_ev("weak_addref", weak->service);
	obs_weak_ref_addref(&weak->ref);
}

bool obs_weak_service_weak_release(obs_weak_service_t *weak)
{
	trace_ev("weak_release", weak->service);
	return obs_weak_ref_release(&weak->ref);
}

void obs_weak_service_free(obs_weak_service_t *weak)
{
	trace_ev("weak_free", weak->service);
	free(weak);
}

/* What the tests read and set on a service. */
void oracle_service_set_state(obs_service_t *service, bool active, void *output);
void oracle_service_set_state(obs_service_t *service, bool active, void *output)
{
	service->active = active;
	service->output = output;
}

bool oracle_service_is_destroying(const obs_service_t *service);
bool oracle_service_is_destroying(const obs_service_t *service)
{
	return service->destroy;
}

bool oracle_service_is_active(const obs_service_t *service);
bool oracle_service_is_active(const obs_service_t *service)
{
	return service->active;
}

void *oracle_service_output(const obs_service_t *service);
void *oracle_service_output(const obs_service_t *service)
{
	return service->output;
}

void *oracle_service_control(const obs_service_t *service);
void *oracle_service_control(const obs_service_t *service)
{
	return service->context.control;
}

/* obs-service.c's find_service() over obs->service_types, here over one
 * table the test sets (NULL: no types). */
static const struct obs_service_info *oracle_types;
static size_t oracle_types_num;

void oracle_service_set_types(const struct obs_service_info *types, size_t num);
void oracle_service_set_types(const struct obs_service_info *types, size_t num)
{
	oracle_types = types;
	oracle_types_num = num;
}

const struct obs_service_info *find_service(const char *id)
{
	for (size_t i = 0; i < oracle_types_num; i++)
		if (strcmp(oracle_types[i].id, id) == 0)
			return oracle_types + i;
	return NULL;
}

/* The context fields #1913 reads; the plugin data and table are set by
 * oracle_service_new(). */
void oracle_service_set_context(struct obs_service *service, char *name, obs_data_t *settings);
void oracle_service_set_context(struct obs_service *service, char *name, obs_data_t *settings)
{
	service->context.name = name;
	service->context.settings = settings;
}

struct obs_service *oracle_service_new(const struct obs_service_info *info, void *data);
struct obs_service *oracle_service_new(const struct obs_service_info *info, void *data)
{
	struct obs_service *service = calloc(1, sizeof(*service));
	service->info = *info;
	service->context.data = data;
	return service;
}

void oracle_service_free(struct obs_service *service);
void oracle_service_free(struct obs_service *service)
{
	free(service);
}

#define SIZE_ALIGN(ty, stem)                        \
	size_t oracle_##stem##_size(void);          \
	size_t oracle_##stem##_size(void)           \
	{                                           \
		return sizeof(ty);                  \
	}                                           \
	size_t oracle_##stem##_align(void);         \
	size_t oracle_##stem##_align(void)          \
	{                                           \
		return _Alignof(ty);                \
	}

#define FIELD(ty, stem, field)                        \
	size_t oracle_##stem##_offset_##field(void);  \
	size_t oracle_##stem##_offset_##field(void)   \
	{                                             \
		return offsetof(ty, field);           \
	}

SIZE_ALIGN(struct obs_service_resolution, service_resolution)
FIELD(struct obs_service_resolution, service_resolution, cy)

SIZE_ALIGN(struct obs_service_info, service_info)
FIELD(struct obs_service_info, service_info, id)
FIELD(struct obs_service_info, service_info, get_name)
FIELD(struct obs_service_info, service_info, create)
FIELD(struct obs_service_info, service_info, destroy)
FIELD(struct obs_service_info, service_info, activate)
FIELD(struct obs_service_info, service_info, deactivate)
FIELD(struct obs_service_info, service_info, update)
FIELD(struct obs_service_info, service_info, get_defaults)
FIELD(struct obs_service_info, service_info, get_properties)
FIELD(struct obs_service_info, service_info, initialize)
FIELD(struct obs_service_info, service_info, get_url)
FIELD(struct obs_service_info, service_info, get_key)
FIELD(struct obs_service_info, service_info, get_username)
FIELD(struct obs_service_info, service_info, get_password)
FIELD(struct obs_service_info, service_info, deprecated_1)
FIELD(struct obs_service_info, service_info, apply_encoder_settings)
FIELD(struct obs_service_info, service_info, type_data)
FIELD(struct obs_service_info, service_info, free_type_data)
FIELD(struct obs_service_info, service_info, get_output_type)
FIELD(struct obs_service_info, service_info, get_supported_resolutions)
FIELD(struct obs_service_info, service_info, get_max_fps)
FIELD(struct obs_service_info, service_info, get_max_bitrate)
FIELD(struct obs_service_info, service_info, get_supported_video_codecs)
FIELD(struct obs_service_info, service_info, get_protocol)
FIELD(struct obs_service_info, service_info, get_supported_audio_codecs)
FIELD(struct obs_service_info, service_info, get_connect_info)
FIELD(struct obs_service_info, service_info, can_try_to_connect)
