#include "libobs/build/libobs.pch.h"

static bool hotkey_show_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed);
static bool hotkey_hide_sceneitem(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed);
static void sceneitem_rename_hotkey(const obs_sceneitem_t *item, const char *name);

bool obs_sceneitem_hotkey_trigger(obs_sceneitem_t *item, bool show, bool pressed, long (*owners)(obs_sceneitem_t *),
				  bool (*compare)(obs_sceneitem_t *, long *, long),
				  bool (*visible)(const obs_sceneitem_t *),
				  void (*set_visible)(obs_sceneitem_t *, bool), void (*release)(obs_sceneitem_t *));
void obs_sceneitem_hotkey_rename(size_t pair, const char *show, const char *hide, const char *name,
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

void obs_sceneitem_hotkey_rename(size_t pair, const char *show, const char *hide, const char *name,
				 void (*set)(size_t, const char *, const char *))
{
	UNUSED_PARAMETER(show);
	UNUSED_PARAMETER(hide);
	UNUSED_PARAMETER(set);
	struct obs_scene_item item = {0};
	item.toggle_visibility = pair;
	sceneitem_rename_hotkey(&item, name);
}
