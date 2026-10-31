/* linux-v4l2: the OBS Virtual Microphone against a real sound server
 * (obs-rust/obs-studio#3186).
 *
 * The test starts its own pipewire, wireplumber and pipewire-pulse in a
 * private runtime directory (no desktop session, no hardware needed) and
 * drives the real v4l2-output.c microphone path: pactl loads the
 * module-pipe-source, OBS's audio goes into its FIFO, and pw-record records
 * it back from "OBS Virtual Microphone". The Virtual Camera's video half
 * needs a v4l2loopback kernel module that CI containers cannot load, so only
 * the microphone half runs here. Without the sound server tools the test is
 * skipped. */

/* v4l2-output.c needs it (versionsort) before the first system header. */
#define _GNU_SOURCE

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include <spawn.h>
#include <sys/wait.h>
#include <math.h>

#include "plugins/linux-v4l2/v4l2-output.c.h"

extern char **environ;

#define RATE 48000
#define PACKET_FRAMES 960
#define PACKETS 50

static char runtime[64];
static pid_t servers[3];
static bool have_server;

static pid_t spawn(char *const argv[], const char *out)
{
	posix_spawn_file_actions_t fa;
	posix_spawn_file_actions_init(&fa);
	posix_spawn_file_actions_addopen(&fa, STDOUT_FILENO, out, O_WRONLY | O_CREAT | O_APPEND, 0600);
	posix_spawn_file_actions_addopen(&fa, STDERR_FILENO, out, O_WRONLY | O_CREAT | O_APPEND, 0600);
	pid_t pid = -1;
	if (posix_spawnp(&pid, argv[0], &fa, NULL, argv, environ) != 0)
		pid = -1;
	posix_spawn_file_actions_destroy(&fa);
	return pid;
}

static bool tool_exists(const char *tool)
{
	char cmd[128];
	snprintf(cmd, sizeof(cmd), "command -v %s >/dev/null 2>&1", tool);
	return system(cmd) == 0;
}

/* The output of a shell command, or an empty string. */
static char *capture(const char *cmd)
{
	struct dstr out;
	dstr_init_copy(&out, "");
	FILE *fp = popen(cmd, "r");
	if (fp) {
		char line[1024];
		while (fgets(line, sizeof(line), fp))
			dstr_cat(&out, line);
		pclose(fp);
	}
	return out.array;
}

static int start_server(void **state)
{
	(void)state;
	const char *tools[] = {"pipewire", "wireplumber", "pipewire-pulse", "pactl", "pw-record"};
	for (size_t i = 0; i < sizeof(tools) / sizeof(tools[0]); i++) {
		if (!tool_exists(tools[i])) {
			fprintf(stderr, "skipped: %s is not installed\n", tools[i]);
			return 0;
		}
	}

	/* Short: PipeWire's socket path must fit in sun_path (108 bytes). */
	snprintf(runtime, sizeof(runtime), "/tmp/vmic-XXXXXX");
	if (!mkdtemp(runtime))
		return 0;
	setenv("XDG_RUNTIME_DIR", runtime, 1);
	setenv("PIPEWIRE_RUNTIME_DIR", runtime, 1);
	char pulse[96];
	snprintf(pulse, sizeof(pulse), "%s/pulse", runtime);
	setenv("PULSE_RUNTIME_PATH", pulse, 1);
	unsetenv("PULSE_SERVER");
	unsetenv("DBUS_SESSION_BUS_ADDRESS");

	char log[96];
	snprintf(log, sizeof(log), "%s/server.log", runtime);
	char *pw[] = {"pipewire", NULL};
	char *wp[] = {"wireplumber", NULL};
	char *pp[] = {"pipewire-pulse", NULL};
	servers[0] = spawn(pw, log);
	usleep(500000);
	servers[1] = spawn(wp, log);
	servers[2] = spawn(pp, log);

	for (int i = 0; i < 50 && !have_server; i++) {
		usleep(100000);
		have_server = system("pactl info >/dev/null 2>&1") == 0;
	}
	if (!have_server) {
		fprintf(stderr, "skipped: the private sound server did not start (see %s)\n", log);
		return 0;
	}

	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	assert_true(obs_reset_audio(&oai));
	return 0;
}

static int stop_server(void **state)
{
	(void)state;
	if (have_server)
		obs_shutdown();
	for (int i = 2; i >= 0; i--) {
		if (servers[i] > 0) {
			kill(servers[i], SIGTERM);
			waitpid(servers[i], NULL, 0);
		}
	}
	return 0;
}

static void vcam_init(struct virtualcam_data *vcam)
{
	memset(vcam, 0, sizeof(*vcam));
	vcam->device = -1;
	vcam->mic_fifo = -1;
	vcam->mic_lock = -1;
	vcam->audio_enabled = true;
}

static bool source_listed(const char *name)
{
	char *list = capture("pactl list sources short 2>/dev/null");
	bool found = vmic_source_listed(list, name);
	bfree(list);
	return found;
}

/* Counts the frames of a float stereo WAV that carry L=+0.5, R=-0.5, and
 * those with the channels swapped. */
static void count_tone(const char *path, size_t *tone, size_t *swapped)
{
	*tone = *swapped = 0;
	FILE *f = fopen(path, "rb");
	assert_non_null(f);
	static uint8_t buf[4 << 20];
	size_t n = fread(buf, 1, sizeof(buf), f);
	fclose(f);

	const uint8_t *data = memmem(buf, n, "data", 4);
	assert_non_null(data);
	data += 8;
	for (; data + 8 <= buf + n; data += 8) {
		float l, r;
		memcpy(&l, data, 4);
		memcpy(&r, data + 4, 4);
		if (fabsf(l - 0.5f) < 1e-3f && fabsf(r + 0.5f) < 1e-3f)
			(*tone)++;
		else if (fabsf(l + 0.5f) < 1e-3f && fabsf(r - 0.5f) < 1e-3f)
			(*swapped)++;
	}
}

static void obs_audio_reaches_an_app_recording_the_microphone(void **state)
{
	(void)state;
	if (!have_server)
		skip();

	struct virtualcam_data vcam;
	vcam_init(&vcam);
	vmic_start(&vcam);
	assert_true(vcam.mic_fifo >= 0);
	assert_true(source_listed("obs_virtual_microphone"));
	char *desc = capture("pactl list sources 2>/dev/null | grep -c 'Description: OBS Virtual Microphone$'");
	assert_int_equal(atoi(desc), 1);
	bfree(desc);

	char rec[96];
	snprintf(rec, sizeof(rec), "%s/rec.wav", runtime);
	char log[96];
	snprintf(log, sizeof(log), "%s/record.log", runtime);
	char *argv[] = {"pw-record", "--target",   "obs_virtual_microphone", "--format", "f32", "--rate", "48000",
			"--channels", "2",       rec, NULL};
	pid_t recorder = spawn(argv, log);
	assert_true(recorder > 0);
	usleep(500000);

	/* One second of OBS audio, packet by packet as libobs delivers it. */
	static float packet[PACKET_FRAMES * 2];
	for (size_t i = 0; i < PACKET_FRAMES; i++) {
		packet[2 * i] = 0.5f;
		packet[2 * i + 1] = -0.5f;
	}
	struct audio_data frames = {.frames = PACKET_FRAMES};
	frames.data[0] = (uint8_t *)packet;
	for (int i = 0; i < PACKETS; i++) {
		virtual_audio(&vcam, &frames);
		usleep(20000);
	}
	usleep(300000);
	kill(recorder, SIGINT);
	waitpid(recorder, NULL, 0);

	size_t tone, swapped;
	count_tone(rec, &tone, &swapped);
	/* Most of the second arrives; a packet that did not fit is dropped
	 * whole, never split or swapped. */
	assert_true(tone >= PACKET_FRAMES * PACKETS / 2);
	assert_int_equal(swapped, 0);

	char fifo[256];
	snprintf(fifo, sizeof(fifo), "%s", vcam.mic_path);
	vmic_stop(&vcam);
	assert_false(source_listed("obs_virtual_microphone"));
	assert_int_equal(access(fifo, F_OK), -1);
}

/* #3185: a second OBS running at the same time gets its own source. */
static void a_second_microphone_gets_its_own_name(void **state)
{
	(void)state;
	if (!have_server)
		skip();

	struct virtualcam_data first, second;
	vcam_init(&first);
	vcam_init(&second);
	vmic_start(&first);
	vmic_start(&second);
	assert_true(first.mic_fifo >= 0);
	assert_true(second.mic_fifo >= 0);
	assert_true(source_listed("obs_virtual_microphone"));
	assert_true(source_listed("obs_virtual_microphone_2"));

	vmic_stop(&second);
	vmic_stop(&first);
	assert_false(source_listed("obs_virtual_microphone_2"));
	assert_false(source_listed("obs_virtual_microphone"));
}

/* #3182: turned off in the dialog, no module is loaded at all. */
static void turned_off_loads_nothing(void **state)
{
	(void)state;
	if (!have_server)
		skip();

	struct virtualcam_data vcam;
	vcam_init(&vcam);
	vcam.audio_enabled = false;
	vmic_start(&vcam);
	assert_int_equal(vcam.mic_fifo, -1);
	assert_false(source_listed("obs_virtual_microphone"));
	vmic_stop(&vcam);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(obs_audio_reaches_an_app_recording_the_microphone),
		cmocka_unit_test(a_second_microphone_gets_its_own_name),
		cmocka_unit_test(turned_off_loads_nothing),
	};
	return cmocka_run_group_tests(tests, start_server, stop_server);
}
