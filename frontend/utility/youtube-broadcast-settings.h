#pragma once

#include <stdbool.h>

#include <util/config-file.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The YouTube broadcast settings that "Remember these settings" keeps in the
 * profile's [YouTube] section, kept free of Qt so they can be unit tested. */
struct youtube_broadcast_settings {
	const char *title;
	const char *description;
	const char *privacy;
	const char *category_id;
	const char *latency;
	const char *projection;
	const char *thumbnail_file;
	bool made_for_kids;
	bool auto_start;
	bool auto_stop;
	bool dvr;
	bool schedule_for_later;
};

/* Writes the settings and sets RememberSettings. NULL strings are saved as
 * empty strings. */
void youtube_broadcast_settings_save(config_t *config, const struct youtube_broadcast_settings *settings);

/* Reads the settings back. The strings point into config and stay valid
 * until those keys are changed or config is closed; a missing key reads as
 * NULL or false. */
void youtube_broadcast_settings_load(config_t *config, struct youtube_broadcast_settings *settings);

#ifdef __cplusplus
}
#endif
