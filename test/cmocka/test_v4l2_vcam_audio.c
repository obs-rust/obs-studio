/* linux-v4l2: the Virtual Camera also sends OBS's audio mix to an
 * "OBS Virtual Microphone" (obs-rust/obs-studio#3011). The audio goes into the
 * FIFO of a PulseAudio/PipeWire module-pipe-source loaded with pactl.
 *
 * The real v4l2-output.c is compiled in. A real non-blocking pipe stands in
 * for the module's FIFO, so no sound server is needed: a packet that does not
 * fit whole is dropped (never split, never blocked on), so the reader always
 * sees whole frames, and pactl's "load-module" reply is parsed for the
 * module index that "unload-module" needs. */

/* v4l2-output.c needs it (versionsort) before the first system header. */
#define _GNU_SOURCE

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <math.h>

#include "plugins/linux-v4l2/v4l2-output.c.h"

static int fds[2];
static size_t pipe_size;
static struct vmic_tail tail;

static int setup_pipe(void **state)
{
	(void)state;
	assert_int_equal(pipe2(fds, O_NONBLOCK | O_CLOEXEC), 0);
	int sz = fcntl(fds[1], F_SETPIPE_SZ, 4096);
	assert_true(sz >= 4096);
	pipe_size = (size_t)sz;
	return 0;
}

static int teardown_pipe(void **state)
{
	(void)state;
	close(fds[0]);
	close(fds[1]);
	vmic_tail_free(&tail);
	return 0;
}

static void packet_that_fits_is_written_whole(void **state)
{
	(void)state;
	uint8_t packet[1024];
	for (size_t i = 0; i < sizeof(packet); i++)
		packet[i] = (uint8_t)i;

	assert_true(vmic_write_packet(fds[1], pipe_size, &tail, packet, sizeof(packet)));

	uint8_t got[2048];
	ssize_t n = read(fds[0], got, sizeof(got));
	assert_int_equal(n, (ssize_t)sizeof(packet));
	assert_memory_equal(got, packet, sizeof(packet));
}

static void packet_that_does_not_fit_is_dropped_whole(void **state)
{
	(void)state;
	uint8_t packet[1000];
	memset(packet, 0xAB, sizeof(packet));

	size_t written = 0;
	size_t accepted = 0;
	for (int i = 0; i < 16; i++) {
		if (vmic_write_packet(fds[1], pipe_size, &tail, packet, sizeof(packet))) {
			accepted++;
			written += sizeof(packet);
		}
	}
	/* The pipe holds only whole packets: nothing was split, and the
	 * writer stopped once the next packet no longer fit. */
	assert_true(accepted >= 1);
	assert_true(accepted < 16);
	assert_true(written <= pipe_size);

	int queued = 0;
	assert_int_equal(ioctl(fds[0], FIONREAD, &queued), 0);
	assert_int_equal((size_t)queued, written);
	assert_int_equal((size_t)queued % sizeof(packet), 0);

	/* Once the reader drains it, packets go through again. */
	uint8_t drain[8192];
	while (read(fds[0], drain, sizeof(drain)) > 0)
		;
	assert_true(vmic_write_packet(fds[1], pipe_size, &tail, packet, sizeof(packet)));
}

static void closed_fifo_is_not_written(void **state)
{
	(void)state;
	uint8_t packet[16] = {0};
	assert_false(vmic_write_packet(-1, pipe_size, &tail, packet, sizeof(packet)));
}

static void module_index_is_parsed_from_pactl_reply(void **state)
{
	(void)state;
	uint32_t id = 0;
	assert_true(vmic_parse_module_id("536870913\n", &id));
	assert_int_equal(id, 536870913u);
	assert_true(vmic_parse_module_id("27", &id));
	assert_int_equal(id, 27u);

	assert_false(vmic_parse_module_id("", &id));
	assert_false(vmic_parse_module_id(NULL, &id));
	assert_false(vmic_parse_module_id("Failure: Module initialization failed\n", &id));
	assert_false(vmic_parse_module_id("12abc\n", &id));
	assert_false(vmic_parse_module_id("99999999999999\n", &id));
}

/* #3014: a module left by a killed OBS is found in "pactl list modules short"
 * by its FIFO name, which carries the pid of the OBS that loaded it. */
static void our_module_line_gives_id_and_pid(void **state)
{
	(void)state;
	uint32_t id = 0;
	long pid = 0;
	assert_true(vmic_parse_module_line(
		"536870916\tmodule-pipe-source\tsource_name=obs_virtual_microphone "
		"file=/run/user/1000/obs-virtual-microphone-969296 format=float32le rate=48000 channels=2 "
		"source_properties=\"device.description=\\\"OBS Virtual Microphone\\\"\"\t\n",
		&id, &pid));
	assert_int_equal(id, 536870916u);
	assert_int_equal(pid, 969296);
}

/* #4057: a module loaded through libpulse keeps the quotes around its path. */
static void quoted_module_line_gives_id_and_pid(void **state)
{
	(void)state;
	uint32_t id = 0;
	long pid = 0;
	assert_true(vmic_parse_module_line("536870917\tmodule-pipe-source\tsource_name=obs_virtual_microphone "
					   "file='/run/user/1000/obs-virtual-microphone/obs-virtual-microphone-4242' "
					   "format=float32le rate=48000 channels=2\t\n",
					   &id, &pid));
	assert_int_equal(id, 536870917u);
	assert_int_equal(pid, 4242);
}

static void other_module_lines_are_ignored(void **state)
{
	(void)state;
	uint32_t id = 0;
	long pid = 0;
	/* Someone else's pipe source. */
	assert_false(vmic_parse_module_line(
		"31\tmodule-pipe-source\tsource_name=mine file=/run/user/1000/mine format=s16le\t\n", &id, &pid));
	/* Not a pipe source, even with the name in its arguments. */
	assert_false(vmic_parse_module_line(
		"32\tmodule-loopback\tsource=obs_virtual_microphone file=/run/user/1000/obs-virtual-microphone-5\t\n",
		&id, &pid));
	/* No pid after the name. */
	assert_false(vmic_parse_module_line(
		"33\tmodule-pipe-source\tfile=/run/user/1000/obs-virtual-microphone- format=s16le\t\n", &id, &pid));
	assert_false(vmic_parse_module_line("", &id, &pid));
	assert_false(vmic_parse_module_line(NULL, &id, &pid));
}

/* #3180: pipe capacity counts in page-sized slots, so a packet that fits by
 * FIONREAD can still be written short when a partly read page holds the
 * queued bytes. The reader must never see half a packet followed by the next
 * one: that shifts every later sample (swapped channels, or noise). */
static void short_write_never_leaves_half_a_packet(void **state)
{
	(void)state;
	int p[2];
	assert_int_equal(pipe2(p, O_NONBLOCK | O_CLOEXEC), 0);
	int sz = fcntl(p[1], F_GETPIPE_SZ);
	assert_true(sz >= 8192);
	size_t size = (size_t)sz;

	/* One page slot with one byte left in it. */
	static uint8_t filler[4000];
	assert_int_equal(write(p[1], filler, sizeof(filler)), (ssize_t)sizeof(filler));
	assert_int_equal(read(p[0], filler, sizeof(filler) - 1), (ssize_t)sizeof(filler) - 1);

	/* Fits by FIONREAD (1 + packet <= size) but needs every slot. */
	size_t packet_size = size - 192;
	uint8_t *packet = malloc(packet_size);
	for (size_t i = 0; i < packet_size; i++)
		packet[i] = (uint8_t)(i * 7);

	uint8_t *drain = malloc(size * 4);
	size_t total = 0;
	for (int round = 0; round < 4; round++) {
		vmic_write_packet(p[1], size, &tail, packet, packet_size);
		ssize_t n;
		while ((n = read(p[0], drain + total, size * 4 - total)) > 0)
			total += (size_t)n;
	}
	/* Drop the filler byte, then the stream is whole packets only. */
	assert_true(total >= 1);
	total -= 1;
	assert_int_equal(total % packet_size, 0);
	for (size_t off = 0; off < total; off += packet_size)
		assert_memory_equal(drain + 1 + off, packet, packet_size);

	free(drain);
	free(packet);
	vmic_tail_free(&tail);
	close(p[0]);
	close(p[1]);
}

/* #3179: in Flatpak the number in the FIFO name is a pid from the sandbox's
 * namespace, so it says nothing about whether that OBS still runs. Each OBS
 * holds a lock next to its FIFO instead: a free lock means the OBS is gone,
 * whatever pid that number happens to name here. */
static void stale_is_decided_by_the_lock_not_the_pid(void **state)
{
	(void)state;
	char dir[] = "/tmp/vmic-lock-XXXXXX";
	assert_non_null(mkdtemp(dir));
	char lock[256];

	/* Number 1 names a live process here (init), but its lock is free. */
	snprintf(lock, sizeof(lock), "%s/obs-virtual-microphone-1.lock", dir);
	int fd = open(lock, O_RDWR | O_CREAT | O_CLOEXEC, 0600);
	assert_true(fd >= 0);
	close(fd);
	assert_true(vmic_instance_is_stale(dir, 1));
	unlink(lock);

	/* A number no process here has, but another OBS holds the lock. */
	long gone = 999999999;
	snprintf(lock, sizeof(lock), "%s/obs-virtual-microphone-%ld.lock", dir, gone);
	fd = open(lock, O_RDWR | O_CREAT | O_CLOEXEC, 0600);
	assert_true(fd >= 0);
	assert_int_equal(flock(fd, LOCK_EX | LOCK_NB), 0);
	assert_false(vmic_instance_is_stale(dir, gone));
	close(fd);
	unlink(lock);

	rmdir(dir);
}

/* #3797: a helper dir with one executable script in it, put first on PATH. */
static void write_script(const char *dir, const char *name, const char *body)
{
	char script[256];
	snprintf(script, sizeof(script), "%s/%s", dir, name);
	FILE *f = fopen(script, "w");
	assert_non_null(f);
	fputs(body, f);
	fclose(f);
	assert_int_equal(chmod(script, 0755), 0);
}

/* #3797: turning the microphone off must not keep a dead OBS's silent device
 * listed in every app: stale modules go away with the microphone off too. */
static void stale_module_is_unloaded_with_the_microphone_off(void **state)
{
	(void)state;
	char dir[] = "/tmp/vmic-off-XXXXXX";
	assert_non_null(mkdtemp(dir));

	char body[512];
	snprintf(
		body, sizeof(body),
		"#!/bin/sh\n"
		"echo \"$*\" >> %s/calls\n"
		"if [ \"$1\" = list ]; then\n"
		"  printf '77\\tmodule-pipe-source\\tfile=%s/obs-virtual-microphone/obs-virtual-microphone-999 format=x\\t\\n'\n"
		"fi\n",
		dir, dir);
	write_script(dir, "pactl", body);

	/* The dead owner's lock: present, and nobody holds it. */
	char mic_dir[256];
	snprintf(mic_dir, sizeof(mic_dir), "%s/obs-virtual-microphone", dir);
	assert_int_equal(mkdir(mic_dir, 0700), 0);
	char lock[320];
	snprintf(lock, sizeof(lock), "%s/obs-virtual-microphone-999.lock", mic_dir);
	int fd = open(lock, O_RDWR | O_CREAT, 0600);
	assert_true(fd >= 0);
	close(fd);

	const char *old_path = getenv("PATH");
	char *saved_path = old_path ? strdup(old_path) : NULL;
	const char *old_runtime = getenv("XDG_RUNTIME_DIR");
	char *saved_runtime = old_runtime ? strdup(old_runtime) : NULL;
	char path[1024];
	snprintf(path, sizeof(path), "%s:%s", dir, old_path ? old_path : "/usr/bin:/bin");
	setenv("PATH", path, 1);
	setenv("XDG_RUNTIME_DIR", dir, 1);
	/* The fake pactl is the sound server here: keep libpulse off any real one. */
	const char *old_server = getenv("PULSE_SERVER");
	char *saved_server = old_server ? strdup(old_server) : NULL;
	setenv("PULSE_SERVER", "unix:/nonexistent/native", 1);

	struct virtualcam_data vcam;
	memset(&vcam, 0, sizeof(vcam));
	vcam.mic_fifo = -1;
	vcam.mic_lock = -1;
	vcam.audio_enabled = false;
	vmic_start(&vcam);

	if (saved_path) {
		setenv("PATH", saved_path, 1);
		free(saved_path);
	}
	if (saved_server) {
		setenv("PULSE_SERVER", saved_server, 1);
		free(saved_server);
	} else {
		unsetenv("PULSE_SERVER");
	}
	if (saved_runtime) {
		setenv("XDG_RUNTIME_DIR", saved_runtime, 1);
		free(saved_runtime);
	} else {
		unsetenv("XDG_RUNTIME_DIR");
	}

	char calls_path[256];
	snprintf(calls_path, sizeof(calls_path), "%s/calls", dir);
	char calls[1024] = {0};
	FILE *f = fopen(calls_path, "r");
	if (f) {
		size_t got = fread(calls, 1, sizeof(calls) - 1, f);
		calls[got] = 0;
		fclose(f);
	}
	unlink(calls_path);
	unlink(lock);
	rmdir(mic_dir);
	char script[256];
	snprintf(script, sizeof(script), "%s/pactl", dir);
	unlink(script);
	rmdir(dir);

	assert_non_null(strstr(calls, "unload-module 77"));
	/* Off means off: no new device, and no lock kept for one. */
	assert_null(strstr(calls, "load-module module-pipe-source"));
	assert_int_equal(vcam.mic_lock, -1);
}

/* #3797: the log tells a pactl that cannot run from a load the server refused.
 * The fake timeout answers like the real one does for a missing command. */
static void pactl_that_cannot_run_is_told_apart(void **state)
{
	(void)state;
	char dir[] = "/tmp/vmic-nopactl-XXXXXX";
	assert_non_null(mkdtemp(dir));

	const char *old = getenv("PATH");
	char *saved = old ? strdup(old) : NULL;
	char path[1024];
	snprintf(path, sizeof(path), "%s:%s", dir, old ? old : "/usr/bin:/bin");
	setenv("PATH", path, 1);

	write_script(dir, "timeout", "#!/bin/sh\nexit 127\n");
	bool missing_runs = vmic_pactl_runs();

	write_script(dir, "timeout", "#!/bin/sh\nshift 3\nexec \"$@\"\n");
	write_script(dir, "pactl", "#!/bin/sh\necho 'pactl 17.0'\n");
	bool present_runs = vmic_pactl_runs();

	if (saved) {
		setenv("PATH", saved, 1);
		free(saved);
	}
	char script[256];
	snprintf(script, sizeof(script), "%s/timeout", dir);
	unlink(script);
	snprintf(script, sizeof(script), "%s/pactl", dir);
	unlink(script);
	rmdir(dir);

	assert_false(missing_runs);
	assert_true(present_runs);
}

/* #4057: the virtual microphone is made through libpulse, with no pactl
 * anywhere on PATH. Runs only under a private sound server
 * (test_pulse_monitor_device.py sets PULSE_SERVER); skipped otherwise. */
static void virtual_microphone_needs_no_pactl(void **state)
{
	(void)state;
	if (!getenv("PULSE_SERVER"))
		skip();

	const char *old = getenv("PATH");
	char *saved = old ? strdup(old) : NULL;
	setenv("PATH", "/nonexistent", 1);

	char dir[] = "/tmp/vmic-pa-XXXXXX";
	assert_non_null(mkdtemp(dir));
	char args[512];
	snprintf(args, sizeof(args),
		 "source_name=obs_vmic_pa_test file='%s/obs-virtual-microphone-4242' format=float32le rate=48000 "
		 "channels=2 source_properties=\"device.description=\\\"OBS Virtual Microphone\\\"\"",
		 dir);

	uint32_t id = 0;
	bool loaded = vmic_pa_load(args, &id);
	char *sources = loaded ? vmic_pa_list(false) : NULL;
	char *modules = loaded ? vmic_pa_list(true) : NULL;
	bool listed = sources && vmic_source_listed(sources, "obs_vmic_pa_test");
	/* The test server is pipewire-pulse: libpulse's list names the driver,
	 * so the pipe tunnel's buffer is found and held out (#4087). */
	uint64_t lead = sources ? vmic_tunnel_lead_ns(sources, 48000) : 0;
	bool found = false;
	for (char *line = modules, *next; line && *line; line = next) {
		next = strchr(line, '\n');
		if (next)
			*next++ = 0;
		uint32_t mid;
		long pid;
		if (vmic_parse_module_line(line, &mid, &pid) && mid == id && pid == 4242)
			found = true;
	}
	bool unloaded = loaded && vmic_pa_unload(id);
	char *after = unloaded ? vmic_pa_list(false) : NULL;
	bool gone = after && !vmic_source_listed(after, "obs_vmic_pa_test");
	bfree(sources);
	bfree(modules);
	bfree(after);

	if (saved) {
		setenv("PATH", saved, 1);
		free(saved);
	}
	char fifo[300];
	snprintf(fifo, sizeof(fifo), "%s/obs-virtual-microphone-4242", dir);
	unlink(fifo);
	rmdir(dir);

	assert_true(loaded);
	assert_true(listed);
	assert_true(found);
	assert_true(unloaded);
	assert_true(gone);
	assert_int_equal(lead, 170666666ULL);
}

/* #3181: pactl runs on the thread that starts the Virtual Camera. A sound
 * server that never answers must not freeze OBS: run_pactl gives up. */
static void hung_pactl_is_given_up_on(void **state)
{
	(void)state;
	char dir[] = "/tmp/vmic-pactl-XXXXXX";
	assert_non_null(mkdtemp(dir));
	char script[256];
	snprintf(script, sizeof(script), "%s/pactl", dir);
	FILE *f = fopen(script, "w");
	assert_non_null(f);
	fputs("#!/bin/sh\nexec sleep 60\n", f);
	fclose(f);
	assert_int_equal(chmod(script, 0755), 0);

	const char *old = getenv("PATH");
	char *saved = old ? strdup(old) : NULL;
	char path[1024];
	snprintf(path, sizeof(path), "%s:%s", dir, old ? old : "/usr/bin:/bin");
	setenv("PATH", path, 1);

	uint64_t start = os_gettime_ns();
	char *reply = run_pactl("info");
	uint64_t elapsed = os_gettime_ns() - start;

	if (saved) {
		setenv("PATH", saved, 1);
		free(saved);
	}
	unlink(script);
	rmdir(dir);

	assert_null(reply);
	assert_true(elapsed < 10000000000ULL);
}

/* #3182: the Virtual Camera dialog turns the microphone off and picks the
 * track; a collection saved before has neither key. */
static void dialog_settings_choose_audio_and_track(void **state)
{
	(void)state;
	struct virtualcam_data vcam = {0};

	obs_data_t *settings = obs_data_create();
	vmic_read_settings(&vcam, settings);
	assert_true(vcam.audio_enabled);
	assert_int_equal(vcam.audio_mixer, 0);

	obs_data_set_bool(settings, "audio_enabled", false);
	obs_data_set_int(settings, "audio_track", 3);
	vmic_read_settings(&vcam, settings);
	assert_false(vcam.audio_enabled);
	assert_int_equal(vcam.audio_mixer, 2);

	obs_data_set_int(settings, "audio_track", 9);
	vmic_read_settings(&vcam, settings);
	assert_int_equal(vcam.audio_mixer, 0);
	obs_data_release(settings);
}

/* #3185: a second running OBS gets its own source name instead of a second
 * source with the same one. */
static void a_second_obs_gets_its_own_source_name(void **state)
{
	(void)state;
	char name[64];

	assert_int_equal(vmic_pick_source_name("", name, sizeof(name)), 1);
	assert_string_equal(name, "obs_virtual_microphone");
	assert_int_equal(vmic_pick_source_name(NULL, name, sizeof(name)), 1);
	assert_string_equal(name, "obs_virtual_microphone");

	const char *one = "53\talsa_input.pci\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n"
			  "54\tobs_virtual_microphone\tPipeWire\tfloat32le 2ch 48000Hz\tRUNNING\n";
	assert_int_equal(vmic_pick_source_name(one, name, sizeof(name)), 2);
	assert_string_equal(name, "obs_virtual_microphone_2");

	/* A longer name that only starts with ours does not take it. */
	const char *prefix = "60\tobs_virtual_microphone_2\tPipeWire\tfloat32le 2ch 48000Hz\tIDLE\n";
	assert_int_equal(vmic_pick_source_name(prefix, name, sizeof(name)), 1);
	assert_string_equal(name, "obs_virtual_microphone");

	const char *two = "54\tobs_virtual_microphone\tPipeWire\tx\tRUNNING\n"
			  "55\tobs_virtual_microphone_2\tPipeWire\tx\tRUNNING\n";
	assert_int_equal(vmic_pick_source_name(two, name, sizeof(name)), 3);
	assert_string_equal(name, "obs_virtual_microphone_3");
}

/* #4087: frame sync. Pipes stand in for the camera device and the FIFO; a
 * frame is 16 bytes, so FIONREAD / 16 counts the frames the camera got. */
#define SYNC_FRAME 16
#define MS(x) ((uint64_t)(x) * 1000000ULL)

static int queued_bytes(int fd)
{
	int n = 0;
	assert_int_equal(ioctl(fd, FIONREAD, &n), 0);
	return n;
}

static struct virtualcam_data *sync_vcam(int dev[2], int mic[2])
{
	assert_int_equal(pipe2(dev, O_NONBLOCK | O_CLOEXEC), 0);
	assert_int_equal(pipe2(mic, O_NONBLOCK | O_CLOEXEC), 0);
	int size = fcntl(mic[1], F_SETPIPE_SZ, 1 << 16);
	assert_true(size >= 1 << 16);
	struct virtualcam_data *vcam = virtualcam_create(NULL, NULL);
	vcam->device = dev[1];
	vcam->frame_size = SYNC_FRAME;
	vcam->mic_fifo = mic[1];
	vcam->mic_pipe_size = (size_t)size;
	vcam->mic_rate = 48000;
	return vcam;
}

static void sync_done(struct virtualcam_data *vcam, int dev[2], int mic[2])
{
	virtualcam_destroy(vcam); /* closes dev[1] and mic[1] */
	close(dev[0]);
	close(mic[0]);
}

static void send_frame(struct virtualcam_data *vcam, uint64_t ts)
{
	uint8_t px[SYNC_FRAME] = {0};
	struct video_data frame = {0};
	frame.data[0] = px;
	frame.timestamp = ts;
	virtual_video(vcam, &frame);
}

static void send_audio(struct virtualcam_data *vcam, uint64_t ts, uint32_t frames)
{
	static float samples[1024 * 2];
	struct audio_data audio = {0};
	audio.data[0] = (uint8_t *)samples;
	audio.frames = frames;
	audio.timestamp = ts;
	virtual_audio(vcam, &audio);
}

/* A frame goes to the camera once the microphone has the audio up to its
 * timestamp, not before. */
static void video_waits_for_its_audio(void **state)
{
	(void)state;
	int dev[2], mic[2];
	struct virtualcam_data *vcam = sync_vcam(dev, mic);

	send_frame(vcam, MS(1000));
	send_frame(vcam, MS(1033));
	send_frame(vcam, MS(1066));
	assert_int_equal(queued_bytes(dev[0]), 0);

	/* 1024 frames at 48 kHz: 21.33 ms each. */
	send_audio(vcam, MS(1000), 1024); /* up to 1021.3 */
	assert_int_equal(queued_bytes(dev[0]), 1 * SYNC_FRAME);
	send_audio(vcam, 1021333333ULL, 1024); /* up to 1042.7 */
	assert_int_equal(queued_bytes(dev[0]), 2 * SYNC_FRAME);
	send_audio(vcam, 1042666667ULL, 1024); /* up to 1064 */
	assert_int_equal(queued_bytes(dev[0]), 2 * SYNC_FRAME);
	send_audio(vcam, 1064000000ULL, 1024); /* up to 1085.3 */
	assert_int_equal(queued_bytes(dev[0]), 3 * SYNC_FRAME);

	/* Audio already ahead: the next frame goes straight through. */
	send_frame(vcam, MS(1080));
	assert_int_equal(queued_bytes(dev[0]), 4 * SYNC_FRAME);

	sync_done(vcam, dev, mic);
}

/* Without a microphone nothing waits. */
static void video_is_not_held_without_microphone(void **state)
{
	(void)state;
	int dev[2], mic[2];
	struct virtualcam_data *vcam = sync_vcam(dev, mic);
	close(vcam->mic_fifo);
	vcam->mic_fifo = -1;

	send_frame(vcam, MS(1000));
	assert_int_equal(queued_bytes(dev[0]), SYNC_FRAME);

	sync_done(vcam, dev, mic);
}

/* Audio that stops never freezes the picture: past 128 waiting frames the
 * oldest goes out anyway. */
static void held_video_is_bounded(void **state)
{
	(void)state;
	int dev[2], mic[2];
	struct virtualcam_data *vcam = sync_vcam(dev, mic);

	for (int i = 0; i < 128 + 5; i++)
		send_frame(vcam, MS(1000) + MS(i * 33));
	assert_int_equal(queued_bytes(dev[0]), 5 * SYNC_FRAME);

	sync_done(vcam, dev, mic);
}

/* Audio nobody read is thrown away, oldest first, down to one packet, so an
 * app that links later starts at the newest audio. */
static void stale_audio_is_dropped(void **state)
{
	(void)state;
	int p[2];
	assert_int_equal(pipe2(p, O_NONBLOCK | O_CLOEXEC), 0);
	assert_true(fcntl(p[1], F_SETPIPE_SZ, 1 << 16) >= 1 << 16);

	uint8_t packet[8192];
	for (int i = 1; i <= 3; i++) {
		memset(packet, i, sizeof(packet));
		assert_int_equal(write(p[1], packet, sizeof(packet)), (ssize_t)sizeof(packet));
	}
	assert_int_equal(vmic_drop_stale(-1, p[1], sizeof(packet)), 0);
	assert_int_equal(vmic_drop_stale(p[0], p[1], sizeof(packet)), 2 * sizeof(packet));
	assert_int_equal(queued_bytes(p[0]), (int)sizeof(packet));
	assert_int_equal(read(p[0], packet, sizeof(packet)), (ssize_t)sizeof(packet));
	assert_int_equal(packet[0], 3);
	assert_int_equal(packet[sizeof(packet) - 1], 3);

	/* Within the limit nothing goes. */
	assert_int_equal(write(p[1], packet, 4096), 4096);
	assert_int_equal(vmic_drop_stale(p[0], p[1], sizeof(packet)), 0);

	close(p[0]);
	close(p[1]);
}

/* Under PipeWire the audio comes out of the pipe tunnel 8192 frames after
 * the FIFO; PulseAudio's own module adds nothing. */
static void pipewire_tunnel_lead_is_8192_frames(void **state)
{
	(void)state;
	const char *pipewire = "53\talsa_input.pci\tPipeWire\ts32le 2ch 48000Hz\tSUSPENDED\n";
	const char *pulse = "1\talsa_input.pci\tmodule-alsa-card.c\ts16le 2ch 44100Hz\tSUSPENDED\n";
	assert_int_equal(vmic_tunnel_lead_ns(pipewire, 48000), 170666666ULL);
	assert_int_equal(vmic_tunnel_lead_ns(pipewire, 44100), 185759637ULL);
	assert_int_equal(vmic_tunnel_lead_ns(pulse, 48000), 0);
	assert_int_equal(vmic_tunnel_lead_ns(NULL, 48000), 0);
	assert_int_equal(vmic_tunnel_lead_ns(pipewire, 0), 0);
}

/* A frame also waits out the audio's extra way through the sound server. */
static void video_waits_for_the_tunnel(void **state)
{
	(void)state;
	int dev[2], mic[2];
	struct virtualcam_data *vcam = sync_vcam(dev, mic);
	vcam->mic_lead_ns = MS(100);

	send_frame(vcam, MS(1000));
	send_audio(vcam, MS(1000), 1024);
	send_audio(vcam, 1021333333ULL, 1024);
	send_audio(vcam, 1042666667ULL, 1024);
	send_audio(vcam, 1064000000ULL, 1024); /* up to 1085.3 */
	assert_int_equal(queued_bytes(dev[0]), 0);
	send_audio(vcam, 1085333333ULL, 1024); /* up to 1106.7 */
	assert_int_equal(queued_bytes(dev[0]), SYNC_FRAME);

	sync_done(vcam, dev, mic);
}

/* #4119: the PipeWire node's reader drops audio only past the high mark, and
 * then down to one packet beyond what the cycle asks for. */
static void pipewire_reader_skips_only_stale_audio(void **state)
{
	(void)state;
	assert_int_equal(vmic_pw_skip(0, 256), 0);
	assert_int_equal(vmic_pw_skip(1024 + 256, 256), 0);
	assert_int_equal(vmic_pw_skip(3072 + 256, 256), 0);
	assert_int_equal(vmic_pw_skip(3073 + 256, 256), 3073 - 1024);
	assert_int_equal(vmic_pw_skip(32768, 1024), 32768 - 1024 - 1024);
}

/* A frame waits for its audio when the microphone is the PipeWire node too. */
static void video_waits_for_pipewire_audio(void **state)
{
	(void)state;
#ifdef HAVE_PIPEWIRE
	int dev[2], mic[2];
	struct virtualcam_data *vcam = sync_vcam(dev, mic);
	close(vcam->mic_fifo);
	vcam->mic_fifo = -1;
	struct vmic_pw *pw = bzalloc(sizeof(*pw));
	spa_ringbuffer_init(&pw->ring);
	vcam->mic_pw = pw;

	send_frame(vcam, MS(1000));
	assert_int_equal(queued_bytes(dev[0]), 0);
	send_audio(vcam, MS(1000), 1024);
	assert_int_equal(queued_bytes(dev[0]), SYNC_FRAME);
	uint32_t idx;
	assert_int_equal(spa_ringbuffer_get_read_index(&pw->ring, &idx), 1024 * 2 * (int)sizeof(float));

	vcam->mic_pw = NULL;
	bfree(pw);
	sync_done(vcam, dev, mic);
#else
	skip();
#endif
}

#ifdef HAVE_PIPEWIRE
struct pw_capture {
	struct pw_stream *stream;
	struct spa_hook listener;
	volatile float peak;
};

static void pw_capture_process(void *arg)
{
	struct pw_capture *cap = arg;
	struct pw_buffer *b = pw_stream_dequeue_buffer(cap->stream);
	if (!b)
		return;
	struct spa_data *d = &b->buffer->datas[0];
	if (d->data && d->chunk) {
		const float *f = (const float *)((const uint8_t *)d->data + d->chunk->offset);
		for (uint32_t i = 0; i < d->chunk->size / sizeof(float); i++)
			if (fabsf(f[i]) > cap->peak)
				cap->peak = fabsf(f[i]);
	}
	pw_stream_queue_buffer(cap->stream, b);
}

static const struct pw_stream_events pw_capture_events = {
	PW_VERSION_STREAM_EVENTS,
	.process = pw_capture_process,
};
#endif

/* Against the private PipeWire: the node is listed under its name, and a
 * tone written through virtual_audio reaches a capture stream linked to it. */
static void pipewire_node_carries_audio(void **state)
{
	(void)state;
#ifdef HAVE_PIPEWIRE
	if (!getenv("PULSE_SERVER"))
		skip();

	struct virtualcam_data *vcam = virtualcam_create(NULL, NULL);
	bool started = vmic_pw_start(vcam, "obs_vmic_pw_test", "OBS Audio", 48000);
	char *sources = started ? vmic_pa_list(false) : NULL;
	bool listed = sources && vmic_source_listed(sources, "obs_vmic_pw_test");
	bfree(sources);

	struct pw_capture cap = {0};
	struct pw_thread_loop *loop = pw_thread_loop_new("vmic-test-capture", NULL);
	pw_thread_loop_lock(loop);
	cap.stream = pw_stream_new_simple(pw_thread_loop_get_loop(loop), "vmic-test-capture",
					  pw_properties_new(PW_KEY_MEDIA_TYPE, "Audio", PW_KEY_MEDIA_CATEGORY,
							    "Capture", PW_KEY_TARGET_OBJECT, "obs_vmic_pw_test",
							    NULL),
					  &pw_capture_events, &cap);
	uint8_t buf[512];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buf, sizeof(buf));
	struct spa_audio_info_raw info = {.format = SPA_AUDIO_FORMAT_F32, .rate = 48000, .channels = 2};
	const struct spa_pod *params[1] = {spa_format_audio_raw_build(&b, SPA_PARAM_EnumFormat, &info)};
	pw_stream_connect(cap.stream, PW_DIRECTION_INPUT, PW_ID_ANY,
			  PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS | PW_STREAM_FLAG_RT_PROCESS, params, 1);
	pw_thread_loop_unlock(loop);
	pw_thread_loop_start(loop);

	static float tone[1024 * 2];
	for (size_t i = 0; i < 1024 * 2; i++)
		tone[i] = 0.5f;
	struct audio_data audio = {0};
	audio.data[0] = (uint8_t *)tone;
	audio.frames = 1024;
	for (int n = 0; started && n < 150 && cap.peak < 0.25f; n++) {
		audio.timestamp = MS(1000) + util_mul_div64((uint64_t)n * 1024, 1000000000ULL, 48000);
		virtual_audio(vcam, &audio);
		usleep(21333);
	}
	float peak = cap.peak;

	pw_thread_loop_stop(loop);
	pw_stream_destroy(cap.stream);
	pw_thread_loop_destroy(loop);
	virtualcam_destroy(vcam); /* stops the node */

	assert_true(started);
	assert_true(listed);
	assert_true(peak > 0.25f);
#else
	skip();
#endif
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(packet_that_fits_is_written_whole, setup_pipe, teardown_pipe),
		cmocka_unit_test_setup_teardown(packet_that_does_not_fit_is_dropped_whole, setup_pipe, teardown_pipe),
		cmocka_unit_test(short_write_never_leaves_half_a_packet),
		cmocka_unit_test(stale_is_decided_by_the_lock_not_the_pid),
		cmocka_unit_test(dialog_settings_choose_audio_and_track),
		cmocka_unit_test(a_second_obs_gets_its_own_source_name),
		cmocka_unit_test(hung_pactl_is_given_up_on),
		cmocka_unit_test(virtual_microphone_needs_no_pactl),
		cmocka_unit_test(stale_module_is_unloaded_with_the_microphone_off),
		cmocka_unit_test(pactl_that_cannot_run_is_told_apart),
		cmocka_unit_test(closed_fifo_is_not_written),
		cmocka_unit_test(module_index_is_parsed_from_pactl_reply),
		cmocka_unit_test(our_module_line_gives_id_and_pid),
		cmocka_unit_test(quoted_module_line_gives_id_and_pid),
		cmocka_unit_test(other_module_lines_are_ignored),
		cmocka_unit_test(video_waits_for_its_audio),
		cmocka_unit_test(video_is_not_held_without_microphone),
		cmocka_unit_test(held_video_is_bounded),
		cmocka_unit_test(stale_audio_is_dropped),
		cmocka_unit_test(pipewire_tunnel_lead_is_8192_frames),
		cmocka_unit_test(video_waits_for_the_tunnel),
		cmocka_unit_test(pipewire_reader_skips_only_stale_audio),
		cmocka_unit_test(video_waits_for_pipewire_audio),
		cmocka_unit_test(pipewire_node_carries_audio),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
