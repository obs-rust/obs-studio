#include "libobs/build/libobs.pch.h"

extern bool obs_hotkey_seam_lock(void);
extern void obs_hotkey_seam_unlock(void);

/* Private adapters for the Rust combination background-id query. The callback
 * table, its mutex and dstr ownership stay in C. */
typedef bool (*obs_hotkey_combination_background_id_func)(obs_key_combination_t combo, struct dstr *id);

bool obs_hotkey_combination_background_id_lock(void);
bool obs_hotkey_combination_background_id_lock(void)
{
	return obs_hotkey_seam_lock();
}

/* Called only with the hotkey mutex held. */
const obs_hotkey_combination_background_id_func *obs_hotkey_combination_background_id_fields(void);
const obs_hotkey_combination_background_id_func *obs_hotkey_combination_background_id_fields(void)
{
	return &obs->hotkeys.background.shortcut_id;
}

void obs_hotkey_combination_background_id_unlock(void);
void obs_hotkey_combination_background_id_unlock(void)
{
	obs_hotkey_seam_unlock();
}

void obs_hotkey_combination_background_id_free(struct dstr *id);
void obs_hotkey_combination_background_id_free(struct dstr *id)
{
	dstr_free(id);
}
