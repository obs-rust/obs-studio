/* Original getter plus real field adapter; the shared original source lookup
 * uses this read-only default registry when no fixture overrides its core. */
#include "libobs/build/libobs-ported-service-getters.pch.h"
#include <stddef.h>
#include <string.h>

static struct obs_source_info icon_registry[] = {
	{.id = "icon_0", .icon_type = OBS_ICON_TYPE_UNKNOWN},
	{.id = "icon_1", .icon_type = OBS_ICON_TYPE_IMAGE},
	{.id = "icon_2", .icon_type = OBS_ICON_TYPE_COLOR},
	{.id = "icon_3", .icon_type = OBS_ICON_TYPE_SLIDESHOW},
	{.id = "icon_4", .icon_type = OBS_ICON_TYPE_AUDIO_INPUT},
	{.id = "icon_5", .icon_type = OBS_ICON_TYPE_AUDIO_OUTPUT},
	{.id = "icon_6", .icon_type = OBS_ICON_TYPE_DESKTOP_CAPTURE},
	{.id = "icon_7", .icon_type = OBS_ICON_TYPE_WINDOW_CAPTURE},
	{.id = "icon_8", .icon_type = OBS_ICON_TYPE_GAME_CAPTURE},
	{.id = "icon_9", .icon_type = OBS_ICON_TYPE_CAMERA},
	{.id = "icon_10", .icon_type = OBS_ICON_TYPE_TEXT},
	{.id = "icon_11", .icon_type = OBS_ICON_TYPE_MEDIA},
	{.id = "icon_12", .icon_type = OBS_ICON_TYPE_BROWSER},
	{.id = "icon_13", .icon_type = OBS_ICON_TYPE_CUSTOM},
	{.id = "icon_14", .icon_type = OBS_ICON_TYPE_PROCESS_AUDIO_OUTPUT},
};

const struct obs_core *oracle_source_icon_core(void);
const struct obs_core *oracle_source_icon_core(void)
{
	static const struct obs_core core = {
		.source_types = {.array = icon_registry, .num = sizeof(icon_registry) / sizeof(icon_registry[0])}};
	return &core;
}

enum obs_icon_type oracle_obs_source_get_icon_type(const char *id);
#define obs_source_get_icon_type oracle_obs_source_get_icon_type
#include "libobs/obs-source-icon-type-getter.c.h"
#undef obs_source_get_icon_type
#include "source_icon_type_fields.inc"

bool oracle_source_icon_type_layout(size_t *size, size_t *alignment);
bool oracle_source_icon_type_layout(size_t *size, size_t *alignment)
{
	*size = sizeof(enum obs_icon_type);
	*alignment = _Alignof(enum obs_icon_type);
	for (size_t i = 0; i < sizeof(icon_registry) / sizeof(icon_registry[0]); i++) {
		if ((unsigned)icon_registry[i].icon_type != i ||
		    obs_source_icon_type_fields(&icon_registry[i]) != &icon_registry[i].icon_type)
			return false;
	}
	return true;
}
