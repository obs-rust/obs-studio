#include "libobs/build/libobs.pch.h"

static bool hotkey_show_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed);
static bool hotkey_hide_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed);
static void sceneitem_renamed(void *param, calldata_t *data);

bool obs_sceneitem_hotkey_trigger(obs_sceneitem_t *item, bool show, bool pressed, long (*owners)(obs_sceneitem_t *),
				  bool (*compare)(obs_sceneitem_t *, long *, long),
				  bool (*visible)(const obs_sceneitem_t *),
				  void (*set_visible)(obs_sceneitem_t *, bool), void (*release)(obs_sceneitem_t *));
void obs_sceneitem_hotkey_rename(obs_sceneitem_t *item, calldata_t *data, const char *(*new_name)(calldata_t *),
				 const char *(*template_of)(bool), size_t (*pair_of)(const obs_sceneitem_t *),
				 void (*set)(size_t, const char *, const char *));

bool obs_sceneitem_hotkey_trigger(obs_sceneitem_t *item, bool show, bool pressed, long (*owners)(obs_sceneitem_t *),
				  bool (*compare)(obs_sceneitem_t *, long *, long),
				  bool (*visible)(const obs_sceneitem_t *),
				  void (*set_visible)(obs_sceneitem_t *, bool), void (*release)(obs_sceneitem_t *))
{
	UNUSED_PARAMETER(owners);
	UNUSED_PARAMETER(compare);
	UNUSED_PARAMETER(visible);
	UNUSED_PARAMETER(set_visible);
	UNUSED_PARAMETER(release);
	return (show ? hotkey_show_sceneitem : hotkey_hide_sceneitem)(item, 0, NULL, pressed);
}

void obs_sceneitem_hotkey_rename(obs_sceneitem_t *item, calldata_t *data, const char *(*new_name)(calldata_t *),
				 const char *(*template_of)(bool), size_t (*pair_of)(const obs_sceneitem_t *),
				 void (*set)(size_t, const char *, const char *))
{
	UNUSED_PARAMETER(new_name);
	UNUSED_PARAMETER(template_of);
	UNUSED_PARAMETER(pair_of);
	UNUSED_PARAMETER(set);
	sceneitem_renamed(item, data);
}
