#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <util/config-file.h>

#include "youtube-broadcast-settings.h"

/* Characterizes the YouTube broadcast settings that "Remember these
 * settings" keeps in the profile: what is saved is what the dialog loads
 * back. */

static config_t *open_empty(void)
{
	config_t *config = NULL;
	if (config_open_string(&config, "") != CONFIG_SUCCESS || !config) {
		fprintf(stderr, "config_open_string failed\n");
		fail();
	}
	return config;
}

static void expect_bool(const char *what, bool got, bool expected)
{
	if (got != expected) {
		fprintf(stderr, "%s\n  got:      %s\n  expected: %s\n", what, got ? "true" : "false",
			expected ? "true" : "false");
		fail();
	}
}

static void expect_str(const char *what, const char *got, const char *expected)
{
	if (!got || strcmp(got, expected) != 0) {
		fprintf(stderr, "%s\n  got:      %s%s%s\n  expected: \"%s\"\n", what, got ? "\"" : "",
			got ? got : "NULL", got ? "\"" : "", expected);
		fail();
	}
}

static struct youtube_broadcast_settings sample(bool auto_start, bool auto_stop)
{
	struct youtube_broadcast_settings settings = {
		.title = "Saturday stream",
		.description = "Line one\nLine two",
		.privacy = "unlisted",
		.category_id = "20",
		.latency = "ultraLow",
		.projection = "360",
		.thumbnail_file = "thumbnail.png",
		.made_for_kids = true,
		.auto_start = auto_start,
		.auto_stop = auto_stop,
		.dvr = false,
		.schedule_for_later = true,
	};
	return settings;
}

/* Saves the settings, loads them back, and checks every field. */
static void round_trip(bool auto_start, bool auto_stop)
{
	config_t *config = open_empty();
	struct youtube_broadcast_settings saved = sample(auto_start, auto_stop);
	struct youtube_broadcast_settings loaded = {0};

	youtube_broadcast_settings_save(config, &saved);
	youtube_broadcast_settings_load(config, &loaded);

	fprintf(stderr, "saved auto_start=%d auto_stop=%d, loaded auto_start=%d auto_stop=%d\n", auto_start, auto_stop,
		loaded.auto_start, loaded.auto_stop);
	expect_bool("AutoStart", loaded.auto_start, auto_start);
	expect_bool("AutoStop", loaded.auto_stop, auto_stop);
	expect_bool("AutoStop key", config_get_bool(config, "YouTube", "AutoStop"), auto_stop);

	expect_str("Title", loaded.title, saved.title);
	expect_str("Description", loaded.description, saved.description);
	expect_str("Privacy", loaded.privacy, saved.privacy);
	expect_str("CategoryID", loaded.category_id, saved.category_id);
	expect_str("Latency", loaded.latency, saved.latency);
	expect_str("Projection", loaded.projection, saved.projection);
	expect_str("ThumbnailFile", loaded.thumbnail_file, saved.thumbnail_file);
	expect_bool("MadeForKids", loaded.made_for_kids, saved.made_for_kids);
	expect_bool("DVR", loaded.dvr, saved.dvr);
	expect_bool("ScheduleForLater", loaded.schedule_for_later, saved.schedule_for_later);
	expect_bool("RememberSettings", config_get_bool(config, "YouTube", "RememberSettings"), true);

	config_close(config);
}

/* Upstream obs-studio issue 13856 (reported by cloud2519): the dialog saved
 * "Automatically stop" from the "Automatically start" checkbox, so the next
 * broadcast restored Auto-stop as whatever Auto-start was. The combination
 * in the report: start automatically, stop by hand. */
static void test_auto_start_without_auto_stop(void **state)
{
	(void)state;
	round_trip(true, false);
}

static void test_auto_stop_without_auto_start(void **state)
{
	(void)state;
	round_trip(false, true);
}

/* Unticking only Auto-stop in a dialog restored with both on is kept. */
static void test_auto_stop_change_is_saved(void **state)
{
	(void)state;
	config_t *config = open_empty();
	struct youtube_broadcast_settings settings = sample(true, true);
	struct youtube_broadcast_settings loaded = {0};

	youtube_broadcast_settings_save(config, &settings);
	settings.auto_stop = false;
	youtube_broadcast_settings_save(config, &settings);
	youtube_broadcast_settings_load(config, &loaded);

	expect_bool("AutoStart", loaded.auto_start, true);
	expect_bool("AutoStop", loaded.auto_stop, false);
	config_close(config);
}

static void test_auto_start_and_auto_stop(void **state)
{
	(void)state;
	round_trip(true, true);
}

static void test_neither_auto_start_nor_auto_stop(void **state)
{
	(void)state;
	round_trip(false, false);
}

/* The other booleans flipped, so a field read from its neighbour shows. */
static void test_other_flags_round_trip(void **state)
{
	(void)state;
	config_t *config = open_empty();
	struct youtube_broadcast_settings settings = sample(false, false);
	struct youtube_broadcast_settings loaded = {0};

	settings.made_for_kids = false;
	settings.dvr = true;
	settings.schedule_for_later = false;
	settings.projection = "rectangular";
	youtube_broadcast_settings_save(config, &settings);
	youtube_broadcast_settings_load(config, &loaded);

	expect_bool("MadeForKids", loaded.made_for_kids, false);
	expect_bool("DVR", loaded.dvr, true);
	expect_bool("ScheduleForLater", loaded.schedule_for_later, false);
	expect_str("Projection", loaded.projection, "rectangular");
	config_close(config);
}

/* An empty dialog field is saved as an empty string, as before. */
static void test_null_strings_saved_empty(void **state)
{
	(void)state;
	config_t *config = open_empty();
	struct youtube_broadcast_settings settings = {0};
	struct youtube_broadcast_settings loaded = {0};

	youtube_broadcast_settings_save(config, &settings);
	youtube_broadcast_settings_load(config, &loaded);

	expect_str("Title", loaded.title, "");
	expect_str("Description", loaded.description, "");
	expect_str("Privacy", loaded.privacy, "");
	expect_str("CategoryID", loaded.category_id, "");
	expect_str("Latency", loaded.latency, "");
	expect_str("Projection", loaded.projection, "");
	expect_str("ThumbnailFile", loaded.thumbnail_file, "");
	config_close(config);
}

/* Nothing remembered yet: the dialog's defaults stay (unchecked, no text). */
static void test_load_without_saved_settings(void **state)
{
	(void)state;
	config_t *config = open_empty();
	struct youtube_broadcast_settings loaded = sample(true, true);

	youtube_broadcast_settings_load(config, &loaded);

	expect_bool("AutoStart", loaded.auto_start, false);
	expect_bool("AutoStop", loaded.auto_stop, false);
	expect_bool("MadeForKids", loaded.made_for_kids, false);
	if (loaded.title || loaded.projection || loaded.thumbnail_file) {
		fprintf(stderr, "missing keys should read as NULL\n");
		fail();
	}
	config_close(config);
}

/* Settings saved by an earlier version, in the profile's basic.ini format. */
static void test_load_from_profile_text(void **state)
{
	(void)state;
	config_t *config = NULL;
	struct youtube_broadcast_settings loaded = {0};
	const char *ini = "[YouTube]\n"
			  "Title=Saturday stream\n"
			  "Privacy=private\n"
			  "AutoStart=true\n"
			  "AutoStop=false\n"
			  "DVR=true\n"
			  "RememberSettings=true\n";

	if (config_open_string(&config, ini) != CONFIG_SUCCESS || !config) {
		fprintf(stderr, "config_open_string failed\n");
		fail();
	}
	youtube_broadcast_settings_load(config, &loaded);

	expect_str("Title", loaded.title, "Saturday stream");
	expect_str("Privacy", loaded.privacy, "private");
	expect_bool("AutoStart", loaded.auto_start, true);
	expect_bool("AutoStop", loaded.auto_stop, false);
	expect_bool("DVR", loaded.dvr, true);
	config_close(config);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_auto_start_without_auto_stop),
		cmocka_unit_test(test_auto_stop_without_auto_start),
		cmocka_unit_test(test_auto_start_and_auto_stop),
		cmocka_unit_test(test_neither_auto_start_nor_auto_stop),
		cmocka_unit_test(test_auto_stop_change_is_saved),
		cmocka_unit_test(test_other_flags_round_trip),
		cmocka_unit_test(test_null_strings_saved_empty),
		cmocka_unit_test(test_load_without_saved_settings),
		cmocka_unit_test(test_load_from_profile_text),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
