#include "frontend/build/frontend-utility-missing-source-types.pch.h"

void missing_source_types_add(struct missing_source_types *list, const char *id, bool registered)
{
	if (registered || !id || !*id) {
		return;
	}
	for (size_t i = 0; i < list->count; i++) {
		if (strncmp(list->ids[i], id, MISSING_SOURCE_TYPE_ID_SIZE - 1) == 0) {
			list->sources[i]++;
			return;
		}
	}
	if (list->count >= MISSING_SOURCE_TYPES_MAX) {
		return;
	}
	snprintf(list->ids[list->count], MISSING_SOURCE_TYPE_ID_SIZE, "%s", id);
	list->sources[list->count] = 1;
	list->count++;
}

enum missing_source_hint missing_source_types_hint(const char *id)
{
	if (id && strcmp(id, "browser_source") == 0) {
		return MISSING_SOURCE_HINT_BROWSER;
	}
	if (id && strcmp(id, "vlc_source") == 0) {
		return MISSING_SOURCE_HINT_VLC;
	}
	return MISSING_SOURCE_HINT_PLUGIN;
}
