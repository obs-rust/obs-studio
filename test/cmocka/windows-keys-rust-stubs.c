/* Link stand-ins for test_windows_keys_steps_rust. The executable takes the
 * Windows key map shims from the production staticlib (libobs_rust-static),
 * whose obs-hotkey and obs-util members also reference libobs's hidden
 * hotkey seams (obs-hotkey.c) and the C halves of the logging port
 * (util/base-variadic.c). libobs links those but does not export them. The
 * shims under test call none of them, so each stand-in aborts. */
#include <stdlib.h>

#include "obs-hotkey-seam.h"
#include "obs-internal.h"

log_handler_t base_default_log_handler(void);
void base_log_literal(log_handler_t handler, void *param, int level, const char *msg);

log_handler_t base_default_log_handler(void)
{
	abort();
}

void base_log_literal(log_handler_t handler, void *param, int level, const char *msg)
{
	(void)handler;
	(void)param;
	(void)level;
	(void)msg;
	abort();
}

struct obs_hotkey_binding_fields obs_hotkey_binding_fields(obs_hotkey_binding_t *binding)
{
	(void)binding;
	abort();
}

bool obs_hotkey_key_is_pressed(obs_key_t key)
{
	(void)key;
	abort();
}

struct obs_hotkey_binding_count_fields obs_hotkey_binding_count_fields(obs_hotkey_binding_t *binding)
{
	(void)binding;
	abort();
}

void obs_hotkey_dispatch(obs_hotkey_t *hotkey, bool pressed)
{
	(void)hotkey;
	(void)pressed;
	abort();
}

struct obs_hotkey_pair_fields obs_hotkey_pair_fields(obs_hotkey_pair_t *pair)
{
	(void)pair;
	abort();
}

bool obs_hotkey_pair_call_func(obs_hotkey_pair_t *pair, size_t half, obs_hotkey_t *hotkey, bool pressed)
{
	(void)pair;
	(void)half;
	(void)hotkey;
	(void)pressed;
	abort();
}

void obs_hotkeys_store_key_translation(obs_key_t key, const char *translation)
{
	(void)key;
	(void)translation;
	abort();
}

void obs_hotkeys_mutex_lock(void)
{
	abort();
}

void obs_hotkeys_mutex_unlock(void)
{
	abort();
}

bool obs_hotkey_seam_lock(void)
{
	abort();
}

void obs_hotkey_seam_unlock(void)
{
	abort();
}

obs_hotkey_t *obs_hotkey_seam_find(obs_hotkey_id id)
{
	(void)id;
	abort();
}

obs_hotkey_pair_t *obs_hotkey_seam_find_pair(obs_hotkey_pair_id id)
{
	(void)id;
	abort();
}

obs_hotkey_id obs_hotkey_seam_pair_id(const obs_hotkey_pair_t *pair, size_t half)
{
	(void)pair;
	(void)half;
	abort();
}

void obs_hotkey_seam_add_binding(obs_hotkey_t *hotkey, obs_key_combination_t combo)
{
	(void)hotkey;
	(void)combo;
	abort();
}

bool obs_hotkey_seam_remove_bindings(obs_hotkey_id id)
{
	(void)id;
	abort();
}

void obs_hotkey_seam_signal(const char *signal, obs_hotkey_t *hotkey)
{
	(void)signal;
	(void)hotkey;
	abort();
}

void obs_hotkey_seam_enum_bindings(obs_hotkey_binding_internal_enum_func func, void *data)
{
	(void)func;
	(void)data;
	abort();
}

void obs_hotkey_seam_enum_context(struct obs_context_data *context, obs_hotkey_internal_enum_func func, void *data)
{
	(void)context;
	(void)func;
	(void)data;
	abort();
}

size_t obs_hotkey_seam_context_count(const struct obs_context_data *context)
{
	(void)context;
	abort();
}

bool obs_hotkey_seam_saved_volume_keys_swapped(void)
{
	abort();
}

struct obs_context_data *obs_hotkey_seam_encoder_context(obs_encoder_t *encoder)
{
	(void)encoder;
	abort();
}

struct obs_context_data *obs_hotkey_seam_output_context(obs_output_t *output)
{
	(void)output;
	abort();
}

struct obs_context_data *obs_hotkey_seam_service_context(obs_service_t *service)
{
	(void)service;
	abort();
}

struct obs_context_data *obs_hotkey_seam_source_context(obs_source_t *source)
{
	(void)source;
	abort();
}
