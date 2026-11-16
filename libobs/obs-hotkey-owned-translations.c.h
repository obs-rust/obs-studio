#include "libobs/build/libobs.pch.h"

void obs_hotkeys_set_audio_hotkeys_translations(const char *mute, const char *unmute, const char *push_to_mute,
						const char *push_to_talk, const char *monitor_on,
						const char *monitor_off)
{
#define SET_T(n)               \
	bfree(obs->hotkeys.n); \
	obs->hotkeys.n = bstrdup(n)
	SET_T(mute);
	SET_T(unmute);
	SET_T(push_to_mute);
	SET_T(push_to_talk);
	SET_T(monitor_on);
	SET_T(monitor_off);
#undef SET_T
}

void obs_hotkeys_set_sceneitem_hotkeys_translations(const char *show, const char *hide)
{
#define SET_T(n)                           \
	bfree(obs->hotkeys.sceneitem_##n); \
	obs->hotkeys.sceneitem_##n = bstrdup(n)
	SET_T(show);
	SET_T(hide);
#undef SET_T
}
