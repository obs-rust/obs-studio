/* Drive the installed VST3 host through the public libobs filter API. */
#include <obs.h>
#if OBS_VST3_TEST_X11
#include <obs-nix-platform.h>
#endif
#include <util/platform.h>

#include <math.h>
#include <stdio.h>
#include <string.h>

#define SAMPLE_RATE 48000
#define BLOCK_FRAMES 240
#define SOURCE_ID "vst3_test_audio"
#define FILTER_ID "vst3_filter"

void *vst3_test_qt_start(int *argc, char **argv, bool editor);
void vst3_test_qt_drain(void);
void vst3_test_qt_stop(void *app);
bool vst3_test_qt_window_visible(void);
bool vst3_test_qt_close_window(void);

struct capture {
	uint32_t frames;
	float first_sample;
	uint64_t timestamp;
};

static const char *audio_name(void *unused)
{
	(void)unused;
	return "VST3 test input";
}

static void *audio_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void audio_destroy(void *data)
{
	(void)data;
}

static void capture_audio(void *data, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	(void)source;
	if (muted || !audio->frames)
		return;
	struct capture *capture = data;
	capture->frames += audio->frames;
	capture->first_sample = ((const float *)audio->data[0])[0];
	capture->timestamp = audio->timestamp;
}

static bool has_filter(void)
{
	const char *id;
	for (size_t i = 0; obs_enum_filter_types(i, &id); ++i) {
		if (strcmp(id, FILTER_ID) == 0)
			return true;
	}
	return false;
}

static bool discovers_class(obs_source_t *filter, const char *class_id)
{
	/* Poll the published scanner result, with the deadline used only as a failure bound. */
	const uint64_t deadline = os_gettime_ns() + 5000000000ULL;
	do {
		obs_properties_t *properties = obs_source_properties(filter);
		obs_property_t *effects = obs_properties_get(properties, "vst3_plugin");
		bool found = false;
		if (effects) {
			for (size_t i = 0; i < obs_property_list_item_count(effects); ++i) {
				if (strcmp(obs_property_list_item_string(effects, i), class_id) == 0) {
					found = true;
					break;
				}
			}
		}
		obs_properties_destroy(properties);
		if (found)
			return true;
		os_sleep_ms(10);
	} while (os_gettime_ns() < deadline);
	return false;
}

static bool verify_gain(obs_source_t *source, obs_source_t *filter, float gain)
{
	if (!filter)
		return false;
	struct capture capture = {0};
	obs_source_filter_add(source, filter);
	obs_source_add_audio_capture_callback(source, capture_audio, &capture);

	float samples[BLOCK_FRAMES];
	for (size_t i = 0; i < BLOCK_FRAMES; ++i)
		samples[i] = 0.5f;

	struct obs_source_audio audio = {
		.data = {(const uint8_t *)samples, (const uint8_t *)samples},
		.frames = BLOCK_FRAMES,
		.speakers = SPEAKERS_STEREO,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.samples_per_sec = SAMPLE_RATE,
		.timestamp = os_gettime_ns(),
	};

	/* The host buffers short inputs before the first processed block.
	 * Send a bounded deterministic stream and check the produced packets. */
	for (size_t i = 0; i < 12; ++i) {
		obs_source_output_audio(source, &audio);
		audio.timestamp += 5000000;
	}
	obs_source_remove_audio_capture_callback(source, capture_audio, &capture);
	obs_source_filter_remove(source, filter);

	bool passed = capture.frames >= BLOCK_FRAMES && fabsf(capture.first_sample - 0.5f * gain) < 0.00001f &&
		      capture.timestamp <= audio.timestamp;
	if (!passed)
		fprintf(stderr, "Expected gain %.2f, got sample %.6f from %u frames\n", (double)gain,
			(double)capture.first_sample, capture.frames);
	return passed;
}

static obs_source_t *create_filter(const char *class_id, const char *state)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "vst3_plugin", class_id);
	if (state)
		obs_data_set_string(settings, "vst3_state", state);
	obs_source_t *filter = obs_source_create_private(FILTER_ID, "Gain", settings);
	obs_data_release(settings);
	return filter;
}

static void report_probe_step(const char *message)
{
	fprintf(stderr, "%s\n", message);
	fflush(stderr);
}

static bool open_editor(obs_source_t *filter, obs_property_t *button)
{
	report_probe_step("Opening the VST3 editor through the filter property");
	if (!obs_property_button_clicked(button, filter))
		return false;
	report_probe_step("Checking the visible VST3 editor window");
	return vst3_test_qt_window_visible();
}

static bool close_editor(void)
{
	report_probe_step("Sending a close event to the VST3 editor window");
	if (!vst3_test_qt_close_window())
		return false;
	report_probe_step("Checking that the VST3 editor window closed");
	return !vst3_test_qt_window_visible();
}

static bool verify_editor(obs_source_t *filter)
{
	report_probe_step("Reading the VST3 filter's editor property");
	obs_properties_t *props = obs_source_properties(filter);
	obs_property_t *button = obs_properties_get(props, "vst3_open_gui");
	bool passed = button && obs_property_visible(button) && open_editor(filter, button) && close_editor() &&
		      open_editor(filter, button) && close_editor();
	obs_properties_destroy(props);
	if (passed)
		puts("Opened, closed and reopened the native VST3 editor");
	else
		fprintf(stderr, "The native VST3 editor could not be opened, closed and reopened\n");
	return passed;
}

static bool test_host(const char *module_path, const char *data_path, const char *class_id, const char *second_id,
		      bool editor)
{
	obs_module_t *module = NULL;
	if (obs_open_module(&module, module_path, data_path) != MODULE_SUCCESS || !obs_init_module(module) ||
	    !has_filter()) {
		fprintf(stderr, "The VST3 host did not register %s\n", FILTER_ID);
		return false;
	}

	const struct obs_source_info source_info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = audio_name,
		.create = audio_create,
		.destroy = audio_destroy,
	};
	obs_register_source(&source_info);

	struct obs_audio_info info = {.samples_per_sec = SAMPLE_RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&info))
		return false;
	obs_source_t *source = obs_source_create(SOURCE_ID, "Gain input", NULL, NULL);
	obs_source_t *filter = obs_source_create_private(FILTER_ID, "Discover gain", NULL);
	bool found = filter && discovers_class(filter, class_id) && discovers_class(filter, second_id);
	obs_source_release(filter);
	if (!source || !found) {
		fprintf(stderr, "The scanner did not discover both fixture classes\n");
		obs_source_release(source);
		return false;
	}

	/* Little-endian state with gain 0.5 and zeroed reserved bytes. */
	filter = create_filter(class_id, "0000003f0000000000000000");
	bool passed = verify_gain(source, filter, 0.5f);
	if (editor)
		passed = filter && verify_editor(filter) && passed;
	obs_data_t *saved = filter ? obs_save_source(filter) : NULL;
	if (editor)
		report_probe_step("Releasing the tested VST3 filter");
	obs_source_release(filter);
	filter = saved ? obs_load_private_source(saved) : NULL;
	obs_data_release(saved);
	passed = verify_gain(source, filter, 0.5f) && passed;
	obs_source_release(filter);

	filter = create_filter(second_id, NULL);
	passed = verify_gain(source, filter, 0.25f) && passed;
	obs_source_release(filter);

	filter = create_filter("00000000000000000000000000000000", NULL);
	passed = verify_gain(source, filter, 1.0f) && passed;
	obs_source_release(filter);
	obs_source_release(source);
	return passed;
}

int main(int argc, char **argv)
{
	const bool editor = argc == 7 && strcmp(argv[6], "--editor") == 0;
	if (argc != 6 && !editor) {
		fprintf(stderr, "usage: %s MODULE LOCALES CONFIG CLASS_ID SECOND_CLASS_ID [--editor]\n", argv[0]);
		return 2;
	}

#if OBS_VST3_TEST_X11
	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
#endif
	void *app = vst3_test_qt_start(&argc, argv, editor);
	bool passed = false;
	if (obs_startup("en-US", argv[3], NULL)) {
		passed = test_host(argv[1], argv[2], argv[4], argv[5], editor);
		if (editor)
			report_probe_step("Waiting for the OBS source destroy queue");
		obs_wait_for_destroy_queue();
		if (editor)
			report_probe_step("Draining deferred Qt plug-in destruction");
		vst3_test_qt_drain();
		if (editor)
			report_probe_step("Shutting down libobs");
		obs_shutdown();
	}
	if (editor)
		report_probe_step("Destroying the Qt test application");
	vst3_test_qt_stop(app);
	return passed ? 0 : 1;
}
