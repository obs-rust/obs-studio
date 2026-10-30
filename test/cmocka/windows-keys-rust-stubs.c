/* Link stand-ins for test_windows_keys_steps_rust. The executable takes the
 * Windows key map shims from the production staticlib (libobs_rust-static),
 * whose obs-hotkey and obs-util members also reference libobs's hidden
 * hotkey seams (obs-hotkey.c) and the C halves of the logging port
 * (util/base-variadic.c). libobs links those but does not export them. The
 * shims under test call none of them, so each stand-in aborts. */
#include <stdlib.h>

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

void obs_hotkey_binding_press(obs_hotkey_binding_t *binding)
{
	(void)binding;
	abort();
}

void obs_hotkey_binding_release(obs_hotkey_binding_t *binding)
{
	(void)binding;
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
