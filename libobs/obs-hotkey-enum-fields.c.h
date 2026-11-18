#include "libobs/build/libobs.pch.h"

/* Private layout access only; Rust owns the walk and callback decisions. */
obs_hotkey_t *obs_hotkey_enum_first(void);
obs_hotkey_t *obs_hotkey_enum_first(void)
{
	return obs->hotkeys.hotkeys;
}

obs_hotkey_t *obs_hotkey_enum_next(const obs_hotkey_t *key);
obs_hotkey_t *obs_hotkey_enum_next(const obs_hotkey_t *key)
{
	return key->hh.next;
}

obs_hotkey_id obs_hotkey_enum_id(const obs_hotkey_t *key);
obs_hotkey_id obs_hotkey_enum_id(const obs_hotkey_t *key)
{
	return key->id;
}

const struct darray *obs_hotkey_enum_context_fields(const struct obs_context_data *context);
const struct darray *obs_hotkey_enum_context_fields(const struct obs_context_data *context)
{
	return &context->hotkeys.da;
}
