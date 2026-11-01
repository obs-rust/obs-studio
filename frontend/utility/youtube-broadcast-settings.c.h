#include "frontend/build/frontend-utility-youtube-broadcast-settings.pch.h"

#define SECTION "YouTube"

static const char *ybs_str_or_empty(const char *str)
{
	return str ? str : "";
}

void youtube_broadcast_settings_save(config_t *config, const struct youtube_broadcast_settings *settings)
{
	config_set_string(config, SECTION, "Title", ybs_str_or_empty(settings->title));
	config_set_string(config, SECTION, "Description", ybs_str_or_empty(settings->description));
	config_set_string(config, SECTION, "Privacy", ybs_str_or_empty(settings->privacy));
	config_set_string(config, SECTION, "CategoryID", ybs_str_or_empty(settings->category_id));
	config_set_string(config, SECTION, "Latency", ybs_str_or_empty(settings->latency));
	config_set_bool(config, SECTION, "MadeForKids", settings->made_for_kids);
	config_set_bool(config, SECTION, "AutoStart", settings->auto_start);
	config_set_bool(config, SECTION, "AutoStop", settings->auto_stop);
	config_set_bool(config, SECTION, "DVR", settings->dvr);
	config_set_bool(config, SECTION, "ScheduleForLater", settings->schedule_for_later);
	config_set_string(config, SECTION, "Projection", ybs_str_or_empty(settings->projection));
	config_set_string(config, SECTION, "ThumbnailFile", ybs_str_or_empty(settings->thumbnail_file));
	config_set_bool(config, SECTION, "RememberSettings", true);
}

void youtube_broadcast_settings_load(config_t *config, struct youtube_broadcast_settings *settings)
{
	settings->title = config_get_string(config, SECTION, "Title");
	settings->description = config_get_string(config, SECTION, "Description");
	settings->privacy = config_get_string(config, SECTION, "Privacy");
	settings->category_id = config_get_string(config, SECTION, "CategoryID");
	settings->latency = config_get_string(config, SECTION, "Latency");
	settings->made_for_kids = config_get_bool(config, SECTION, "MadeForKids");
	settings->auto_start = config_get_bool(config, SECTION, "AutoStart");
	settings->auto_stop = config_get_bool(config, SECTION, "AutoStop");
	settings->dvr = config_get_bool(config, SECTION, "DVR");
	settings->schedule_for_later = config_get_bool(config, SECTION, "ScheduleForLater");
	settings->projection = config_get_string(config, SECTION, "Projection");
	settings->thumbnail_file = config_get_string(config, SECTION, "ThumbnailFile");
}

#undef SECTION
