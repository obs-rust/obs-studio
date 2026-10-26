/* obs-websocket SetInputAudioMonitorType "Monitor Only" must keep the input
 * out of the outputs on 33.0 (#2447, the API side of #2444).
 *
 * Before 33.0 a source had three monitoring types: off, Monitor Only and
 * Monitor and Output. 33.0 keeps an on/off switch, and on means monitor and
 * output; the monitor ignores mute, so Monitor Only is now monitoring on plus
 * muted (#2445 loads old scene collections that way). The request mapped
 * every type but none to monitoring on and left the mute alone, so an input
 * set to Monitor Only played into the stream and recordings, and
 * GetInputAudioMonitorType reported Monitor and Output for it.
 *
 * This compiles the plugin's input requests (RequestHandler_Inputs.cpp with
 * Request.cpp and the Utils helpers it calls) against real libobs, as
 * test_ws_remove_input does. A thread delivers 10 ms packets of a constant on
 * the left channel, stamped with the system clock, like a live capture. The
 * input is put on the program, and the mixed audio libobs hands to outputs
 * (obs_add_raw_audio_callback) must carry the tone, or stay silent for Monitor
 * Only. The audio the monitor is fed (the input's audio capture callbacks,
 * which get it muted or not) must carry the tone whenever monitoring is on.
 * Mixing needs core video, so the cases run under xvfb-run and skip without a
 * display. No monitoring device is used: PULSE_SERVER points nowhere. */

/* The C++ headers come before cmocka.h: its macros break libstdc++. */
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <mutex>
#include <string>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#include "requesthandler/RequestHandler.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
// cmocka.h 1.1 (Debian 13) and the Windows cmocka.h have no extern "C" guard.
extern "C" {
#include <cmocka.h>
}

RequestHandler::RequestHandler(SessionPtr session) : _session(session) {}

/* The monitor type requests are private. An explicit instantiation may name
 * them. */
using RequestFn = RequestResult (RequestHandler::*)(const Request &);
template<RequestFn F> struct UnlockSetMonitorType {
	friend RequestFn set_monitor_type() { return F; }
};
RequestFn set_monitor_type();
template struct UnlockSetMonitorType<&RequestHandler::SetInputAudioMonitorType>;
template<RequestFn F> struct UnlockGetMonitorType {
	friend RequestFn get_monitor_type() { return F; }
};
RequestFn get_monitor_type();
template struct UnlockGetMonitorType<&RequestHandler::GetInputAudioMonitorType>;

/* The helpers and the other input requests linked in call these. None of them
 * runs here. */
extern "C" {
obs_source_t *obs_frontend_get_current_scene(void)
{
	return nullptr;
}
obs_source_t *obs_frontend_get_current_preview_scene(void)
{
	return nullptr;
}
bool obs_frontend_preview_program_mode_active(void)
{
	return false;
}
void obs_frontend_get_scenes(struct obs_frontend_source_list *) {}
void obs_frontend_get_transitions(struct obs_frontend_source_list *) {}
void obs_frontend_set_current_scene(obs_source_t *) {}
void obs_frontend_set_current_preview_scene(obs_source_t *) {}
obs_source_t *obs_frontend_get_current_transition(void)
{
	return nullptr;
}
char **obs_frontend_get_scene_collections(void)
{
	return nullptr;
}
char **obs_frontend_get_profiles(void)
{
	return nullptr;
}
obs_data_t *obs_frontend_get_scene_transition_override(obs_source_t *)
{
	return nullptr;
}
}
os_cpu_usage_info_t *GetCpuUsageInfo()
{
	return nullptr;
}
std::string Utils::Obs::StringHelper::GetCurrentRecordOutputPath()
{
	return "";
}

#define SOURCE_ID "ws_monitor_only_capture"
#define W 64
#define H 64
#define RATE 48000
#define MSEC 1000000ULL
#define PACKET_FRAMES 480 /* 10 ms */
#define PACKET_NS (10 * MSEC)
#define LEVEL 0.25f
#define TOLERANCE 1e-6f
/* frames of the tone to see before a case passes */
#define WANT_FRAMES 4096
#define WAIT_MS 4000
/* how long a silent case listens, once the monitor feed has the tone */
#define LISTEN_MS 500

/* ------------------------------------------------------------------------ */
/* the capture */

struct capture {
	obs_source_t *source;
	pthread_t thread;
	bool thread_started;
	volatile bool stop;
};

static void *capture_thread(void *param)
{
	auto c = static_cast<capture *>(param);
	static float left[PACKET_FRAMES];
	static float right[PACKET_FRAMES];
	for (size_t i = 0; i < PACKET_FRAMES; i++) {
		left[i] = LEVEL;
	}

	os_sleep_ms(50);
	uint64_t t0 = os_gettime_ns();
	for (uint64_t n = 0; !c->stop; n++) {
		uint64_t ts = t0 + n * PACKET_NS;
		os_sleepto_ns(ts + PACKET_NS);

		struct obs_source_audio audio = {};
		audio.data[0] = (const uint8_t *)left;
		audio.data[1] = (const uint8_t *)right;
		audio.frames = PACKET_FRAMES;
		audio.speakers = SPEAKERS_STEREO;
		audio.format = AUDIO_FORMAT_FLOAT_PLANAR;
		audio.samples_per_sec = RATE;
		audio.timestamp = ts;
		obs_source_output_audio(c->source, &audio);
	}
	return nullptr;
}

static const char *capture_name(void *)
{
	return "ws monitor only capture";
}

static void *capture_create(obs_data_t *, obs_source_t *source)
{
	auto c = static_cast<capture *>(bzalloc(sizeof(capture)));
	c->source = source;
	if (pthread_create(&c->thread, nullptr, capture_thread, c) == 0) {
		c->thread_started = true;
	} else {
		fprintf(stderr, "could not start the capture thread\n");
	}
	return c;
}

static void capture_destroy(void *data)
{
	auto c = static_cast<capture *>(data);
	if (c->thread_started) {
		c->stop = true;
		pthread_join(c->thread, nullptr);
	}
	bfree(c);
}

static struct obs_source_info capture_info = {};

/* ------------------------------------------------------------------------ */
/* what is heard: the program mix and the monitor feed */

static std::mutex heard_mutex;
static size_t program_frames; /* program frames carrying the tone */
static size_t monitor_frames; /* monitor feed frames carrying the tone */

static size_t count_tone(const float *left, uint32_t frames)
{
	size_t n = 0;
	for (uint32_t i = 0; i < frames; i++) {
		if (std::fabs(left[i] - LEVEL) <= TOLERANCE) {
			n++;
		}
	}
	return n;
}

static void raw_audio(void *, size_t, struct audio_data *data)
{
	size_t n = count_tone((const float *)data->data[0], data->frames);
	std::lock_guard<std::mutex> lock(heard_mutex);
	program_frames += n;
}

static void monitor_feed(void *, obs_source_t *, const struct audio_data *data, bool)
{
	size_t n = count_tone((const float *)data->data[0], data->frames);
	std::lock_guard<std::mutex> lock(heard_mutex);
	monitor_frames += n;
}

static void read_heard(size_t *program, size_t *monitor)
{
	std::lock_guard<std::mutex> lock(heard_mutex);
	*program = program_frames;
	*monitor = monitor_frames;
}

static void reset_heard(void)
{
	std::lock_guard<std::mutex> lock(heard_mutex);
	program_frames = 0;
	monitor_frames = 0;
}

static bool obs_up;

static int group_setup(void **)
{
	/* the monitor never plays anything */
	setenv("PULSE_SERVER", "unix:/nonexistent-ws-monitor-only", 1);

	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}
	if (!obs_startup("en-US", nullptr, nullptr)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {};
	ovi.graphics_module = GRAPHICS_MODULE_PATH;
	ovi.fps_num = 30;
	ovi.fps_den = 1;
	ovi.base_width = W;
	ovi.base_height = H;
	ovi.output_width = W;
	ovi.output_height = H;
	ovi.output_format = VIDEO_FORMAT_NV12;
	ovi.colorspace = VIDEO_CS_709;
	ovi.range = VIDEO_RANGE_PARTIAL;
	ovi.scale_type = OBS_SCALE_BICUBIC;
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed\n");
		obs_shutdown();
		return 0;
	}

	struct obs_audio_info oai = {};
	oai.samples_per_sec = RATE;
	oai.speakers = SPEAKERS_STEREO;
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "skipped: obs_reset_audio failed\n");
		obs_shutdown();
		return 0;
	}

	capture_info.id = SOURCE_ID;
	capture_info.type = OBS_SOURCE_TYPE_INPUT;
	capture_info.output_flags = OBS_SOURCE_AUDIO;
	capture_info.get_name = capture_name;
	capture_info.create = capture_create;
	capture_info.destroy = capture_destroy;
	obs_register_source(&capture_info);
	obs_add_raw_audio_callback(0, nullptr, raw_audio, nullptr);

	obs_up = true;
	return 0;
}

static int group_teardown(void **)
{
	if (obs_up) {
		obs_remove_raw_audio_callback(0, raw_audio, nullptr);
		obs_shutdown();
		obs_up = false;
	}
	return 0;
}

/* ------------------------------------------------------------------------ */
/* the input and the requests */

static obs_source_t *input;
static int input_count;
static char input_name[64];

static int case_setup(void **)
{
	if (!obs_up) {
		return 0;
	}
	snprintf(input_name, sizeof(input_name), "ws monitor only %d", input_count++);
	input = obs_source_create(SOURCE_ID, input_name, nullptr, nullptr);
	if (!input) {
		return -1;
	}
	obs_source_add_audio_capture_callback(input, monitor_feed, nullptr);
	return 0;
}

static int case_teardown(void **)
{
	if (!input) {
		return 0;
	}
	obs_set_output_source(0, nullptr);
	obs_source_remove_audio_capture_callback(input, monitor_feed, nullptr);
	obs_source_remove(input);
	obs_source_release(input);
	input = nullptr;
	return 0;
}

/* Runs a request; an exception would escape the websocket thread and abort
 * OBS, so it fails the case instead. */
static RequestResult call(RequestFn fn, const char *type, const json &data)
{
	Request request(type, data);
	RequestHandler handler;
	try {
		return (handler.*fn)(request);
	} catch (const std::exception &e) {
		fail_msg("%s threw '%s'", type, e.what());
	}
	return RequestResult::Error(RequestStatus::Unknown);
}

/* SetInputAudioMonitorType; false (and a message) unless it succeeds */
static bool set_type(const char *type)
{
	RequestResult r = call(set_monitor_type(), "SetInputAudioMonitorType",
			       json{{"inputName", input_name}, {"monitorType", type}});
	if (r.StatusCode != RequestStatus::Success) {
		fprintf(stderr, "SetInputAudioMonitorType %s: status %d, '%s'\n", type, (int)r.StatusCode,
			r.Comment.c_str());
		return false;
	}
	return true;
}

/* GetInputAudioMonitorType's monitorType, or "" (and a message) */
static std::string get_type(void)
{
	RequestResult r = call(get_monitor_type(), "GetInputAudioMonitorType", json{{"inputName", input_name}});
	if (r.StatusCode != RequestStatus::Success || !r.ResponseData.contains("monitorType") ||
	    !r.ResponseData["monitorType"].is_string()) {
		fprintf(stderr, "GetInputAudioMonitorType: status %d, '%s'\n", (int)r.StatusCode, r.Comment.c_str());
		return "";
	}
	return r.ResponseData["monitorType"].get<std::string>();
}

struct expected {
	const char *type; /* what GetInputAudioMonitorType reports */
	bool monitoring;
	bool muted;
	bool in_program;
};

/* Checks the input's state, puts it on the program and listens. Reports every
 * difference before returning false. */
static bool expect(const char *what, const expected &e)
{
	bool ok = true;

	std::string type = get_type();
	if (type != e.type) {
		fprintf(stderr, "%s: GetInputAudioMonitorType reports '%s', expected %s\n", what, type.c_str(), e.type);
		ok = false;
	}

	/* what is left of the previous step leaves the mix first */
	obs_set_output_source(0, nullptr);
	os_sleep_ms(100);
	reset_heard();
	obs_set_output_source(0, input);

	size_t program = 0, monitor = 0;
	int heard_monitor_at = -1;
	for (int waited = 0; waited < WAIT_MS; waited += 20) {
		os_sleep_ms(20);
		read_heard(&program, &monitor);
		if (heard_monitor_at < 0 && monitor >= WANT_FRAMES) {
			heard_monitor_at = waited;
		}
		if (e.in_program && program >= WANT_FRAMES && monitor >= WANT_FRAMES) {
			break;
		}
		if (!e.in_program && program) {
			break;
		}
		if (!e.in_program && heard_monitor_at >= 0 && waited - heard_monitor_at >= LISTEN_MS) {
			break;
		}
	}
	obs_set_output_source(0, nullptr);

	bool monitoring = obs_source_get_monitoring_enabled(input);
	bool muted = obs_source_muted(input);
	fprintf(stderr, "%s: reports %s, monitoring %s, muted %s, %zu program frames carry the tone\n", what,
		type.c_str(), monitoring ? "on" : "off", muted ? "yes" : "no", program);

	if (monitoring != e.monitoring) {
		fprintf(stderr, "%s: monitoring %s, expected %s\n", what, monitoring ? "on" : "off",
			e.monitoring ? "on" : "off");
		ok = false;
	}
	if (muted != e.muted) {
		fprintf(stderr, "%s: muted %s, expected %s\n", what, muted ? "yes" : "no", e.muted ? "yes" : "no");
		ok = false;
	}
	if (e.in_program && program < WANT_FRAMES) {
		fprintf(stderr, "%s: only %zu of %d program frames carry the tone\n", what, program, WANT_FRAMES);
		ok = false;
	}
	if (!e.in_program && program) {
		fprintf(stderr, "%s: the tone reached the program mix (%zu frames); it must only be monitored\n", what,
			program);
		ok = false;
	}
	if (monitor < WANT_FRAMES) {
		fprintf(stderr, "%s: only %zu of %d monitor feed frames carry the tone\n", what, monitor, WANT_FRAMES);
		ok = false;
	}
	return ok;
}

static const expected MONITOR_ONLY = {"OBS_MONITORING_TYPE_MONITOR_ONLY", true, true, false};
static const expected MONITOR_AND_OUTPUT = {"OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT", true, false, true};
static const expected NONE_UNMUTED = {"OBS_MONITORING_TYPE_NONE", false, false, true};
static const expected NONE_MUTED = {"OBS_MONITORING_TYPE_NONE", false, true, false};

/* ------------------------------------------------------------------------ */
/* cases */

/* #2447: Monitor Only keeps the tone out of the program and reads back */
static void monitor_only_stays_out_of_program(void **)
{
	if (!obs_up) {
		skip();
	}
	if (!set_type("OBS_MONITORING_TYPE_MONITOR_ONLY") || !expect("Monitor Only", MONITOR_ONLY)) {
		fail();
	}
}

/* the other two types read back and sound as they say */
static void other_types_round_trip(void **)
{
	if (!obs_up) {
		skip();
	}
	bool ok = true;
	ok &= set_type("OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT") && expect("Monitor and Output", MONITOR_AND_OUTPUT);
	ok &= set_type("OBS_MONITORING_TYPE_NONE") && expect("then None", NONE_UNMUTED);
	if (!ok) {
		fail();
	}
}

/* Changing the type away from Monitor Only undoes its mute: Monitor and Output
 * unmutes, and so does None, which plays the input into the outputs as it did
 * before 33.0. */
static void leaving_monitor_only_undoes_its_mute(void **)
{
	if (!obs_up) {
		skip();
	}
	bool ok = true;
	ok &= set_type("OBS_MONITORING_TYPE_MONITOR_ONLY") && expect("Monitor Only", MONITOR_ONLY);
	ok &= set_type("OBS_MONITORING_TYPE_NONE") && expect("Monitor Only, then None", NONE_UNMUTED);
	ok &= set_type("OBS_MONITORING_TYPE_MONITOR_ONLY") && expect("None, then Monitor Only", MONITOR_ONLY);
	ok &= set_type("OBS_MONITORING_TYPE_MONITOR_ONLY") && expect("Monitor Only twice", MONITOR_ONLY);
	ok &= set_type("OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT") &&
	      expect("Monitor Only, then Monitor and Output", MONITOR_AND_OUTPUT);
	if (!ok) {
		fail();
	}
}

/* None keeps a mute the user set, also one that was there before Monitor
 * Only; a source the user unmuted after Monitor Only stays unmuted. */
static void none_keeps_the_users_mute(void **)
{
	if (!obs_up) {
		skip();
	}
	bool ok = true;
	obs_source_set_muted(input, true);
	ok &= set_type("OBS_MONITORING_TYPE_NONE") && expect("muted, None", NONE_MUTED);
	ok &= set_type("OBS_MONITORING_TYPE_MONITOR_ONLY") && expect("muted, Monitor Only", MONITOR_ONLY);
	ok &= set_type("OBS_MONITORING_TYPE_NONE") && expect("muted, Monitor Only, then None", NONE_MUTED);

	obs_source_set_muted(input, false);
	ok &= set_type("OBS_MONITORING_TYPE_MONITOR_ONLY") && expect("Monitor Only", MONITOR_ONLY);
	obs_source_set_muted(input, false);
	ok &= expect("Monitor Only, unmuted by the user", MONITOR_AND_OUTPUT);
	ok &= set_type("OBS_MONITORING_TYPE_NONE") && expect("unmuted by the user, then None", NONE_UNMUTED);
	if (!ok) {
		fail();
	}
}

/* Get reports the 33.0 state however it was set: monitoring on and muted is
 * Monitor Only, on and unmuted is Monitor and Output. */
static void get_reports_monitoring_and_mute(void **)
{
	if (!obs_up) {
		skip();
	}
	bool ok = true;
	obs_source_set_monitoring_enabled(input, true);
	obs_source_set_muted(input, true);
	ok &= expect("monitoring on, muted", MONITOR_ONLY);
	obs_source_set_muted(input, false);
	ok &= expect("monitoring on, unmuted", MONITOR_AND_OUTPUT);
	obs_source_set_monitoring_enabled(input, false);
	ok &= expect("monitoring off", NONE_UNMUTED);
	if (!ok) {
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(monitor_only_stays_out_of_program, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(other_types_round_trip, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(leaving_monitor_only_undoes_its_mute, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(none_keeps_the_users_mute, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(get_reports_monitoring_and_mute, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
