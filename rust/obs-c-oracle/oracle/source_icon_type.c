/* Original getter plus the real field adapter over a read-only test registry.
 * Lookup is a fixture; real registry/registration behavior is covered by the
 * unchanged test_module_register_copy_steps against hybrid and reference. */
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

/* The production shim and original C use the same deterministic registry.
 * No NULL-id guarantee is added: strcmp keeps the original precondition. */
struct obs_source_info *get_source_info(const char *id)
{
	for (size_t i = 0; i < sizeof(icon_registry) / sizeof(icon_registry[0]); i++) {
		if (strcmp(icon_registry[i].id, id) == 0)
			return &icon_registry[i];
	}
	return NULL;
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
