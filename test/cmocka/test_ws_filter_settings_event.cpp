// The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
// with libstdc++.
#include <string>
#include <vector>

#include "eventhandler/EventHandler.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

#ifdef __linux__
#include <obs-nix-platform.h>
#include <X11/Xlib.h>
#endif

/* obs-websocket must send SourceFilterSettingsChanged when a filter's
 * settings change (SetSourceFilterSettings, or the filter's properties in the
 * UI). The event handler listened for it on the core "source_update" signal
 * only, which libobs never emits for filters: filters are always private
 * sources, and private sources do not fire core signals. So the event never
 * came, while the other filter events (created, removed, renamed, enabled)
 * did.
 *
 * The test builds the plugin's event handler from its own sources against
 * real libobs (no frontend, no websocket server: the frontend calls the
 * handler makes are stubbed below) and records the events it broadcasts.
 * Sources and filters are real libobs sources of minimal registered types;
 * no video is needed. obs_startup() needs a display on Linux; there the
 * cases skip without one. */

#define INPUT_ID "ws_filter_event_input"
#define FILTER_ID "ws_filter_event_filter"

/* Frontend API the event handler calls; there is no frontend here. */
extern "C" {
void obs_frontend_add_event_callback(obs_frontend_event_cb, void *) {}
void obs_frontend_remove_event_callback(obs_frontend_event_cb, void *) {}
obs_source_t *obs_frontend_get_current_scene(void)
{
	return nullptr;
}
obs_source_t *obs_frontend_get_current_preview_scene(void)
{
	return nullptr;
}
obs_source_t *obs_frontend_get_current_transition(void)
{
	return nullptr;
}
void obs_frontend_get_scenes(struct obs_frontend_source_list *) {}
void obs_frontend_get_transitions(struct obs_frontend_source_list *) {}
int obs_frontend_get_transition_duration(void)
{
	return 0;
}
char **obs_frontend_get_scene_collections(void)
{
	return nullptr;
}
char *obs_frontend_get_current_scene_collection(void)
{
	return nullptr;
}
char **obs_frontend_get_profiles(void)
{
	return nullptr;
}
char *obs_frontend_get_current_profile(void)
{
	return nullptr;
}
char *obs_frontend_get_current_profile_path(void)
{
	return nullptr;
}
char *obs_frontend_get_current_record_output_path(void)
{
	return nullptr;
}
char *obs_frontend_get_last_replay(void)
{
	return nullptr;
}
char *obs_frontend_get_last_screenshot(void)
{
	return nullptr;
}
obs_output_t *obs_frontend_get_streaming_output(void)
{
	return nullptr;
}
obs_output_t *obs_frontend_get_recording_output(void)
{
	return nullptr;
}
void *obs_frontend_get_system_tray(void)
{
	return nullptr;
}
}

/* Plugin globals the handler's sources refer to. */
bool IsDebugEnabled()
{
	return false;
}

os_cpu_usage_info_t *GetCpuUsageInfo()
{
	return nullptr;
}

/* Qt-backed helpers, not on the paths tested here. */
std::string Utils::Obs::StringHelper::GetCurrentSceneCollection()
{
	return {};
}
std::string Utils::Obs::StringHelper::GetCurrentProfile()
{
	return {};
}
std::string Utils::Obs::StringHelper::GetCurrentRecordOutputPath()
{
	return {};
}
std::string Utils::Obs::StringHelper::GetLastRecordFileName()
{
	return {};
}
std::string Utils::Obs::StringHelper::GetLastReplayBufferFileName()
{
	return {};
}
std::string Utils::Obs::StringHelper::GetLastScreenshotFileName()
{
	return {};
}

struct event {
	std::string type;
	json data;
};

static std::vector<event> events;
static bool obs_up;
#ifdef __linux__
static Display *display;
#endif

static size_t count_events(const char *type)
{
	size_t n = 0;
	for (const event &e : events) {
		if (e.type == type) {
			n++;
		}
	}
	return n;
}

static const event *last_event(const char *type)
{
	for (auto it = events.rbegin(); it != events.rend(); ++it) {
		if (it->type == type) {
			return &*it;
		}
	}
	return nullptr;
}

static const char *input_name(void *)
{
	return "ws filter event input";
}

static const char *filter_name(void *)
{
	return "ws filter event filter";
}

static void *data_create(obs_data_t *, obs_source_t *source)
{
	return source;
}

static void data_destroy(void *) {}

static void data_update(void *, obs_data_t *) {}

static void register_types(void)
{
	struct obs_source_info input = {};
	input.id = INPUT_ID;
	input.type = OBS_SOURCE_TYPE_INPUT;
	input.output_flags = OBS_SOURCE_AUDIO;
	input.get_name = input_name;
	input.create = data_create;
	input.destroy = data_destroy;
	input.update = data_update;
	obs_register_source(&input);

	/* An audio filter: without OBS_SOURCE_VIDEO, obs_source_update()
	 * applies the settings (and signals) right away instead of on the
	 * next video tick. */
	struct obs_source_info filter = {};
	filter.id = FILTER_ID;
	filter.type = OBS_SOURCE_TYPE_FILTER;
	filter.output_flags = OBS_SOURCE_AUDIO;
	filter.get_name = filter_name;
	filter.create = data_create;
	filter.destroy = data_destroy;
	filter.update = data_update;
	obs_register_source(&filter);
}

/* Without video, destroying a source logs a debug line for every graphics
 * call it skips; keep warnings and errors only. */
static void log_handler(int lvl, const char *msg, va_list args, void *)
{
	if (lvl > LOG_WARNING) {
		return;
	}
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static int group_setup(void **)
{
	base_set_log_handler(log_handler, nullptr);
#ifdef __linux__
	display = XOpenDisplay(nullptr);
	if (!display) {
		fprintf(stderr, "no X display: cases skip\n");
		return 0;
	}
	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
	obs_set_nix_platform_display(display);
#endif
	if (!obs_startup("en-US", nullptr, nullptr)) {
		fprintf(stderr, "obs_startup failed: cases skip\n");
		return 0;
	}
	register_types();
	obs_up = true;
	return 0;
}

static int group_teardown(void **)
{
	if (obs_up) {
		obs_shutdown();
	}
#ifdef __linux__
	if (display) {
		XCloseDisplay(display);
	}
#endif
	return 0;
}

struct fixture {
	EventHandler *handler;
	obs_source_t *input;
	obs_source_t *filter;
};

static int setup(void **state)
{
	if (!obs_up) {
		return 0;
	}

	events.clear();
	auto *f = new fixture();
	f->handler = new EventHandler();
	f->handler->SetEventCallback(
		[](uint64_t, std::string type, json data, uint8_t) { events.push_back({type, data}); });

	/* The handler connects to the input when it is created and to the
	 * filter when the filter is added to the input, as in OBS. */
	f->input = obs_source_create(INPUT_ID, "Input", nullptr, nullptr);
	f->filter = obs_source_create(FILTER_ID, "Filter", nullptr, nullptr);
	assert_non_null(f->input);
	assert_non_null(f->filter);
	obs_source_filter_add(f->input, f->filter);

	*state = f;
	return 0;
}

static int teardown(void **state)
{
	auto *f = static_cast<fixture *>(*state);
	if (!f) {
		return 0;
	}

	obs_source_filter_remove(f->input, f->filter);
	obs_source_release(f->filter);
	obs_source_remove(f->input);
	obs_source_release(f->input);
	delete f->handler;
	delete f;
	*state = nullptr;
	return 0;
}

static void update_filter(obs_source_t *filter, long long value)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "value", value);
	obs_source_update(filter, settings);
	obs_data_release(settings);
}

/* Changing a filter's settings sends one SourceFilterSettingsChanged with
 * the source, the filter and the new settings. */
static void settings_change_sends_event(void **state)
{
	if (!obs_up) {
		skip();
	}
	auto *f = static_cast<fixture *>(*state);

	/* The other filter events already worked. */
	assert_int_equal(count_events("SourceFilterCreated"), 1);

	update_filter(f->filter, 42);

	assert_int_equal(count_events("SourceFilterSettingsChanged"), 1);
	const event *e = last_event("SourceFilterSettingsChanged");
	assert_non_null(e);
	assert_string_equal(e->data["sourceName"].get<std::string>().c_str(), "Input");
	assert_string_equal(e->data["filterName"].get<std::string>().c_str(), "Filter");
	assert_int_equal(e->data["filterSettings"]["value"].get<long long>(), 42);
}

/* Every change sends its own event, with that change's settings. */
static void every_change_sends_event(void **state)
{
	if (!obs_up) {
		skip();
	}
	auto *f = static_cast<fixture *>(*state);

	update_filter(f->filter, 1);
	update_filter(f->filter, 2);
	update_filter(f->filter, 3);

	assert_int_equal(count_events("SourceFilterSettingsChanged"), 3);
	assert_int_equal(last_event("SourceFilterSettingsChanged")->data["filterSettings"]["value"].get<long long>(),
			 3);
}

/* The input's own settings change is still InputSettingsChanged, and it
 * does not pass for a filter change. */
static void input_change_is_not_filter_event(void **state)
{
	if (!obs_up) {
		skip();
	}
	auto *f = static_cast<fixture *>(*state);

	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "value", 7);
	obs_source_update(f->input, settings);
	obs_data_release(settings);

	assert_int_equal(count_events("InputSettingsChanged"), 1);
	assert_int_equal(count_events("SourceFilterSettingsChanged"), 0);
}

/* Once the filter is removed from its source, its changes are no longer
 * reported (the handler lets go of the filter). */
static void removed_filter_sends_nothing(void **state)
{
	if (!obs_up) {
		skip();
	}
	auto *f = static_cast<fixture *>(*state);

	obs_source_filter_remove(f->input, f->filter);
	assert_int_equal(count_events("SourceFilterRemoved"), 1);

	update_filter(f->filter, 5);
	assert_int_equal(count_events("SourceFilterSettingsChanged"), 0);

	/* Put it back for teardown; adding it reconnects it. */
	obs_source_filter_add(f->input, f->filter);
	update_filter(f->filter, 6);
	assert_int_equal(count_events("SourceFilterSettingsChanged"), 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(settings_change_sends_event, setup, teardown),
		cmocka_unit_test_setup_teardown(every_change_sends_event, setup, teardown),
		cmocka_unit_test_setup_teardown(input_change_is_not_filter_event, setup, teardown),
		cmocka_unit_test_setup_teardown(removed_filter_sends_nothing, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
