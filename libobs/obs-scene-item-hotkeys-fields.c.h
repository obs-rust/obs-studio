#include "libobs/build/libobs.pch.h"

long obs_sceneitem_hotkey_owners(obs_sceneitem_t *item);
bool obs_sceneitem_hotkey_compare(obs_sceneitem_t *item, long *expected, long next);
size_t obs_sceneitem_hotkey_pair(const obs_sceneitem_t *item);
const char *obs_sceneitem_hotkey_template(bool show);
const char *obs_sceneitem_hotkey_new_name(calldata_t *data);
void obs_sceneitem_hotkey_release(obs_sceneitem_t *item);
void obs_sceneitem_hotkey_set_visible(obs_sceneitem_t *item, bool visible);
void obs_sceneitem_hotkey_descriptions(size_t pair, const char *show, const char *hide);

long obs_sceneitem_hotkey_owners(obs_sceneitem_t *item)
{
	return os_atomic_load_long(&item->ref);
}

bool obs_sceneitem_hotkey_compare(obs_sceneitem_t *item, long *expected, long next)
{
	return os_atomic_compare_exchange_long(&item->ref, expected, next);
}

size_t obs_sceneitem_hotkey_pair(const obs_sceneitem_t *item)
{
	return item->toggle_visibility;
}

const char *obs_sceneitem_hotkey_template(bool show)
{
	return show ? obs->hotkeys.sceneitem_show : obs->hotkeys.sceneitem_hide;
}

const char *obs_sceneitem_hotkey_new_name(calldata_t *data)
{
	return calldata_string(data, "new_name");
}

void obs_sceneitem_hotkey_release(obs_sceneitem_t *item)
{
	obs_sceneitem_release(item);
}

void obs_sceneitem_hotkey_set_visible(obs_sceneitem_t *item, bool visible)
{
	obs_sceneitem_set_visible(item, visible);
}

void obs_sceneitem_hotkey_descriptions(size_t pair, const char *show, const char *hide)
{
	obs_hotkey_pair_set_descriptions(pair, show, hide);
}
