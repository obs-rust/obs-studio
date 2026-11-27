/* A mixer control (VolumeControl) that outlives its source.
 *
 * Closing OBS could crash in VolumeControl::onSourceActiveChanged()
 * (upstream obsproject/obs-studio#14262): OBSBasic::ClearSceneData() clears
 * the output sources, whose "deactivate" signals queue
 * onSourceActiveChanged(false) on every control, then frees the sources and
 * only then runs the posted events. VolumeControl kept the source's UUID as
 * the raw pointer obs_source_get_uuid() returned, which points into the
 * source and is freed with it, and onSourceActiveChanged() went on to emit
 * OBSBasic::mixerStatusChanged(uuid) after it had found the source gone: a
 * QString built from freed memory. setGlobalInMixer() emits the same pointer
 * without looking at the source at all.
 *
 * The test builds the real control (frontend/components/VolumeControl.cpp.hpp
 * and VolumeName.cpp.hpp) on Qt's offscreen platform against a real libobs
 * source, with OBSBasic, OBSApp, the meter and the slider replaced by the
 * stand-ins in volume-control-stubs/. obs_source_get_uuid() is wrapped
 * (--wrap, GNU ld) to hand the control a test-owned copy of the UUID; when
 * the source is destroyed the test overwrites that copy, as the allocator
 * may reuse the freed string, and keeps it allocated so a failure is an
 * assertion instead of undefined behaviour.
 *
 * obs_startup() needs a display on Linux (run under xvfb-run); the cases
 * skip when it cannot start. */

#include <QApplication>
#include <QMetaObject>
#include <QString>

#include <obs.h>
#include <obs.hpp>

#include "VolumeControl.hpp"
#include "volume-control-main.hpp"

#include <string>
#include <vector>

/* cmocka.h last: its fail() and inline macros break C++ standard headers. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
extern "C" {
#include <cmocka.h>
}

/* ---- the stand-in main window and application -------------------------- */

static OBSBasic *main_window;
static OBSApp *app;

OBSBasic *OBSBasic::Get()
{
	return main_window;
}

OBSApp *App()
{
	return app;
}

static std::vector<std::string> emitted;

/* ---- the source's UUID, owned by the test ------------------------------- */

static obs_source_t *watched;
static char uuid_copy[64];
static std::string original_uuid;

extern "C" {
const char *__real_obs_source_get_uuid(const obs_source_t *source);
EXPORT const char *__wrap_obs_source_get_uuid(const obs_source_t *source);

const char *__wrap_obs_source_get_uuid(const obs_source_t *source)
{
	if (source && source == watched) {
		return uuid_copy;
	}
	return __real_obs_source_get_uuid(source);
}
}

/* ---- a source ------------------------------------------------------------- */

static const char *audio_name(void *)
{
	return "test audio";
}

static void *audio_create(obs_data_t *, obs_source_t *source)
{
	return source;
}

static void audio_destroy(void *) {}

static bool obs_up;

static obs_source_t *create_watched_source(void)
{
	obs_source_t *source = obs_source_create_private("test_volume_control_audio", "Mic", nullptr);
	assert_non_null(source);

	original_uuid = __real_obs_source_get_uuid(source);
	assert_true(original_uuid.size() < sizeof(uuid_copy));
	strcpy(uuid_copy, original_uuid.c_str());
	watched = source;
	return source;
}

/* Release the last reference and wait until libobs has destroyed the
 * source; then scribble over the UUID the control was given, as the
 * allocator may once libobs has freed it. */
static void destroy_watched_source(obs_source_t *source)
{
	obs_source_release(source);
	obs_wait_for_destroy_queue();
	watched = nullptr;
	memset(uuid_copy, 'X', sizeof(uuid_copy) - 1);
	uuid_copy[sizeof(uuid_copy) - 1] = '\0';
}

static void set_source_active(VolumeControl *control, bool active)
{
	/* What the queued "activate"/"deactivate" signal runs. */
	assert_true(
		QMetaObject::invokeMethod(control, "onSourceActiveChanged", Qt::DirectConnection, Q_ARG(bool, active)));
}

static int setup(void **)
{
	emitted.clear();
	return 0;
}

/* Guard: a control whose source is alive reports its status changes with
 * the source's UUID. */
static void live_source_reports_its_uuid(void **)
{
	if (!obs_up) {
		skip();
	}
	obs_source_t *source = create_watched_source();
	VolumeControl *control = new VolumeControl(source, nullptr);
	emitted.clear();

	set_source_active(control, false);
	assert_int_equal(emitted.size(), 1);
	assert_string_equal(emitted[0].c_str(), original_uuid.c_str());

	control->setGlobalInMixer(true);
	assert_int_equal(emitted.size(), 2);
	assert_string_equal(emitted[1].c_str(), original_uuid.c_str());

	delete control;
	destroy_watched_source(source);
}

/* The shutdown order of upstream #14262: the source is destroyed before its
 * queued deactivate runs. The control finds the source gone and reports no
 * status change, instead of emitting the UUID it no longer owns. */
static void deactivate_after_source_destroyed_reports_nothing(void **)
{
	if (!obs_up) {
		skip();
	}
	obs_source_t *source = create_watched_source();
	VolumeControl *control = new VolumeControl(source, nullptr);
	destroy_watched_source(source);
	emitted.clear();

	set_source_active(control, false);
	for (const std::string &uuid : emitted) {
		print_message("mixerStatusChanged(\"%s\") after the source was destroyed\n", uuid.c_str());
	}
	assert_int_equal(emitted.size(), 0);

	delete control;
}

/* setGlobalInMixer() does not look at the source: once it is gone the
 * control still names it by the UUID it had, not by freed memory. */
static void global_change_after_source_destroyed_keeps_uuid(void **)
{
	if (!obs_up) {
		skip();
	}
	obs_source_t *source = create_watched_source();
	VolumeControl *control = new VolumeControl(source, nullptr);
	destroy_watched_source(source);
	emitted.clear();

	control->setGlobalInMixer(true);
	assert_int_equal(emitted.size(), 1);
	assert_string_equal(emitted[0].c_str(), original_uuid.c_str());

	delete control;
}

int main(int argc, char *argv[])
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication qt(argc, argv);

	OBSBasic basic;
	OBSApp obs_app;
	main_window = &basic;
	app = &obs_app;
	QObject::connect(&basic, &OBSBasic::mixerStatusChanged,
			 [](QString uuid) { emitted.push_back(uuid.toStdString()); });

	obs_up = obs_startup("en-US", nullptr, nullptr);
	if (obs_up) {
		struct obs_source_info info = {};
		info.id = "test_volume_control_audio";
		info.type = OBS_SOURCE_TYPE_INPUT;
		info.output_flags = OBS_SOURCE_AUDIO;
		info.get_name = audio_name;
		info.create = audio_create;
		info.destroy = audio_destroy;
		obs_register_source(&info);
	} else {
		print_message("obs_startup failed (no display?); skipping\n");
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(live_source_reports_its_uuid, setup),
		cmocka_unit_test_setup(deactivate_after_source_destroyed_reports_nothing, setup),
		cmocka_unit_test_setup(global_change_after_source_destroyed_keeps_uuid, setup),
	};
	int failed = cmocka_run_group_tests(tests, nullptr, nullptr);

	if (obs_up) {
		obs_shutdown();
	}
	return failed;
}
