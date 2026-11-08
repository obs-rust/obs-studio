/* Original C query, with every external operation supplied by the caller. */
#include "libobs/build/libobs-ported-hotkey-binding-press.pch.h"
#include <stddef.h>

bool oracle_hotkey_background_query(obs_key_combination_t combo, void *context, int (*query_state)(void *, uint32_t *),
				    char *(*query_id)(void *, obs_key_combination_t), void (*release)(void *, void *));
#define obs_hotkey_combination_is_background(combo) \
	oracle_hotkey_background_query(combo, void *context, int (*query_state)(void *, uint32_t *), \
				      char *(*query_id)(void *, obs_key_combination_t), \
				      void (*release)(void *, void *))
#define obs_hotkeys_get_background_state(flags) query_state(context, flags)
#define obs_hotkey_combination_get_background_id(combo) query_id(context, combo)
#define bfree(ptr) release(context, ptr)
#include "libobs/obs-hotkey-background-query.c.h"
#undef bfree
#undef obs_hotkey_combination_get_background_id
#undef obs_hotkeys_get_background_state
#undef obs_hotkey_combination_is_background

void oracle_hotkey_background_query_layout(size_t *out);
void oracle_hotkey_background_query_layout(size_t *out)
{
	out[0] = sizeof(obs_key_combination_t);
	out[1] = _Alignof(obs_key_combination_t);
	out[2] = offsetof(obs_key_combination_t, modifiers);
	out[3] = offsetof(obs_key_combination_t, key);
	out[4] = OBS_HOTKEYS_BACKGROUND_NATIVE;
	out[5] = OBS_KEY_LAST_VALUE;
}
