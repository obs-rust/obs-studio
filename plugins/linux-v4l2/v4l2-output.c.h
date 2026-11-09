#include "plugins/linux-v4l2/build/linux-v4l2-nix-output.pch.h"

/* The part of a packet a short write left unwritten. Pipe space counts in
 * page-sized slots, so a non-blocking write that fits by FIONREAD can still
 * stop part way; the rest goes out before anything else, or every later
 * sample would be shifted. */
struct vmic_tail {
	uint8_t *data;
	size_t len;
	size_t cap;
};

/* Frame sync (obs-rust/obs-studio#4087): libobs hands the output each video
 * frame as it renders but its audio only after the audio buffering, so with
 * the microphone on, frames wait here until the audio with the same timestamp
 * has gone to the microphone. Bounded, so the picture never stops. */
#define VCAM_SYNC_FRAMES 128
struct vcam_sync {
	pthread_mutex_t mutex;
	uint8_t *frame[VCAM_SYNC_FRAMES];
	uint64_t ts[VCAM_SYNC_FRAMES];
	size_t head;
	size_t len;
	/* End timestamp of the audio last handed to the microphone. */
	uint64_t audio_end;
};

/* The OBS Audio node when PipeWire runs (#4119); defined below. */
struct vmic_pw;

struct virtualcam_data {
	obs_output_t *output;
	int device;
	uint32_t frame_size;
	bool use_caps_workaround;

	/* OBS Virtual Microphone (#3011): the FIFO of a module-pipe-source. */
	int mic_fifo;
	/* A read end of the same FIFO, to throw away audio nobody read. */
	int mic_drain;
	uint32_t mic_rate;
	/* How much longer than the video the audio takes past the FIFO. */
	uint64_t mic_lead_ns;
	struct vmic_pw *mic_pw;
	struct vcam_sync sync;
	size_t mic_pipe_size;
	struct vmic_tail mic_tail;
	uint32_t mic_module;
	bool mic_loaded;
	/* Settings from the Virtual Camera dialog (#3182). */
	bool audio_enabled;
	size_t audio_mixer;
	char mic_path[256];
	char mic_lock_path[256];
	int mic_lock;
};

static void vmic_stop(struct virtualcam_data *vcam);

/* OBS Virtual Microphone (obs-rust/obs-studio#3011): while the Virtual Camera
 * runs, OBS's audio mix goes to a PulseAudio/PipeWire module-pipe-source, so
 * apps get picture and sound from one output with the same timestamps. */
#define VMIC_SOURCE_NAME "obs_virtual_microphone"
#define VMIC_CHANNELS 2
/* Two libobs audio packets (1024 float stereo frames = 8 KiB each), about
 * 42 ms at 48 kHz: small enough to stay in sync with the video. */
#define VMIC_PIPE_SIZE 16384
/* pactl runs on the thread that starts and stops the Virtual Camera: a sound
 * server that never answers costs at most this long (#3181). */
#define VMIC_PACTL_TIMEOUT "timeout -k 1 3 "

/* pactl load-module prints the new module's index on its own line. */
static bool vmic_parse_module_id(const char *text, uint32_t *id)
{
	if (!text || !*text)
		return false;

	char *end = NULL;
	errno = 0;
	unsigned long long value = strtoull(text, &end, 10);
	if (end == text || errno != 0 || value > UINT32_MAX)
		return false;
	while (*end == '\n' || *end == '\r' || *end == ' ')
		end++;
	if (*end)
		return false;

	*id = (uint32_t)value;
	return true;
}

#define VMIC_FIFO_NAME "obs-virtual-microphone-"
#define VMIC_DIR_NAME "obs-virtual-microphone"

/* A line of "pactl list modules short" ("<index>\t<name>\t<arguments>\t")
 * for a module-pipe-source this plugin loaded: its FIFO is named after the
 * pid of the OBS that loaded it (obs-rust/obs-studio#3014). */
static bool vmic_parse_module_line(const char *line, uint32_t *id, long *pid)
{
	if (!line)
		return false;

	const char *name = strchr(line, '\t');
	if (!name || strncmp(name + 1, "module-pipe-source\t", 19) != 0)
		return false;

	char *end = NULL;
	errno = 0;
	unsigned long long index = strtoull(line, &end, 10);
	if (end != name || errno != 0 || index > UINT32_MAX)
		return false;

	const char *fifo = strstr(name, "/" VMIC_FIFO_NAME);
	if (!fifo)
		return false;
	fifo += 1 + strlen(VMIC_FIFO_NAME);

	errno = 0;
	long value = strtol(fifo, &end, 10);
	/* A module loaded through libpulse keeps its quotes (file='...', #4057). */
	if (end == fifo || errno != 0 || value <= 0 ||
	    (*end != ' ' && *end != '\t' && *end != '\0' && *end != '\'' && *end != '"'))
		return false;

	*id = (uint32_t)index;
	*pid = value;
	return true;
}

/* #3180 writer start */
static void vmic_tail_free(struct vmic_tail *tail)
{
	bfree(tail->data);
	memset(tail, 0, sizeof(*tail));
}

/* Writes what it can of data; returns how many bytes went out, or -1 on an
 * error other than a full pipe. */
static ssize_t vmic_write_some(int fd, const uint8_t *data, size_t size)
{
	size_t done = 0;
	while (done < size) {
		ssize_t written = write(fd, data + done, size - done);
		if (written < 0 && errno == EINTR)
			continue;
		if (written < 0 && (errno == EAGAIN || errno == EWOULDBLOCK))
			break;
		if (written <= 0)
			return done ? (ssize_t)done : -1;
		done += (size_t)written;
	}
	return (ssize_t)done;
}

/* Whether the OBS that loaded virtual-microphone FIFO number n in dir is
 * gone (obs-rust/obs-studio#3179). */
static bool vmic_instance_is_stale(const char *dir, long n)
{
	char lock[512];
	snprintf(lock, sizeof(lock), "%s/" VMIC_FIFO_NAME "%ld.lock", dir, n);
	int fd = open(lock, O_RDWR | O_CLOEXEC);
	if (fd < 0) {
		/* Loaded by a build before the lock: only the pid tells. */
		return kill((pid_t)n, 0) != 0 && errno == ESRCH;
	}
	/* The running OBS holds this lock for as long as its module is loaded. */
	bool unlocked = flock(fd, LOCK_EX | LOCK_NB) == 0;
	close(fd);
	return unlocked;
}

/* Reads the Virtual Microphone settings: audio_enabled, and audio_track
 * 1..6 (anything else is track 1). */
static void vmic_read_settings(struct virtualcam_data *vcam, obs_data_t *settings)
{
	vcam->audio_enabled = !obs_data_has_user_value(settings, "audio_enabled") ||
			      obs_data_get_bool(settings, "audio_enabled");
	long long track = obs_data_get_int(settings, "audio_track");
	vcam->audio_mixer = track >= 1 && track <= MAX_AUDIO_MIXES ? (size_t)(track - 1) : 0;
}

/* Picks the source name and number for a new virtual microphone from
 * "pactl list sources short": the plain name while it is free, so apps keep
 * remembering it, else the first free "_<k>" (#3185). Returns k (1 for the
 * plain name), or 0 when none of the first 9 is free. */
static bool vmic_source_listed(const char *sources, const char *name)
{
	size_t len = strlen(name);
	for (const char *line = sources; line && *line;) {
		const char *field = strchr(line, '\t');
		const char *next = strchr(line, '\n');
		if (field && (!next || field < next) && strncmp(field + 1, name, len) == 0 &&
		    (field[1 + len] == '\t' || field[1 + len] == '\n' || field[1 + len] == '\0'))
			return true;
		line = next ? next + 1 : NULL;
	}
	return false;
}

static int vmic_pick_source_name(const char *sources, char *name, size_t size)
{
	for (int k = 1; k <= 9; k++) {
		if (k == 1)
			snprintf(name, size, "%s", VMIC_SOURCE_NAME);
		else
			snprintf(name, size, "%s_%d", VMIC_SOURCE_NAME, k);
		if (!vmic_source_listed(sources, name))
			return k;
	}
	return 0;
}

/* Writes one audio packet to the non-blocking FIFO, or drops it whole when it
 * does not fit: blocking would hold up OBS's output thread. A packet that was
 * started is always finished first, so the reader only ever sees whole
 * packets. */
static bool vmic_write_packet(int fd, size_t pipe_size, struct vmic_tail *tail, const uint8_t *data, size_t size)
{
	if (fd < 0 || size == 0)
		return false;

	if (tail->len > 0) {
		ssize_t done = vmic_write_some(fd, tail->data, tail->len);
		if (done < 0)
			return false;
		memmove(tail->data, tail->data + done, tail->len - (size_t)done);
		tail->len -= (size_t)done;
		if (tail->len > 0)
			return false;
	}

	int queued = 0;
	if (ioctl(fd, FIONREAD, &queued) < 0 || queued < 0)
		return false;
	if ((size_t)queued + size > pipe_size)
		return false;

	ssize_t done = vmic_write_some(fd, data, size);
	if (done <= 0)
		return false;
	size_t rest = size - (size_t)done;
	if (rest > 0) {
		if (tail->cap < rest) {
			tail->data = brealloc(tail->data, rest);
			tail->cap = rest;
		}
		memcpy(tail->data, data + done, rest);
		tail->len = rest;
	}
	return true;
}
/* Throws away audio that waits in the FIFO beyond keep bytes: while no app
 * reads the microphone the FIFO fills with old audio, and an app that links
 * later would hear it first and stay that far behind (#4087). Returns the
 * bytes dropped. */
static size_t vmic_drop_stale(int drain_fd, int fd, size_t keep)
{
	if (drain_fd < 0 || fd < 0)
		return 0;
	int queued = 0;
	if (ioctl(fd, FIONREAD, &queued) < 0 || queued <= 0 || (size_t)queued <= keep)
		return 0;
	/* Whole sample frames only, so what stays starts on a frame. */
	size_t want = ((size_t)queued - keep) & ~(size_t)(VMIC_CHANNELS * sizeof(float) - 1);
	uint8_t buf[4096];
	size_t dropped = 0;
	while (dropped < want) {
		size_t n = want - dropped < sizeof(buf) ? want - dropped : sizeof(buf);
		ssize_t got = read(drain_fd, buf, n);
		if (got < 0 && errno == EINTR)
			continue;
		if (got <= 0)
			break;
		dropped += (size_t)got;
	}
	return dropped;
}

/* PipeWire's pipe tunnel (what pipewire-pulse loads for module-pipe-source)
 * keeps 8192 frames buffered and no argument changes that (module-pipe-tunnel.c,
 * target_buffer), so its audio comes out that much after the FIFO; PulseAudio's
 * own module reads the FIFO directly. sources is "pactl list sources short",
 * whose third column is the driver (#4087). */
#define VMIC_PIPEWIRE_TUNNEL_FRAMES 8192
static uint64_t vmic_tunnel_lead_ns(const char *sources, uint32_t rate)
{
	if (!sources || rate == 0)
		return 0;
	for (const char *line = sources; *line;) {
		const char *end = strchr(line, '\n');
		size_t len = end ? (size_t)(end - line) : strlen(line);
		const char *tab1 = memchr(line, '\t', len);
		const char *tab2 = tab1 ? memchr(tab1 + 1, '\t', len - (size_t)(tab1 + 1 - line)) : NULL;
		if (tab2) {
			const char *driver = tab2 + 1;
			const char *tab3 = memchr(driver, '\t', len - (size_t)(driver - line));
			size_t dlen = tab3 ? (size_t)(tab3 - driver) : len - (size_t)(driver - line);
			if (dlen == strlen("PipeWire") && strncmp(driver, "PipeWire", dlen) == 0)
				return util_mul_div64(VMIC_PIPEWIRE_TUNNEL_FRAMES, 1000000000ULL, rate);
		}
		if (!end)
			break;
		line = end + 1;
	}
	return 0;
}

/* The PipeWire node's reader keeps at most this much audio beyond what a
 * cycle asks for, and drops older audio only past the high mark, so libobs's
 * 1024-frame packets never trip it in steady state (#4119). Returns the
 * frames to skip. */
#define VMIC_PW_KEEP_FRAMES 1024
#define VMIC_PW_HIGH_FRAMES 3072
static uint32_t vmic_pw_skip(uint32_t have, uint32_t want)
{
	if (have <= want || have - want <= VMIC_PW_HIGH_FRAMES)
		return 0;
	return have - want - VMIC_PW_KEEP_FRAMES;
}

/* Frees the frames waiting for their audio; under sync.mutex. */
static void vcam_sync_free(struct vcam_sync *sync)
{
	for (size_t i = 0; i < VCAM_SYNC_FRAMES; i++) {
		bfree(sync->frame[i]);
		sync->frame[i] = NULL;
	}
	sync->head = 0;
	sync->len = 0;
	sync->audio_end = 0;
}
/* #3180 writer end */

static const char *virtualcam_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "Virtual Camera Output";
}

static void virtualcam_destroy(void *data)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;

	if (vcam->device >= 0)
		close(vcam->device);
	vmic_stop(vcam);
	pthread_mutex_destroy(&vcam->sync.mutex);

	bfree(data);
}

#if defined(__linux__)
static bool is_flatpak_sandbox(void)
{
	static bool flatpak_info_exists = false;
	static bool initialized = false;

	if (!initialized) {
		flatpak_info_exists = access("/.flatpak-info", F_OK) == 0;
		initialized = true;
	}

	return flatpak_info_exists;
}

static int run_command(const char *command)
{
	struct dstr str;
	int result;

	dstr_init_copy(&str, "PATH=\"$PATH:/sbin\" ");

	if (is_flatpak_sandbox())
		dstr_cat(&str, "flatpak-spawn --host ");

	dstr_cat(&str, command);
	result = system(str.array);
	dstr_free(&str);
	return result;
}

/* Runs a pactl command (on the host when sandboxed) and returns its first
 * line of output, or NULL. */
static char *run_pactl(const char *args)
{
	struct dstr cmd;
	dstr_init_copy(&cmd, is_flatpak_sandbox() ? VMIC_PACTL_TIMEOUT "flatpak-spawn --host pactl "
						  : VMIC_PACTL_TIMEOUT "pactl ");
	dstr_cat(&cmd, args);
	dstr_cat(&cmd, " 2>/dev/null");

	FILE *fp = popen(cmd.array, "r");
	dstr_free(&cmd);
	if (!fp)
		return NULL;

	char line[128] = {0};
	char *out = fgets(line, sizeof(line), fp) ? bstrdup(line) : NULL;
	if (pclose(fp) != 0) {
		bfree(out);
		return NULL;
	}
	return out;
}

/* The sound server through libpulse, so no external pactl is needed (#4057).
 * Each returns false/NULL when libpulse cannot do it; callers then fall back
 * to pactl. Every call is bounded like pactl's timeout (#3181). */
#ifdef HAVE_LIBPULSE
#define VMIC_PA_TIMEOUT_NS 3000000000ULL

struct vmic_pa {
	pa_mainloop *loop;
	pa_context *ctx;
	uint64_t deadline;
};

static void vmic_pa_close(struct vmic_pa *pa)
{
	if (pa->ctx) {
		pa_context_disconnect(pa->ctx);
		pa_context_unref(pa->ctx);
	}
	if (pa->loop)
		pa_mainloop_free(pa->loop);
	pa->ctx = NULL;
	pa->loop = NULL;
}

/* Runs the loop until done(arg) or the deadline; false on the deadline or a loop error. */
static bool vmic_pa_run(struct vmic_pa *pa, bool (*done)(void *), void *arg)
{
	while (!done(arg)) {
		if (os_gettime_ns() > pa->deadline)
			return false;
		if (pa_mainloop_prepare(pa->loop, 50000) < 0 || pa_mainloop_poll(pa->loop) < 0 ||
		    pa_mainloop_dispatch(pa->loop) < 0)
			return false;
	}
	return true;
}

static bool vmic_pa_settled(void *arg)
{
	pa_context_state_t state = pa_context_get_state((pa_context *)arg);
	return state == PA_CONTEXT_READY || !PA_CONTEXT_IS_GOOD(state);
}

static bool vmic_pa_open(struct vmic_pa *pa)
{
	memset(pa, 0, sizeof(*pa));
	pa->deadline = os_gettime_ns() + VMIC_PA_TIMEOUT_NS;
	pa->loop = pa_mainloop_new();
	if (!pa->loop)
		return false;
	pa->ctx = pa_context_new(pa_mainloop_get_api(pa->loop), "OBS Virtual Microphone");
	if (!pa->ctx || pa_context_connect(pa->ctx, NULL, PA_CONTEXT_NOAUTOSPAWN, NULL) < 0 ||
	    !vmic_pa_run(pa, vmic_pa_settled, pa->ctx) || pa_context_get_state(pa->ctx) != PA_CONTEXT_READY) {
		vmic_pa_close(pa);
		return false;
	}
	return true;
}

static bool vmic_pa_op_done(void *arg)
{
	return pa_operation_get_state((pa_operation *)arg) != PA_OPERATION_RUNNING;
}

/* Waits for op and drops it; false when it did not finish in time. */
static bool vmic_pa_wait(struct vmic_pa *pa, pa_operation *op)
{
	if (!op)
		return false;
	bool finished = vmic_pa_run(pa, vmic_pa_op_done, op);
	if (!finished)
		pa_operation_cancel(op);
	pa_operation_unref(op);
	return finished;
}

static void vmic_pa_index_cb(pa_context *ctx, uint32_t index, void *userdata)
{
	UNUSED_PARAMETER(ctx);
	*(uint32_t *)userdata = index;
}

static bool vmic_pa_load(const char *args, uint32_t *id)
{
	struct vmic_pa pa;
	if (!vmic_pa_open(&pa))
		return false;
	uint32_t index = PA_INVALID_INDEX;
	bool ok = vmic_pa_wait(&pa,
			       pa_context_load_module(pa.ctx, "module-pipe-source", args, vmic_pa_index_cb, &index)) &&
		  index != PA_INVALID_INDEX;
	vmic_pa_close(&pa);
	if (ok)
		*id = index;
	return ok;
}

static void vmic_pa_success_cb(pa_context *ctx, int success, void *userdata)
{
	UNUSED_PARAMETER(ctx);
	*(bool *)userdata = success != 0;
}

static bool vmic_pa_unload(uint32_t id)
{
	struct vmic_pa pa;
	if (!vmic_pa_open(&pa))
		return false;
	bool success = false;
	bool ok = vmic_pa_wait(&pa, pa_context_unload_module(pa.ctx, id, vmic_pa_success_cb, &success)) && success;
	vmic_pa_close(&pa);
	return ok;
}

struct vmic_pa_list_data {
	struct dstr text;
	bool failed;
};

static void vmic_pa_module_cb(pa_context *ctx, const pa_module_info *info, int eol, void *userdata)
{
	UNUSED_PARAMETER(ctx);
	struct vmic_pa_list_data *data = userdata;
	if (eol < 0)
		data->failed = true;
	if (eol || !info)
		return;
	dstr_catf(&data->text, "%u\t%s\t%s\t\n", info->index, info->name ? info->name : "",
		  info->argument ? info->argument : "");
}

static void vmic_pa_source_cb(pa_context *ctx, const pa_source_info *info, int eol, void *userdata)
{
	UNUSED_PARAMETER(ctx);
	struct vmic_pa_list_data *data = userdata;
	if (eol < 0)
		data->failed = true;
	if (eol || !info)
		return;
	/* Same columns as "pactl list sources short": the driver tells PipeWire apart (#4087). */
	dstr_catf(&data->text, "%u\t%s\t%s\t\n", info->index, info->name ? info->name : "",
		  info->driver ? info->driver : "");
}

/* "pactl list modules short" (modules) or "pactl list sources short" text:
 * the index and name, tab separated, and for a module its arguments. */
static char *vmic_pa_list(bool modules)
{
	struct vmic_pa pa;
	if (!vmic_pa_open(&pa))
		return NULL;
	struct vmic_pa_list_data data;
	memset(&data, 0, sizeof(data));
	dstr_init(&data.text);
	pa_operation *op = modules ? pa_context_get_module_info_list(pa.ctx, vmic_pa_module_cb, &data)
				   : pa_context_get_source_info_list(pa.ctx, vmic_pa_source_cb, &data);
	bool ok = vmic_pa_wait(&pa, op) && !data.failed;
	vmic_pa_close(&pa);
	if (!ok) {
		dstr_free(&data.text);
		return NULL;
	}
	/* An empty list is still an answer, unlike NULL. */
	if (!data.text.array)
		dstr_copy(&data.text, "");
	return data.text.array;
}
#else
static bool vmic_pa_load(const char *args, uint32_t *id)
{
	UNUSED_PARAMETER(args);
	UNUSED_PARAMETER(id);
	return false;
}

static bool vmic_pa_unload(uint32_t id)
{
	UNUSED_PARAMETER(id);
	return false;
}

static char *vmic_pa_list(bool modules)
{
	UNUSED_PARAMETER(modules);
	return NULL;
}
#endif

/* Whether pactl itself can run at all, whatever the sound server would say (#3797). */
#ifdef HAVE_PIPEWIRE
/* OBS Audio as a native PipeWire Audio/Source node (#4119). The output thread
 * writes libobs packets into the ring; PipeWire's process callback reads it.
 * One writer, one reader: spa_ringbuffer needs no lock. */
#define VMIC_PW_RING (1u << 18)
#define VMIC_PW_STRIDE ((uint32_t)(VMIC_CHANNELS * sizeof(float)))
struct vmic_pw {
	struct pw_thread_loop *loop;
	struct pw_stream *stream;
	struct spa_hook listener;
	struct spa_ringbuffer ring;
	uint8_t data[VMIC_PW_RING];
};

static void vmic_pw_process(void *arg)
{
	struct vmic_pw *pw = arg;
	struct pw_buffer *b = pw_stream_dequeue_buffer(pw->stream);
	if (!b)
		return;
	struct spa_data *d = &b->buffer->datas[0];
	if (!d->data) {
		pw_stream_queue_buffer(pw->stream, b);
		return;
	}
	uint32_t want = d->maxsize / VMIC_PW_STRIDE;
	if (b->requested && b->requested < want)
		want = (uint32_t)b->requested;

	uint32_t index;
	int32_t avail = spa_ringbuffer_get_read_index(&pw->ring, &index);
	uint32_t have = avail > 0 ? (uint32_t)avail / VMIC_PW_STRIDE : 0;
	/* Audio nobody read (no app linked) or a backlog that grew: start
	 * from the newest instead of staying behind. */
	uint32_t skip = vmic_pw_skip(have, want);
	index += skip * VMIC_PW_STRIDE;
	have -= skip;

	uint32_t n = have < want ? have : want;
	spa_ringbuffer_read_data(&pw->ring, pw->data, VMIC_PW_RING, index % VMIC_PW_RING, d->data, n * VMIC_PW_STRIDE);
	spa_ringbuffer_read_update(&pw->ring, (int32_t)(index + n * VMIC_PW_STRIDE));
	if (n < want)
		memset((uint8_t *)d->data + n * VMIC_PW_STRIDE, 0, (want - n) * VMIC_PW_STRIDE);
	d->chunk->offset = 0;
	d->chunk->stride = (int32_t)VMIC_PW_STRIDE;
	d->chunk->size = want * VMIC_PW_STRIDE;
	pw_stream_queue_buffer(pw->stream, b);
}

static void vmic_pw_state_changed(void *arg, enum pw_stream_state old, enum pw_stream_state state, const char *error)
{
	UNUSED_PARAMETER(old);
	UNUSED_PARAMETER(state);
	UNUSED_PARAMETER(error);
	struct vmic_pw *pw = arg;
	pw_thread_loop_signal(pw->loop, false);
}

static const struct pw_stream_events vmic_pw_events = {
	PW_VERSION_STREAM_EVENTS,
	.state_changed = vmic_pw_state_changed,
	.process = vmic_pw_process,
};

/* Fails (and the pipe source is used instead) when no PipeWire daemon
 * answers or the node cannot be made. */
static bool vmic_pw_start(struct virtualcam_data *vcam, const char *name, const char *description, uint32_t rate)
{
	if (rate == 0)
		return false;
	pw_init(NULL, NULL);
	struct vmic_pw *pw = bzalloc(sizeof(*pw));
	spa_ringbuffer_init(&pw->ring);
	pw->loop = pw_thread_loop_new("obs-virtual-microphone", NULL);
	if (!pw->loop) {
		bfree(pw);
		return false;
	}

	char latency[32];
	snprintf(latency, sizeof(latency), "256/%u", rate);
	struct pw_properties *props = pw_properties_new(PW_KEY_MEDIA_CLASS, "Audio/Source", PW_KEY_NODE_NAME, name,
							PW_KEY_NODE_DESCRIPTION, description, PW_KEY_NODE_LATENCY,
							latency, PW_KEY_NODE_VIRTUAL, "true", PW_KEY_MEDIA_ROLE,
							"Communication", NULL);

	pw_thread_loop_lock(pw->loop);
	if (pw_thread_loop_start(pw->loop) < 0) {
		pw_thread_loop_unlock(pw->loop);
		pw_properties_free(props);
		pw_thread_loop_destroy(pw->loop);
		bfree(pw);
		return false;
	}
	pw->stream = pw_stream_new_simple(pw_thread_loop_get_loop(pw->loop), name, props, &vmic_pw_events, pw);
	bool ok = pw->stream != NULL;
	if (ok) {
		uint8_t buf[512];
		struct spa_pod_builder bld = SPA_POD_BUILDER_INIT(buf, sizeof(buf));
		struct spa_audio_info_raw info = {.format = SPA_AUDIO_FORMAT_F32,
						  .rate = rate,
						  .channels = VMIC_CHANNELS};
		info.position[0] = SPA_AUDIO_CHANNEL_FL;
		info.position[1] = SPA_AUDIO_CHANNEL_FR;
		const struct spa_pod *params[1] = {spa_format_audio_raw_build(&bld, SPA_PARAM_EnumFormat, &info)};
		/* No AUTOCONNECT: it is a device apps link to, not a stream to
		 * play on the speakers. */
		ok = pw_stream_connect(pw->stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
				       PW_STREAM_FLAG_MAP_BUFFERS | PW_STREAM_FLAG_RT_PROCESS, params, 1) == 0;
	}
	/* Wait until the daemon took the node, or gave up on it. */
	uint64_t deadline = os_gettime_ns() + 3000000000ULL;
	while (ok) {
		const char *error = NULL;
		enum pw_stream_state state = pw_stream_get_state(pw->stream, &error);
		if (state == PW_STREAM_STATE_ERROR || state == PW_STREAM_STATE_UNCONNECTED) {
			blog(LOG_INFO, "Virtual microphone: PipeWire node failed (%s)", error ? error : "unconnected");
			ok = false;
			break;
		}
		if (state == PW_STREAM_STATE_PAUSED || state == PW_STREAM_STATE_STREAMING)
			break;
		if (os_gettime_ns() > deadline) {
			ok = false;
			break;
		}
		pw_thread_loop_timed_wait(pw->loop, 1);
	}
	pw_thread_loop_unlock(pw->loop);
	if (!ok) {
		pw_thread_loop_stop(pw->loop);
		if (pw->stream)
			pw_stream_destroy(pw->stream);
		pw_thread_loop_destroy(pw->loop);
		bfree(pw);
		return false;
	}
	vcam->mic_pw = pw;
	vcam->mic_rate = rate;
	vcam->mic_lead_ns = 0;
	return true;
}

static void vmic_pw_stop(struct virtualcam_data *vcam)
{
	struct vmic_pw *pw = vcam->mic_pw;
	if (!pw)
		return;
	pthread_mutex_lock(&vcam->sync.mutex);
	vcam->mic_pw = NULL;
	pthread_mutex_unlock(&vcam->sync.mutex);
	pw_thread_loop_stop(pw->loop);
	pw_stream_destroy(pw->stream);
	pw_thread_loop_destroy(pw->loop);
	bfree(pw);
}

/* Drops the packet when the ring is full, which only happens while no app
 * reads the node; the reader then starts from the newest audio. */
static void vmic_pw_write(struct vmic_pw *pw, const uint8_t *data, size_t size)
{
	uint32_t index;
	int32_t filled = spa_ringbuffer_get_write_index(&pw->ring, &index);
	if (filled < 0 || (size_t)filled + size > VMIC_PW_RING)
		return;
	spa_ringbuffer_write_data(&pw->ring, pw->data, VMIC_PW_RING, index % VMIC_PW_RING, data, (uint32_t)size);
	spa_ringbuffer_write_update(&pw->ring, (int32_t)(index + (uint32_t)size));
}
#else
static bool vmic_pw_start(struct virtualcam_data *vcam, const char *name, const char *description, uint32_t rate)
{
	UNUSED_PARAMETER(vcam);
	UNUSED_PARAMETER(name);
	UNUSED_PARAMETER(description);
	UNUSED_PARAMETER(rate);
	return false;
}

static void vmic_pw_stop(struct virtualcam_data *vcam)
{
	UNUSED_PARAMETER(vcam);
}

static void vmic_pw_write(struct vmic_pw *pw, const uint8_t *data, size_t size)
{
	UNUSED_PARAMETER(pw);
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(size);
}
#endif

static bool vmic_pactl_runs(void)
{
	char *version = run_pactl("--version");
	bool runs = version != NULL;
	bfree(version);
	return runs;
}

static void vmic_stop(struct virtualcam_data *vcam)
{
	vmic_pw_stop(vcam);
	pthread_mutex_lock(&vcam->sync.mutex);
	if (vcam->mic_fifo >= 0) {
		close(vcam->mic_fifo);
		vcam->mic_fifo = -1;
	}
	vcam_sync_free(&vcam->sync);
	pthread_mutex_unlock(&vcam->sync.mutex);
	/* > 0: a zeroed struct (tests) must not close stdin. */
	if (vcam->mic_drain > 0)
		close(vcam->mic_drain);
	vcam->mic_drain = -1;
	vcam->mic_rate = 0;
	vcam->mic_lead_ns = 0;
	vmic_tail_free(&vcam->mic_tail);
	if (vcam->mic_loaded) {
		char args[64];
		if (!vmic_pa_unload(vcam->mic_module)) {
			snprintf(args, sizeof(args), "unload-module %u", vcam->mic_module);
			bfree(run_pactl(args));
		}
		vcam->mic_loaded = false;
	}
	if (vcam->mic_path[0]) {
		unlink(vcam->mic_path);
		vcam->mic_path[0] = 0;
	}
	if (vcam->mic_lock >= 0) {
		unlink(vcam->mic_lock_path);
		close(vcam->mic_lock);
		vcam->mic_lock = -1;
	}
}

/* Unloads the virtual microphones of OBS processes that are gone: one killed
 * while its Virtual Camera ran never unloaded its module, and apps would see
 * two "OBS Virtual Microphone"s. Another running OBS keeps its own. */
/* All of "pactl list modules short", through libpulse or else pactl; NULL
 * when neither answers. */
static char *vmic_list_modules(void)
{
	char *text = vmic_pa_list(true);
	if (text)
		return text;
	FILE *fp = popen(is_flatpak_sandbox() ? VMIC_PACTL_TIMEOUT
				 "flatpak-spawn --host pactl list modules short 2>/dev/null"
					      : VMIC_PACTL_TIMEOUT "pactl list modules short 2>/dev/null",
			 "r");
	if (!fp)
		return NULL;
	struct dstr out;
	dstr_init(&out);
	char line[4096];
	while (fgets(line, sizeof(line), fp))
		dstr_cat(&out, line);
	pclose(fp);
	return out.array;
}

static void vmic_unload_stale(const char *dir)
{
	char *modules = vmic_list_modules();
	if (!modules)
		return;

	uint32_t stale[16];
	size_t count = 0;
	for (char *line = modules, *next; *line; line = next) {
		next = strchr(line, '\n');
		if (next)
			*next++ = 0;
		else
			next = line + strlen(line);
		uint32_t id;
		long pid;
		if (!vmic_parse_module_line(line, &id, &pid) || !vmic_instance_is_stale(dir, pid))
			continue;
		if (count < sizeof(stale) / sizeof(stale[0]))
			stale[count++] = id;
		else
			blog(LOG_WARNING, "Virtual microphone: more than %zu stale modules; module %u is left loaded",
			     sizeof(stale) / sizeof(stale[0]), id);
	}
	bfree(modules);

	for (size_t i = 0; i < count; i++) {
		char args[64];
		if (!vmic_pa_unload(stale[i])) {
			snprintf(args, sizeof(args), "unload-module %u", stale[i]);
			bfree(run_pactl(args));
		}
		blog(LOG_INFO, "Virtual microphone: unloaded module %u left by an OBS that is no longer running",
		     stale[i]);
	}
}

/* Loads the module-pipe-source and opens its FIFO. On failure the Virtual
 * Camera runs video-only, as before. */
/* All of "pactl list sources short", or NULL. */
static char *vmic_list_sources(void)
{
	char *text = vmic_pa_list(false);
	if (text)
		return text;
	FILE *fp = popen(is_flatpak_sandbox() ? VMIC_PACTL_TIMEOUT
				 "flatpak-spawn --host pactl list sources short 2>/dev/null"
					      : VMIC_PACTL_TIMEOUT "pactl list sources short 2>/dev/null",
			 "r");
	if (!fp)
		return NULL;

	struct dstr out;
	dstr_init(&out);
	char line[1024];
	while (fgets(line, sizeof(line), fp))
		dstr_cat(&out, line);
	pclose(fp);
	return out.array;
}

static void vmic_start(struct virtualcam_data *vcam)
{
	if (!vcam->audio_enabled) {
		blog(LOG_INFO, "Virtual microphone turned off in the Virtual Camera settings");
		/* A dead OBS's silent device would stay listed in every app (#3797). */
		const char *off_runtime = getenv("XDG_RUNTIME_DIR");
		char off_dir[256];
		if (off_runtime && *off_runtime &&
		    snprintf(off_dir, sizeof(off_dir), "%s/" VMIC_DIR_NAME, off_runtime) < (int)sizeof(off_dir))
			vmic_unload_stale(off_dir);
		return;
	}

	/* Only the user's private runtime directory: a predictable name in a
	 * shared directory such as /tmp could be raced by another user. */
	const char *runtime = getenv("XDG_RUNTIME_DIR");
	if (!runtime || !*runtime) {
		blog(LOG_WARNING, "Virtual microphone unavailable (XDG_RUNTIME_DIR is not set); "
				  "the Virtual Camera sends video only");
		return;
	}
	/* The path goes into pactl's command line inside single quotes. */
	if (strchr(runtime, '\'')) {
		blog(LOG_WARNING, "Virtual microphone unavailable (XDG_RUNTIME_DIR contains a quote); "
				  "the Virtual Camera sends video only");
		return;
	}
	/* One directory that the host's sound server and a Flatpak sandbox both
	 * see (the manifest shares xdg-run/obs-virtual-microphone, #3179). */
	char dir[256];
	if (snprintf(dir, sizeof(dir), "%s/" VMIC_DIR_NAME, runtime) >= (int)sizeof(dir))
		return;
	if (mkdir(dir, 0700) != 0 && errno != EEXIST) {
		blog(LOG_WARNING, "Virtual microphone: cannot create '%s' (%s); the Virtual Camera sends video only",
		     dir, strerror(errno));
		return;
	}

	/* The number in the FIFO name starts at our pid, which in a sandbox
	 * another OBS can share, so the lock decides who owns it. */
	long n = (long)getpid();
	for (int tries = 0; tries < 64; tries++, n++) {
		if (snprintf(vcam->mic_lock_path, sizeof(vcam->mic_lock_path), "%s/" VMIC_FIFO_NAME "%ld.lock", dir,
			     n) >= (int)sizeof(vcam->mic_lock_path))
			break;
		int fd = open(vcam->mic_lock_path, O_RDWR | O_CREAT | O_CLOEXEC, 0600);
		if (fd < 0)
			break;
		if (flock(fd, LOCK_EX | LOCK_NB) == 0) {
			vcam->mic_lock = fd;
			break;
		}
		close(fd);
	}
	if (vcam->mic_lock < 0) {
		blog(LOG_WARNING, "Virtual microphone: no free FIFO name in '%s'; the Virtual Camera sends video only",
		     dir);
		return;
	}
	vmic_unload_stale(dir);

	if (snprintf(vcam->mic_path, sizeof(vcam->mic_path), "%s/" VMIC_FIFO_NAME "%ld", dir, n) >=
	    (int)sizeof(vcam->mic_path)) {
		vcam->mic_path[0] = 0;
		vmic_stop(vcam);
		return;
	}
	unlink(vcam->mic_path);

	/* After the stale cleanup, a listed name belongs to a running OBS. */
	char source[64];
	char *sources = vmic_list_sources();
	int k = vmic_pick_source_name(sources, source, sizeof(source));
	vcam->mic_lead_ns = vmic_tunnel_lead_ns(sources, audio_output_get_sample_rate(obs_get_audio()));
	bfree(sources);
	if (k == 0) {
		blog(LOG_WARNING, "Virtual microphone: nine are already in use; the Virtual Camera sends video only");
		vmic_stop(vcam);
		return;
	}
	char description[64];
	if (k == 1)
		snprintf(description, sizeof(description), "OBS Audio");
	else
		snprintf(description, sizeof(description), "OBS Audio (%d)", k);

	uint32_t rate = audio_output_get_sample_rate(obs_get_audio());
	/* A native node first: no FIFO, no tunnel buffer (#4119). */
	if (vmic_pw_start(vcam, source, description, rate)) {
		struct audio_convert_info pw_aci = {0};
		pw_aci.format = AUDIO_FORMAT_FLOAT;
		pw_aci.speakers = SPEAKERS_STEREO;
		pw_aci.samples_per_sec = rate;
		obs_output_set_audio_conversion(vcam->output, &pw_aci);
		obs_output_set_mixer(vcam->output, vcam->audio_mixer);
		blog(LOG_INFO, "Virtual microphone '%s' started (PipeWire node, %u Hz, track %zu)", source, rate,
		     vcam->audio_mixer + 1);
		return;
	}
	struct dstr args;
	dstr_init(&args);
	dstr_printf(&args,
		    "load-module module-pipe-source source_name=%s"
		    " file='%s' format=float32le rate=%u channels=%d"
		    /* The whole property list is quoted, or the server keeps only "OBS". */
		    " 'source_properties=\"device.description=\\\"%s\\\"\"'",
		    source, vcam->mic_path, rate, VMIC_CHANNELS, description);
	/* The same arguments as pactl gets once the shell has taken its quotes
	 * off: the server parses quoted values itself (#4057). */
	struct dstr pa_args;
	dstr_init(&pa_args);
	dstr_printf(&pa_args,
		    "source_name=%s file='%s' format=float32le rate=%u channels=%d"
		    " source_properties=\"device.description=\\\"%s\\\"\"",
		    source, vcam->mic_path, rate, VMIC_CHANNELS, description);
	vcam->mic_loaded = vmic_pa_load(pa_args.array, &vcam->mic_module);
	dstr_free(&pa_args);
	if (!vcam->mic_loaded) {
		char *reply = run_pactl(args.array);
		vcam->mic_loaded = vmic_parse_module_id(reply, &vcam->mic_module);
		bfree(reply);
	}
	dstr_free(&args);
	if (!vcam->mic_loaded) {
		if (vmic_pactl_runs())
			blog(LOG_WARNING,
			     "Virtual microphone unavailable (pactl load-module module-pipe-source failed); "
			     "the Virtual Camera sends video only");
		else
			blog(LOG_WARNING, "Virtual microphone unavailable (pactl cannot run: install PulseAudio's "
					  "client tools); the Virtual Camera sends video only");
		vcam->mic_path[0] = 0;
		return;
	}

	/* The module holds the FIFO open for reading, so this does not fail
	 * with ENXIO. */
	vcam->mic_fifo = open(vcam->mic_path, O_WRONLY | O_NONBLOCK | O_CLOEXEC);
	if (vcam->mic_fifo < 0) {
		blog(LOG_WARNING, "Virtual microphone: cannot open '%s' (%s)", vcam->mic_path, strerror(errno));
		vmic_stop(vcam);
		return;
	}

	int size = fcntl(vcam->mic_fifo, F_SETPIPE_SZ, VMIC_PIPE_SIZE);
	if (size < 0)
		size = fcntl(vcam->mic_fifo, F_GETPIPE_SZ);
	vcam->mic_pipe_size = size > 0 ? (size_t)size : VMIC_PIPE_SIZE;
	vcam->mic_rate = rate;
	/* Without it old audio is only not thrown away (#4087). */
	vcam->mic_drain = open(vcam->mic_path, O_RDONLY | O_NONBLOCK | O_CLOEXEC);

	struct audio_convert_info aci = {0};
	aci.format = AUDIO_FORMAT_FLOAT;
	aci.speakers = SPEAKERS_STEREO;
	aci.samples_per_sec = rate;
	obs_output_set_audio_conversion(vcam->output, &aci);
	/* Track 1 unless the Virtual Camera dialog picked another (#3182). */
	obs_output_set_mixer(vcam->output, vcam->audio_mixer);

	blog(LOG_INFO, "Virtual microphone '%s' started (module %u, %u Hz, track %zu)", source, vcam->mic_module, rate,
	     vcam->audio_mixer + 1);
}

static bool loopback_module_loaded(void)
{
	bool loaded = false;

	char temp[512];

	FILE *fp = fopen("/proc/modules", "r");

	if (!fp)
		return false;

	while (fgets(temp, sizeof(temp), fp)) {
		if (strstr(temp, "v4l2loopback")) {
			loaded = true;
			break;
		}
	}

	fclose(fp);

	return loaded;
}

bool loopback_module_available(void);
bool loopback_module_available(void)
{
	if (loopback_module_loaded()) {
		return true;
	}

	if (run_command("modinfo v4l2loopback >/dev/null 2>&1") == 0) {
		return true;
	}

	return false;
}

static int loopback_module_load(void)
{
	return run_command(
		"pkexec modprobe v4l2loopback exclusive_caps=1 card_label='OBS Virtual Camera' && sleep 0.5");
}
#else
bool loopback_module_available()
{
	struct v4l2_capability capability;
	char new_device[16];
	int fd;
	int i;

	for (i = 0; i != 32; i++) {
		snprintf(new_device, 16, "/dev/video%d", i);
		fd = open(new_device, O_RDWR | O_CLOEXEC);
		if (fd < 0)
			continue;
		if (ioctl(fd, VIDIOC_QUERYCAP, &capability) < 0) {
			close(fd);
			continue;
		}
		if (capability.capabilities & V4L2_CAP_VIDEO_OUTPUT) {
			close(fd);
			return true;
		}
		close(fd);
	}
	return false;
}

static void vmic_start(struct virtualcam_data *vcam)
{
	UNUSED_PARAMETER(vcam);
}

static void vmic_stop(struct virtualcam_data *vcam)
{
	UNUSED_PARAMETER(vcam);
}

static void vmic_pw_write(struct vmic_pw *pw, const uint8_t *data, size_t size)
{
	UNUSED_PARAMETER(pw);
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(size);
}
#endif

static void virtualcam_audio_proc(void *data, calldata_t *cd)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	calldata_set_bool(cd, "active", vcam->mic_fifo >= 0);
}

static void *virtualcam_create(obs_data_t *settings, obs_output_t *output)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)bzalloc(sizeof(*vcam));
	vcam->output = output;
	vcam->device = -1;
	vcam->mic_fifo = -1;
	vcam->mic_drain = -1;
	vcam->mic_lock = -1;
	pthread_mutex_init(&vcam->sync.mutex, NULL);
	vmic_read_settings(vcam, settings);

	/* The frontend asks after start whether the microphone runs, to tell
	 * the user when the camera fell back to video only (#3182). */
	if (output)
		proc_handler_add(obs_output_get_proc_handler(output), "void virtualcam_audio(out bool active)",
				 virtualcam_audio_proc, vcam);
	return vcam;
}

static void virtualcam_update(void *data, obs_data_t *settings)
{
	vmic_read_settings((struct virtualcam_data *)data, settings);
}

static bool try_connect(void *data, const char *device)
{
	static bool use_caps_workaround = false;
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	struct v4l2_capability capability;
	struct v4l2_format format;
	struct v4l2_streamparm parm;

	uint32_t width = obs_output_get_width(vcam->output);
	uint32_t height = obs_output_get_height(vcam->output);

	vcam->frame_size = width * height * 2;

	vcam->device = open(device, O_RDWR | O_CLOEXEC);

	if (vcam->device < 0)
		return false;

	if (ioctl(vcam->device, VIDIOC_QUERYCAP, &capability) < 0)
		goto fail_close_device;

	if (!(capability.device_caps & V4L2_CAP_VIDEO_OUTPUT))
		goto fail_close_device;

	format.type = V4L2_BUF_TYPE_VIDEO_OUTPUT;

	if (ioctl(vcam->device, VIDIOC_G_FMT, &format) < 0)
		goto fail_close_device;

	struct obs_video_info ovi;
	obs_get_video_info(&ovi);

	memset(&parm, 0, sizeof(parm));
	parm.type = V4L2_BUF_TYPE_VIDEO_OUTPUT;

	parm.parm.output.capability = V4L2_CAP_TIMEPERFRAME;
	parm.parm.output.timeperframe.numerator = ovi.fps_den;
	parm.parm.output.timeperframe.denominator = ovi.fps_num;

	if (ioctl(vcam->device, VIDIOC_S_PARM, &parm) < 0)
		goto fail_close_device;

	format.fmt.pix.width = width;
	format.fmt.pix.height = height;
	format.fmt.pix.pixelformat = V4L2_PIX_FMT_YUYV;
	format.fmt.pix.sizeimage = vcam->frame_size;

	if (ioctl(vcam->device, VIDIOC_S_FMT, &format) < 0)
		goto fail_close_device;

	struct vcam_format vf;
	if (!vcam_format_negotiate(width, height, &format.fmt.pix, &vf))
		goto fail_close_device;
	vcam->frame_size = vf.frame_size;

	if (vf.width != width || vf.height != height)
		blog(LOG_INFO, "Virtual camera device %s has a fixed size of %ux%u, scaling the %ux%u output to it",
		     device, vf.width, vf.height, width, height);

	if (!use_caps_workaround && ioctl(vcam->device, VIDIOC_STREAMON, &format.type) == 0) {
		/* STREAMON should error on all devices except v4l2loopback 0.12.x-0.13.x */
		use_caps_workaround = true;
	}
	vcam->use_caps_workaround = use_caps_workaround;

	struct video_scale_info vsi = {0};
	vsi.format = VIDEO_FORMAT_YUY2;
	vsi.width = vf.width;
	vsi.height = vf.height;
	obs_output_set_video_conversion(vcam->output, &vsi);

	memset(&parm, 0, sizeof(parm));
	parm.type = V4L2_BUF_TYPE_VIDEO_OUTPUT;

	if (vcam->use_caps_workaround && ioctl(vcam->device, VIDIOC_STREAMON, &format.type) < 0) {
		blog(LOG_ERROR, "Failed to start streaming on '%s' (%s)", device, strerror(errno));
		goto fail_close_device;
	}

	vmic_start(vcam);

	blog(LOG_INFO, "Virtual camera started");
	obs_output_begin_data_capture(vcam->output, 0);

	return true;

fail_close_device:
	close(vcam->device);
	vcam->device = -1;
	return false;
}

static int scanfilter(const struct dirent *entry)
{
	return !astrcmp_n(entry->d_name, "video", 5);
}

static bool virtualcam_start(void *data)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	struct dirent **list;
	bool success = false;
	int n;

#if defined(__linux__)
	if (!loopback_module_loaded()) {
		if (loopback_module_load() != 0)
			return false;
	}
#endif

	n = scandir("/dev", &list, scanfilter,
#if defined(__linux__)
		    versionsort
#else
		    alphasort
#endif
	);

	if (n == -1)
		return false;

	for (int i = 0; i < n; i++) {
		char device[32] = {0};

#if !defined(__linux__)
		if (strstr(list[i]->d_name, "video") != list[i]->d_name)
			continue;
#endif

		// Use the return value of snprintf to prevent truncation warning.
		int written = snprintf(device, 32, "/dev/%s", list[i]->d_name);
		if (written >= 32)
			blog(LOG_DEBUG, "v4l2-output: A format truncation may have occurred."
					" This can be ignored since it is quite improbable.");

		if (try_connect(vcam, device)) {
			success = true;
			break;
		}
	}

	while (n--)
		free(list[n]);
	free(list);

	if (!success)
		blog(LOG_WARNING, "Failed to start virtual camera");

	return success;
}

static void virtualcam_stop(void *data, uint64_t ts)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	obs_output_end_data_capture(vcam->output);

	uint32_t buf_type = V4L2_BUF_TYPE_VIDEO_OUTPUT;

	if (vcam->use_caps_workaround && ioctl(vcam->device, VIDIOC_STREAMOFF, &buf_type) < 0) {
		blog(LOG_WARNING, "Failed to stop streaming on video device %d (%s)", vcam->device, strerror(errno));
	}

	close(vcam->device);
	vcam->device = -1;
	vmic_stop(vcam);
	blog(LOG_INFO, "Virtual camera stopped");

	UNUSED_PARAMETER(ts);
}

static void vcam_write_frame(struct virtualcam_data *vcam, const uint8_t *frame_data)
{
	uint32_t frame_size = vcam->frame_size;
	while (frame_size > 0) {
		ssize_t written = write(vcam->device, frame_data, frame_size);
		if (written < 0 && errno == EINTR)
			continue;
		if (written <= 0)
			break;
		frame_data += written;
		frame_size -= (uint32_t)written;
	}
}

/* Sends the waiting frames whose audio the microphone has; under sync.mutex. */
static void vcam_sync_flush(struct virtualcam_data *vcam)
{
	struct vcam_sync *sync = &vcam->sync;
	while (sync->len > 0 && sync->ts[sync->head] + vcam->mic_lead_ns <= sync->audio_end) {
		vcam_write_frame(vcam, sync->frame[sync->head]);
		sync->head = (sync->head + 1) % VCAM_SYNC_FRAMES;
		sync->len--;
	}
}

static void virtual_video(void *param, struct video_data *frame)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)param;
	struct vcam_sync *sync = &vcam->sync;

	pthread_mutex_lock(&sync->mutex);
	/* No running microphone (mic_rate is set only by vmic_start): nothing to wait for. */
	if ((vcam->mic_fifo < 0 && !vcam->mic_pw) || vcam->mic_rate == 0 ||
	    (sync->len == 0 && frame->timestamp + vcam->mic_lead_ns <= sync->audio_end)) {
		vcam_write_frame(vcam, frame->data[0]);
		pthread_mutex_unlock(&sync->mutex);
		return;
	}
	if (sync->len == VCAM_SYNC_FRAMES) {
		/* The audio stopped: the picture goes on without it. */
		vcam_write_frame(vcam, sync->frame[sync->head]);
		sync->head = (sync->head + 1) % VCAM_SYNC_FRAMES;
		sync->len--;
	}
	size_t slot = (sync->head + sync->len) % VCAM_SYNC_FRAMES;
	if (!sync->frame[slot])
		sync->frame[slot] = bmalloc(vcam->frame_size);
	memcpy(sync->frame[slot], frame->data[0], vcam->frame_size);
	sync->ts[slot] = frame->timestamp;
	sync->len++;
	pthread_mutex_unlock(&sync->mutex);
}

static void virtual_audio(void *param, struct audio_data *frames)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)param;
	if ((vcam->mic_fifo < 0 && !vcam->mic_pw) || vcam->mic_rate == 0)
		return;
	size_t size = (size_t)frames->frames * VMIC_CHANNELS * sizeof(float);
	/* Keep at most one packet unread, so a reader hears the newest audio. */
	vmic_drop_stale(vcam->mic_drain, vcam->mic_fifo, size);
	vmic_write_packet(vcam->mic_fifo, vcam->mic_pipe_size, &vcam->mic_tail, frames->data[0], size);

	uint64_t end = frames->timestamp + util_mul_div64(frames->frames, 1000000000ULL, vcam->mic_rate);
	pthread_mutex_lock(&vcam->sync.mutex);
	/* Under the mutex: vmic_pw_stop clears mic_pw under it before freeing. */
	if (vcam->mic_pw)
		vmic_pw_write(vcam->mic_pw, frames->data[0], size);
	if (end > vcam->sync.audio_end)
		vcam->sync.audio_end = end;
	vcam_sync_flush(vcam);
	pthread_mutex_unlock(&vcam->sync.mutex);
}

struct obs_output_info virtualcam_info = {
	.id = "virtualcam_output",
	.flags = OBS_OUTPUT_AV,
	.get_name = virtualcam_name,
	.create = virtualcam_create,
	.update = virtualcam_update,
	.destroy = virtualcam_destroy,
	.start = virtualcam_start,
	.stop = virtualcam_stop,
	.raw_video = virtual_video,
	.raw_audio = virtual_audio,
};

#undef VMIC_SOURCE_NAME
#undef VMIC_CHANNELS
#undef VMIC_PIPE_SIZE
#undef VMIC_PACTL_TIMEOUT
#undef VMIC_FIFO_NAME
#undef VMIC_DIR_NAME
#undef VMIC_PA_TIMEOUT_NS
#undef VCAM_SYNC_FRAMES
#undef VMIC_PIPEWIRE_TUNNEL_FRAMES
#undef VMIC_PW_KEEP_FRAMES
#undef VMIC_PW_HIGH_FRAMES
#undef VMIC_PW_RING
#undef VMIC_PW_STRIDE
