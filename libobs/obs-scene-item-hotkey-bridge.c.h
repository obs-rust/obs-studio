#include "libobs/build/libobs.pch.h"

long obs_sceneitem_hotkey_owners(obs_sceneitem_t *item);
bool obs_sceneitem_hotkey_compare(obs_sceneitem_t *item, long *expected, long next);
bool obs_sceneitem_user_visible_load(const obs_sceneitem_t *item);
void obs_sceneitem_hotkey_set_visible(obs_sceneitem_t *item, bool visible);
void obs_sceneitem_hotkey_release(obs_sceneitem_t *item);
size_t obs_sceneitem_hotkey_pair(const obs_sceneitem_t *item);
const char *obs_sceneitem_hotkey_template(bool show);
const char *obs_sceneitem_hotkey_new_name(calldata_t *data);
void obs_sceneitem_hotkey_descriptions(size_t pair, const char *show, const char *hide);

bool obs_sceneitem_hotkey_trigger(obs_sceneitem_t *item, bool show, bool pressed, long (*owners)(obs_sceneitem_t *),
				  bool (*compare)(obs_sceneitem_t *, long *, long),
				  bool (*visible)(const obs_sceneitem_t *),
				  void (*set_visible)(obs_sceneitem_t *, bool), void (*release)(obs_sceneitem_t *));
void obs_sceneitem_hotkey_rename(obs_sceneitem_t *item, calldata_t *data, const char *(*new_name)(calldata_t *),
				 const char *(*template_of)(bool), size_t (*pair_of)(const obs_sceneitem_t *),
				 void (*set)(size_t, const char *, const char *));

static bool obs_sceneitem_hotkey_show(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	return obs_sceneitem_hotkey_trigger(data, true, pressed, obs_sceneitem_hotkey_owners,
					    obs_sceneitem_hotkey_compare, obs_sceneitem_user_visible_load,
					    obs_sceneitem_hotkey_set_visible, obs_sceneitem_hotkey_release);
}

static bool obs_sceneitem_hotkey_hide(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	return obs_sceneitem_hotkey_trigger(data, false, pressed, obs_sceneitem_hotkey_owners,
					    obs_sceneitem_hotkey_compare, obs_sceneitem_user_visible_load,
					    obs_sceneitem_hotkey_set_visible, obs_sceneitem_hotkey_release);
}

static void obs_sceneitem_hotkey_renamed(void *param, calldata_t *data)
{
	obs_sceneitem_hotkey_rename(param, data, obs_sceneitem_hotkey_new_name, obs_sceneitem_hotkey_template,
				    obs_sceneitem_hotkey_pair, obs_sceneitem_hotkey_descriptions);
}
