#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <signal.h>
#include <sys/types.h>
#include <unistd.h>
#include <cmocka.h>

#include <obs.h>
#include <util/base.h>
#include <util/platform.h>

/* The samples PulseAudio audio monitoring plays, end to end. The monitor
 * converts to the sink's sample format and applies the source volume in that
 * format (pulseaudio_monitor_process_volume); above a volume of 1 the integer
 * formats must saturate to their rail (#1709), not wrap. The other pulse
 * tests check streams and devices, not the sample values.
 *
 * Runs libobs audio monitoring against a private PipeWire + WirePlumber +
 * pipewire-pulse stack started by test_pulse_monitor_device.sh (no hardware,
 * its own runtime dir and D-Bus). For each sample format it loads a null sink
 * in that format, records the sink's monitor with parec, and plays a constant
 * +0.75 (left) / -0.75 (right) at source volumes 0.5, 1 and 2, 0.3 s each.
 * The recording must hold at least 0.1 s of each expected level (float is
 * not clamped: 1.5 at volume 2); where in the recording they are (start-up
 * silence, latency) is not assumed. Without that stack or parec the test
 * exits 77 and ctest reports a skip. */

#define SOURCE_ID "pulse_monitor_samples_test_source"
#define RATE 48000
#define PACKET_FRAMES 480
#define STEP_PACKETS 30 /* 0.3 s per volume */
#define MIN_MATCH_FRAMES (RATE / 10)
#define LEVEL 0.75f

static bool obs_up;

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl <= LOG_WARNING) {
		vfprintf(stderr, msg, args);
		fputc('\n', stderr);
	}
}

static const char *test_source_name(void *type_data)
{
	(void)type_data;
	return "monitor samples test source";
}

static void *test_source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void test_source_destroy(void *data)
{
	(void)data;
}

/* Runs a command and returns the first line of its output in buf. */
static bool run(const char *cmd, char *buf, size_t size)
{
	FILE *p = popen(cmd, "r");
	if (!p)
		return false;
	if (buf && size) {
		buf[0] = 0;
		if (!fgets(buf, (int)size, p))
			buf[0] = 0;
		buf[strcspn(buf, "\n")] = 0;
	}
	return pclose(p) == 0;
}

struct format {
	const char *name;
	size_t bytes;
	bool saturates; /* integer formats clamp to their range */
	double tolerance;
};

static double sample_value(const struct format *f, const uint8_t *p)
{
	switch (f->bytes) {
	case 1:
		return ((int)p[0] - 128) / 128.0;
	case 2: {
		int16_t v;
		memcpy(&v, p, sizeof(v));
		return v / 32768.0;
	}
	default:
		if (f->saturates) {
			int32_t v;
			memcpy(&v, p, sizeof(v));
			return v / 2147483648.0;
		} else {
			float v;
			memcpy(&v, p, sizeof(v));
			return v;
		}
	}
}

/* The level the sink holds for `level` at `vol`: the integer formats stop at
 * their rail, one step below +1.0 and exactly -1.0. */
static double expected(const struct format *f, double level, double vol)
{
	double v = level * vol;
	if (f->saturates) {
		double top = f->bytes == 1 ? 127.0 / 128.0 : f->bytes == 2 ? 32767.0 / 32768.0 : 1.0;
		if (v > top)
			v = top;
		if (v < -1.0)
			v = -1.0;
	}
	return v;
}

static int load_sink(const char *name, const char *format)
{
	char cmd[256], out[64];
	snprintf(cmd, sizeof(cmd),
		 "pactl load-module module-null-sink sink_name=%s format=%s rate=%d channels=2 2>/dev/null", name,
		 format, RATE);
	if (!run(cmd, out, sizeof(out)) || !*out)
		return -1;
	return atoi(out);
}

static void unload_module(int module)
{
	char cmd[128];
	snprintf(cmd, sizeof(cmd), "pactl unload-module %d 2>/dev/null", module);
	run(cmd, NULL, 0);
}

/* The sample spec column of `pactl list short sinks` for `name`. */
static void sink_spec(const char *name, char *spec, size_t size)
{
	char line[256];
	spec[0] = 0;
	FILE *p = popen("pactl list short sinks 2>/dev/null", "r");
	if (!p)
		return;
	while (fgets(line, sizeof(line), p)) {
		char sink[128], driver[128], fmt[32];
		unsigned idx;
		if (sscanf(line, "%u %127s %127s %31s", &idx, sink, driver, fmt) == 4 && strcmp(sink, name) == 0) {
			snprintf(spec, size, "%s", fmt);
			break;
		}
	}
	pclose(p);
}

static bool has_source_output(void)
{
	char line[256];
	return run("pactl list short source-outputs 2>/dev/null", line, sizeof(line)) && *line;
}

static void play_steps(obs_source_t *source, const float *volumes, size_t count)
{
	static float left[PACKET_FRAMES], right[PACKET_FRAMES];
	for (size_t i = 0; i < PACKET_FRAMES; i++) {
		left[i] = LEVEL;
		right[i] = -LEVEL;
	}

	uint64_t t = os_gettime_ns();
	for (size_t step = 0; step < count; step++) {
		obs_source_set_volume(source, volumes[step]);
		for (int n = 0; n < STEP_PACKETS; n++) {
			struct obs_source_audio a = {
				.frames = PACKET_FRAMES,
				.speakers = SPEAKERS_STEREO,
				.format = AUDIO_FORMAT_FLOAT_PLANAR,
				.samples_per_sec = RATE,
				.timestamp = t,
			};
			a.data[0] = (const uint8_t *)left;
			a.data[1] = (const uint8_t *)right;
			obs_source_output_audio(source, &a);
			t += (uint64_t)PACKET_FRAMES * 1000000000ULL / RATE;
			os_sleepto_ns(t);
		}
	}
	/* Let the monitor's buffer play out. */
	os_sleepto_ns(t + 300000000ULL);
}

static uint8_t *read_file(const char *path, size_t *size)
{
	FILE *f = fopen(path, "rb");
	if (!f)
		return NULL;
	fseek(f, 0, SEEK_END);
	long len = ftell(f);
	fseek(f, 0, SEEK_SET);
	uint8_t *data = malloc(len > 0 ? (size_t)len : 1);
	*size = len > 0 ? fread(data, 1, (size_t)len, f) : 0;
	fclose(f);
	return data;
}

/* Prints the most common frames of a recording, for a failure message. */
static void print_levels(const struct format *f, const uint8_t *data, size_t frames)
{
	struct level {
		double l, r;
		size_t n;
	} seen[8];
	size_t count = 0, other = 0;
	size_t frame_bytes = f->bytes * 2;

	for (size_t i = 0; i < frames; i++) {
		double l = sample_value(f, data + i * frame_bytes);
		double r = sample_value(f, data + i * frame_bytes + f->bytes);
		size_t k = 0;
		for (; k < count; k++) {
			if (seen[k].l == l && seen[k].r == r) {
				seen[k].n++;
				break;
			}
		}
		if (k < count)
			continue;
		if (count < 8)
			seen[count++] = (struct level){l, r, 1};
		else
			other++;
	}
	for (size_t k = 0; k < count; k++)
		fprintf(stderr, "  L %+.5f R %+.5f: %zu frames\n", seen[k].l, seen[k].r, seen[k].n);
	if (other)
		fprintf(stderr, "  %zu other frames\n", other);
}

static void check_format(const struct format *f)
{
	if (!obs_up)
		skip();

	static const float volumes[] = {0.5f, 1.0f, 2.0f};
	const size_t steps = sizeof(volumes) / sizeof(volumes[0]);
	char sink[64], spec[32], cmd[512], path[256], pid_line[32];
	const char *dir = getenv("XDG_RUNTIME_DIR");
	assert_non_null(dir);

	snprintf(sink, sizeof(sink), "obs_test_samples_%s", f->name);
	int module = load_sink(sink, f->name);
	if (module < 0) {
		fprintf(stderr, "could not load a %s null sink\n", f->name);
		fail();
	}
	sink_spec(sink, spec, sizeof(spec));
	if (strcmp(spec, f->name) != 0) {
		fprintf(stderr, "null sink %s has format '%s', asked for %s\n", sink, spec, f->name);
		unload_module(module);
		fail();
	}

	snprintf(path, sizeof(path), "%s/samples-%s.raw", dir, f->name);
	snprintf(cmd, sizeof(cmd),
		 "parec -d %s.monitor --format=%s --rate=%d --channels=2 --raw --latency-msec=20 >'%s' "
		 "2>/dev/null & echo $!",
		 sink, f->name, RATE, path);
	if (!run(cmd, pid_line, sizeof(pid_line)) || atoi(pid_line) <= 0) {
		fprintf(stderr, "could not start parec\n");
		unload_module(module);
		fail();
	}
	pid_t parec = (pid_t)atoi(pid_line);
	bool recording = false;
	for (int i = 0; i < 60 && !recording; i++) {
		recording = has_source_output();
		if (!recording)
			usleep(50000);
	}

	if (recording && obs_set_audio_monitoring_device(sink, sink)) {
		obs_source_t *source = obs_source_create_private(SOURCE_ID, "monitored", NULL);
		assert_non_null(source);
		obs_source_set_monitoring_enabled(source, true);
		/* The monitor plays only active sources. */
		obs_set_output_source(0, source);
		play_steps(source, volumes, steps);
		obs_set_output_source(0, NULL);
		obs_source_release(source);
	}

	kill(parec, SIGINT);
	usleep(200000);
	unload_module(module);
	if (!recording) {
		fprintf(stderr, "parec did not connect to %s.monitor\n", sink);
		fail();
	}

	size_t size = 0;
	uint8_t *data = read_file(path, &size);
	unlink(path);
	assert_non_null(data);
	size_t frame_bytes = f->bytes * 2;
	size_t frames = size / frame_bytes;

	bool ok = true;
	for (size_t s = 0; s < steps; s++) {
		double want_l = expected(f, LEVEL, volumes[s]);
		double want_r = expected(f, -LEVEL, volumes[s]);
		size_t match = 0;
		for (size_t i = 0; i < frames; i++) {
			double l = sample_value(f, data + i * frame_bytes);
			double r = sample_value(f, data + i * frame_bytes + f->bytes);
			if (l > want_l - f->tolerance && l < want_l + f->tolerance && r > want_r - f->tolerance &&
			    r < want_r + f->tolerance)
				match++;
		}
		if (match < MIN_MATCH_FRAMES) {
			fprintf(stderr, "%s, volume %.1f: %zu frames at L %+.5f R %+.5f, expected at least %d\n",
				f->name, volumes[s], match, want_l, want_r, MIN_MATCH_FRAMES);
			ok = false;
		}
	}
	if (!ok) {
		fprintf(stderr, "%s recording, %zu frames:\n", f->name, frames);
		print_levels(f, data, frames);
	}
	free(data);
	assert_true(ok);
}

static void monitor_samples_s16le(void **state)
{
	(void)state;
	static const struct format f = {"s16le", 2, true, 2.0 / 32768.0};
	check_format(&f);
}

static void monitor_samples_s32le(void **state)
{
	(void)state;
	static const struct format f = {"s32le", 4, true, 1e-4};
	check_format(&f);
}

static void monitor_samples_u8(void **state)
{
	(void)state;
	static const struct format f = {"u8", 1, true, 1.5 / 128.0};
	check_format(&f);
}

static void monitor_samples_float32le(void **state)
{
	(void)state;
	static const struct format f = {"float32le", 4, false, 1e-4};
	check_format(&f);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	/* obs_startup() needs a display on Linux; the cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}

	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = test_source_name,
		.create = test_source_create,
		.destroy = test_source_destroy,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	if (!getenv("PULSE_SERVER")) {
		fprintf(stderr, "PULSE_SERVER is not set: run through test_pulse_monitor_device.sh\n");
		return 77;
	}
	if (system("command -v parec >/dev/null 2>&1") != 0) {
		fprintf(stderr, "parec not found: skipping\n");
		return 77;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(monitor_samples_s16le),
		cmocka_unit_test(monitor_samples_s32le),
		cmocka_unit_test(monitor_samples_u8),
		cmocka_unit_test(monitor_samples_float32le),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
