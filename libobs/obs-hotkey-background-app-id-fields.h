#pragma once

#include <stdbool.h>

/* Private field access: called only with the hotkey mutex held. */
extern bool obs_hotkey_background_app_id_lock(void);
extern const char *const *obs_hotkey_background_app_id_fields(void);
extern void obs_hotkey_background_app_id_unlock(void);
