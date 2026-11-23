/* Drive the installed VST3 host through the public libobs filter API.
 * The fixture is Steinberg's MIT-licensed AGain sample set to 50% gain. */
#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>

#include <math.h>
#include <stdio.h>
#include <string.h>

#define SAMPLE_RATE 48000
#define BLOCK_FRAMES 240
#define SOURCE_ID "vst3_test_audio"
#define FILTER_ID "vst3_filter"

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

static bool verify_gain(obs_source_t *source, struct capture *capture)
{
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
		if (capture->frames && fabsf(capture->first_sample - 0.25f) < 0.00001f &&
		    capture->timestamp <= audio.timestamp)
			return true;
	}
	fprintf(stderr, "Expected gain 0.5, got %.6f from %u frames\n", (double)capture->first_sample, capture->frames);
	return false;
}

static bool test_host(const char *module_path, const char *data_path, const char *class_id)
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
	};
	obs_register_source(&source_info);

	struct obs_audio_info info = {.samples_per_sec = SAMPLE_RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&info))
		return false;
	obs_source_t *source = obs_source_create(SOURCE_ID, "Gain input", NULL, NULL);
	obs_source_t *filter = obs_source_create_private(FILTER_ID, "Discover gain", NULL);
	bool found = filter && discovers_class(filter, class_id);
	obs_source_release(filter);
	if (!source || !found) {
		fprintf(stderr, "The scanner did not discover class %s\n", class_id);
		obs_source_release(source);
		return false;
	}

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "vst3_plugin", class_id);
	/* AGain's little-endian state is gain (float), reduction (float), bypass (int32). */
	obs_data_set_string(settings, "vst3_state", "0000003f0000000000000000");
	filter = obs_source_create_private(FILTER_ID, "Gain", settings);
	obs_data_release(settings);
	struct capture capture = {0};
	if (filter) {
		obs_source_filter_add(source, filter);
		obs_source_add_audio_capture_callback(source, capture_audio, &capture);
	}
	bool passed = filter && verify_gain(source, &capture);
	if (filter) {
		obs_source_remove_audio_capture_callback(source, capture_audio, &capture);
		obs_source_filter_remove(source, filter);
	}
	obs_source_release(filter);
	obs_source_release(source);
	return passed;
}

int main(int argc, char **argv)
{
	if (argc != 5) {
		fprintf(stderr, "usage: %s MODULE LOCALES CONFIG CLASS_ID\n", argv[0]);
		return 2;
	}

	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
	if (!obs_startup("en-US", argv[3], NULL))
		return 2;
	bool passed = test_host(argv[1], argv[2], argv[4]);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return passed ? 0 : 1;
}
