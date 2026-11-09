#include "libobs/build/libobs.pch.h"

/* Keep the extracted bodies verbatim; dependencies stay in obs-hotkey.c.h. */
#define lock obs_hotkey_seam_lock
#define unlock obs_hotkey_seam_unlock
#define enum_bindings obs_hotkey_seam_enum_bindings
#define enum_context_hotkeys hkenum_enum_context_hotkeys
#define HASH_FIND_HKEY(head, id, out) HASH_FIND(hh, head, &(id), sizeof(size_t), out)

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

void obs_hotkey_seam_enum_context(struct obs_context_data *context, obs_hotkey_internal_enum_func func, void *data)
{
	enum_context_hotkeys(context, func, data);
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

#undef HASH_FIND_HKEY
#undef enum_context_hotkeys
#undef enum_bindings
#undef unlock
#undef lock
