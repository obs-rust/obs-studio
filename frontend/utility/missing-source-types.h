#pragma once

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The source types of a loaded scene collection that no loaded module
 * provides, for the notice OBSBasic::LoadData() shows after loading, kept
 * free of Qt so it can be unit tested (obs-rust/obs-studio#1865). libobs
 * keeps such sources with their settings ("Source ID '%s' not found"), but
 * they render nothing, and without a notice users lose their browser
 * overlays silently when they move a collection to a build without them. */

#define MISSING_SOURCE_TYPES_MAX 16
#define MISSING_SOURCE_TYPE_ID_SIZE 64

struct missing_source_types {
	char ids[MISSING_SOURCE_TYPES_MAX][MISSING_SOURCE_TYPE_ID_SIZE];
	size_t count;
	/* Sources found per id, by index. */
	size_t sources[MISSING_SOURCE_TYPES_MAX];
};

/* What the notice says about a missing type. */
enum missing_source_hint {
	/* obs-browser: not in this build (the AppImage, #1865). */
	MISSING_SOURCE_HINT_BROWSER,
	/* vlc-video: not in this build, or VLC is not installed. */
	MISSING_SOURCE_HINT_VLC,
	/* Any other type: its plugin is not loaded. */
	MISSING_SOURCE_HINT_PLUGIN,
};

/* Records a loaded source of type id. registered: a loaded module provides
 * the type (obs_source_get_display_name(id) != NULL); those are ignored.
 * Each missing id is listed once, in first-seen order, and counted. */
void missing_source_types_add(struct missing_source_types *list, const char *id, bool registered);

enum missing_source_hint missing_source_types_hint(const char *id);

#ifdef __cplusplus
}
#endif
