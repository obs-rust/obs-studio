/* A headless runtime workload for libobs (#3385): the before/after
 * measurement for compiler-flag changes such as IPO (#3193), cross-language
 * ThinLTO (#3217) and PGO (#3219), which change no behaviour and need a
 * steady per-frame workload instead of a unit test.
 *
 * It starts libobs with libobs-opengl at 1280x720, 60 fps, NV12 output and
 * 48 kHz stereo, and builds one scene with:
 * - solid-colour sources drawn through OBS_EFFECT_SOLID, as items with
 *   position, scale, rotation and crop;
 * - an async video source fed BGRA frames from a thread;
 * - audio sources fed 10 ms float packets from a thread, with volume and
 *   balance set;
 * plus a raw video callback (GPU conversion to NV12 and readback) and a raw
 * audio callback on mix 0, so the output and mixer paths run.
 *
 * After a warm-up it runs for --seconds and prints one JSON object: libobs'
 * average frame time, the frames rendered, lagged and skipped, the process
 * CPU seconds per second, and the video and audio callbacks per second.
 *
 *   obs_runtime_bench [--seconds N] [--warmup N]
 *
 * Needs a display (xvfb-run on CI); exits 77 (ctest skip) without one. */

#include <dirent.h>
#include <errno.h>
#include <math.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>
#include <util/threading.h>

#define WIDTH 1280
#define HEIGHT 720
#define COLOR_SOURCES 8
#define AUDIO_SOURCES 4
#define ASYNC_W 960
#define ASYNC_H 540

/* ------------------------------------------------------------------------- */
/* solid colour source */

struct color {
	struct vec4 value;
	uint32_t cx, cy;
};

static const char *color_name(void *unused)
{
	(void)unused;
	return "bench color";
}

static void *color_create(obs_data_t *settings, obs_source_t *source)
{
	(void)source;
	struct color *c = bzalloc(sizeof(*c));
	uint32_t rgba = (uint32_t)obs_data_get_int(settings, "color");
	vec4_from_rgba(&c->value, rgba);
	c->cx = (uint32_t)obs_data_get_int(settings, "width");
	c->cy = (uint32_t)obs_data_get_int(settings, "height");
	return c;
}

static void color_destroy(void *data)
{
	bfree(data);
}

static uint32_t color_width(void *data)
{
	return ((struct color *)data)->cx;
}

static uint32_t color_height(void *data)
{
	return ((struct color *)data)->cy;
}

static void color_render(void *data, gs_effect_t *effect)
{
	(void)effect;
	struct color *c = data;
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &c->value);
	gs_technique_t *tech = gs_effect_get_technique(solid, "Solid");
	gs_technique_begin(tech);
	gs_technique_begin_pass(tech, 0);
	gs_draw_sprite(0, 0, c->cx, c->cy);
	gs_technique_end_pass(tech);
	gs_technique_end(tech);
}

static struct obs_source_info color_info = {
	.id = "bench_color",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = color_name,
	.create = color_create,
	.destroy = color_destroy,
	.get_width = color_width,
	.get_height = color_height,
	.video_render = color_render,
};

/* ------------------------------------------------------------------------- */
/* async video and audio feeders */

static volatile bool feeding;

static const char *async_name(void *unused)
{
	(void)unused;
	return "bench async";
}

static void *async_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void async_destroy(void *data)
{
	(void)data;
}

static struct obs_source_info async_info = {
	.id = "bench_async",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO,
	.get_name = async_name,
	.create = async_create,
	.destroy = async_destroy,
};

static const char *audio_name(void *unused)
{
	(void)unused;
	return "bench audio";
}

static struct obs_source_info audio_info = {
	.id = "bench_audio",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = audio_name,
	.create = async_create,
	.destroy = async_destroy,
};

static void *video_feeder(void *data)
{
	obs_source_t *source = data;
	uint8_t *pixels = bmalloc((size_t)ASYNC_W * ASYNC_H * 4);
	struct obs_source_frame frame = {
		.data = {pixels},
		.linesize = {ASYNC_W * 4},
		.width = ASYNC_W,
		.height = ASYNC_H,
		.format = VIDEO_FORMAT_BGRA,
	};
	uint64_t next = os_gettime_ns();
	for (uint32_t n = 0; feeding; n++) {
		memset(pixels, (int)(n & 0xff), (size_t)ASYNC_W * ASYNC_H * 4);
		frame.timestamp = next;
		obs_source_output_video(source, &frame);
		next += 1000000000ULL / 60;
		os_sleepto_ns(next);
	}
	bfree(pixels);
	return NULL;
}

struct audio_feed {
	obs_source_t *sources[AUDIO_SOURCES];
};

static void *audio_feeder(void *data)
{
	struct audio_feed *feed = data;
	enum { FRAMES = 480 };
	float *left = bmalloc(FRAMES * sizeof(float));
	float *right = bmalloc(FRAMES * sizeof(float));
	uint64_t next = os_gettime_ns();
	double phase = 0.0;
	while (feeding) {
		for (int i = 0; i < FRAMES; i++) {
			left[i] = (float)sin(phase);
			right[i] = (float)cos(phase);
			phase += 2.0 * M_PI * 440.0 / 48000.0;
		}
		struct obs_source_audio audio = {
			.data = {(uint8_t *)left, (uint8_t *)right},
			.frames = FRAMES,
			.speakers = SPEAKERS_STEREO,
			.format = AUDIO_FORMAT_FLOAT_PLANAR,
			.samples_per_sec = 48000,
			.timestamp = next,
		};
		for (int s = 0; s < AUDIO_SOURCES; s++)
			obs_source_output_audio(feed->sources[s], &audio);
		next += 10000000ULL;
		os_sleepto_ns(next);
	}
	bfree(left);
	bfree(right);
	return NULL;
}

/* ------------------------------------------------------------------------- */
/* output callbacks */

static volatile long video_frames;
static volatile long audio_ticks;

static void on_video(void *param, struct video_data *frame)
{
	(void)param;
	(void)frame;
	os_atomic_inc_long(&video_frames);
}

static void on_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	(void)param;
	(void)mix_idx;
	(void)data;
	os_atomic_inc_long(&audio_ticks);
}

/* ------------------------------------------------------------------------- */

static double cpu_seconds(void)
{
	struct rusage usage;
	getrusage(RUSAGE_SELF, &usage);
	return (double)usage.ru_utime.tv_sec + (double)usage.ru_utime.tv_usec / 1e6 + (double)usage.ru_stime.tv_sec +
	       (double)usage.ru_stime.tv_usec / 1e6;
}

/* CPU seconds of this process's threads whose name starts with `prefix`
 * (Linux truncates thread names to 15 characters). Mesa's software
 * rasteriser threads ("llvmpipe-N") are counted apart, so a compiler change
 * in libobs is read from the libobs threads, not from Mesa's work. */
static double thread_cpu(const char *prefix)
{
	const double tick = (double)sysconf(_SC_CLK_TCK);
	double total = 0.0;
	DIR *dir = opendir("/proc/self/task");
	if (!dir)
		return 0.0;
	struct dirent *entry;
	while ((entry = readdir(dir)) != NULL) {
		if (entry->d_name[0] == '.')
			continue;
		char path[300], comm[32] = {0}, stat[1024] = {0};
		snprintf(path, sizeof(path), "/proc/self/task/%s/comm", entry->d_name);
		FILE *f = fopen(path, "r");
		if (!f)
			continue;
		if (!fgets(comm, sizeof(comm), f))
			comm[0] = 0;
		fclose(f);
		if (strncmp(comm, prefix, strlen(prefix)) != 0)
			continue;
		snprintf(path, sizeof(path), "/proc/self/task/%s/stat", entry->d_name);
		f = fopen(path, "r");
		if (!f)
			continue;
		if (fgets(stat, sizeof(stat), f)) {
			/* Fields 14 and 15 (utime, stime) follow the ")" that ends the name. */
			const char *rest = strrchr(stat, ')');
			unsigned long utime = 0, stime = 0;
			if (rest && sscanf(rest + 2, "%*c %*d %*d %*d %*d %*d %*u %*u %*u %*u %*u %lu %lu", &utime,
					   &stime) == 2)
				total += (double)(utime + stime) / tick;
		}
		fclose(f);
	}
	closedir(dir);
	return total;
}

struct thread_times {
	double graphics, audio, video, llvmpipe;
};

static struct thread_times thread_times(void)
{
	struct thread_times t = {
		thread_cpu("libobs: graphic"),
		thread_cpu("audio-io: audio"),
		thread_cpu("video-io: video"),
		thread_cpu("llvmpipe"),
	};
	return t;
}

static obs_source_t *add_color(obs_scene_t *scene, uint32_t rgba, uint32_t cx, uint32_t cy, int i)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "color", rgba);
	obs_data_set_int(settings, "width", cx);
	obs_data_set_int(settings, "height", cy);
	char name[32];
	snprintf(name, sizeof(name), "color %d", i);
	obs_source_t *source = obs_source_create("bench_color", name, settings, NULL);
	obs_data_release(settings);

	obs_sceneitem_t *item = obs_scene_add(scene, source);
	struct vec2 pos = {(float)(i * 180 % WIDTH), (float)(i * 97 % HEIGHT)};
	struct vec2 scale = {0.5f + 0.1f * (float)i, 0.75f};
	struct obs_sceneitem_crop crop = {i, i * 2, 0, i};
	obs_sceneitem_set_pos(item, &pos);
	obs_sceneitem_set_scale(item, &scale);
	obs_sceneitem_set_rot(item, (float)(i * 11));
	obs_sceneitem_set_crop(item, &crop);
	return source;
}

int main(int argc, char **argv)
{
	double seconds = 10.0, warmup = 2.0;
	for (int i = 1; i + 1 < argc; i += 2) {
		if (strcmp(argv[i], "--seconds") == 0)
			seconds = atof(argv[i + 1]);
		else if (strcmp(argv[i], "--warmup") == 0)
			warmup = atof(argv[i + 1]);
	}

	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "obs_runtime_bench: no display; skipping\n");
		return 77;
	}
	if (!obs_startup("en-US", NULL, NULL))
		return 1;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 60,
		.fps_den = 1,
		.base_width = WIDTH,
		.base_height = HEIGHT,
		.output_width = WIDTH,
		.output_height = HEIGHT,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS || !obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_runtime_bench: core video/audio failed; skipping\n");
		obs_shutdown();
		return 77;
	}

	obs_register_source(&color_info);
	obs_register_source(&async_info);
	obs_register_source(&audio_info);

	obs_scene_t *scene = obs_scene_create("bench scene");
	obs_source_t *colors[COLOR_SOURCES];
	for (int i = 0; i < COLOR_SOURCES; i++)
		colors[i] = add_color(scene, 0xff000000u | (uint32_t)(0x203040 * (i + 1)), 640 + 80 * (uint32_t)i,
				      360 + 40 * (uint32_t)i, i);

	obs_source_t *async = obs_source_create("bench_async", "async", NULL, NULL);
	obs_sceneitem_t *async_item = obs_scene_add(scene, async);
	struct vec2 async_pos = {320.0f, 180.0f};
	obs_sceneitem_set_pos(async_item, &async_pos);

	struct audio_feed feed;
	for (int s = 0; s < AUDIO_SOURCES; s++) {
		char name[32];
		snprintf(name, sizeof(name), "audio %d", s);
		feed.sources[s] = obs_source_create("bench_audio", name, NULL, NULL);
		obs_source_set_volume(feed.sources[s], 0.5f + 0.1f * (float)s);
		obs_source_set_balance_value(feed.sources[s], 0.25f * (float)s);
		obs_scene_add(scene, feed.sources[s]);
	}

	obs_set_output_source(0, obs_scene_get_source(scene));

	struct video_scale_info conversion = {VIDEO_FORMAT_NV12, WIDTH, HEIGHT, VIDEO_RANGE_PARTIAL, VIDEO_CS_709};
	obs_add_raw_video_callback(&conversion, on_video, NULL);
	struct audio_convert_info audio_conv = {48000, AUDIO_FORMAT_FLOAT_PLANAR, SPEAKERS_STEREO, false};
	obs_add_raw_audio_callback(0, &audio_conv, on_audio, NULL);

	feeding = true;
	pthread_t video_thread, audio_thread;
	pthread_create(&video_thread, NULL, video_feeder, async);
	pthread_create(&audio_thread, NULL, audio_feeder, &feed);

	os_sleep_ms((uint32_t)(warmup * 1000.0));

	uint32_t total0 = obs_get_total_frames(), lagged0 = obs_get_lagged_frames();
	video_t *video = obs_get_video();
	uint32_t skipped0 = video_output_get_skipped_frames(video);
	long vframes0 = os_atomic_load_long(&video_frames), aticks0 = os_atomic_load_long(&audio_ticks);
	double cpu0 = cpu_seconds();
	struct thread_times threads0 = thread_times();
	uint64_t t0 = os_gettime_ns();

	os_sleep_ms((uint32_t)(seconds * 1000.0));

	double elapsed = (double)(os_gettime_ns() - t0) / 1e9;
	double cpu = cpu_seconds() - cpu0;
	struct thread_times threads1 = thread_times();
	uint64_t frame_ns = obs_get_average_frame_time_ns();
	uint32_t total = obs_get_total_frames() - total0;
	uint32_t lagged = obs_get_lagged_frames() - lagged0;
	uint32_t skipped = video_output_get_skipped_frames(video) - skipped0;
	long vframes = os_atomic_load_long(&video_frames) - vframes0;
	long aticks = os_atomic_load_long(&audio_ticks) - aticks0;

	feeding = false;
	pthread_join(video_thread, NULL);
	pthread_join(audio_thread, NULL);

	obs_remove_raw_audio_callback(0, on_audio, NULL);
	obs_remove_raw_video_callback(on_video, NULL);
	obs_set_output_source(0, NULL);
	for (int s = 0; s < AUDIO_SOURCES; s++)
		obs_source_release(feed.sources[s]);
	obs_source_release(async);
	for (int i = 0; i < COLOR_SOURCES; i++)
		obs_source_release(colors[i]);
	obs_scene_release(scene);
	obs_shutdown();

	printf("{\"seconds\": %.3f, \"average_frame_time_ns\": %llu, \"frames\": %u, \"lagged_frames\": %u, "
	       "\"skipped_frames\": %u, \"cpu_seconds_per_second\": %.4f, \"video_callbacks_per_second\": %.2f, "
	       "\"audio_ticks_per_second\": %.2f, \"thread_cpu_seconds_per_second\": {\"graphics\": %.4f, "
	       "\"audio\": %.4f, \"video\": %.4f, \"llvmpipe\": %.4f}}\n",
	       elapsed, (unsigned long long)frame_ns, total, lagged, skipped, cpu / elapsed, (double)vframes / elapsed,
	       (double)aticks / elapsed, (threads1.graphics - threads0.graphics) / elapsed,
	       (threads1.audio - threads0.audio) / elapsed, (threads1.video - threads0.video) / elapsed,
	       (threads1.llvmpipe - threads0.llvmpipe) / elapsed);
	return total > 0 ? 0 : 1;
}
