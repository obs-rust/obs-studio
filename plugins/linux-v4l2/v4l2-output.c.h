#include "plugins/linux-v4l2/build/linux-v4l2-nix-output.pch.h"

struct virtualcam_data {
	obs_output_t *output;
	int device;
	uint32_t frame_size;
	bool use_caps_workaround;

	/* OBS Virtual Microphone (#3011): the FIFO of a module-pipe-source. */
	int mic_fifo;
	size_t mic_pipe_size;
	uint32_t mic_module;
	bool mic_loaded;
	char mic_path[256];
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

/* Writes one audio packet to the non-blocking FIFO, or drops it whole when it
 * does not fit: blocking would hold up OBS's output thread and a split packet
 * would leave the reader between two samples. */
static bool vmic_write_packet(int fd, size_t pipe_size, const uint8_t *data, size_t size)
{
	if (fd < 0 || size == 0)
		return false;

	int queued = 0;
	if (ioctl(fd, FIONREAD, &queued) < 0 || queued < 0)
		return false;
	if ((size_t)queued + size > pipe_size)
		return false;

	while (size > 0) {
		ssize_t written = write(fd, data, size);
		if (written < 0 && errno == EINTR)
			continue;
		if (written <= 0)
			return false;
		data += written;
		size -= (size_t)written;
	}
	return true;
}

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
	dstr_init_copy(&cmd, is_flatpak_sandbox() ? "flatpak-spawn --host pactl " : "pactl ");
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

static void vmic_stop(struct virtualcam_data *vcam)
{
	if (vcam->mic_fifo >= 0) {
		close(vcam->mic_fifo);
		vcam->mic_fifo = -1;
	}
	if (vcam->mic_loaded) {
		char args[64];
		snprintf(args, sizeof(args), "unload-module %u", vcam->mic_module);
		bfree(run_pactl(args));
		vcam->mic_loaded = false;
	}
	if (vcam->mic_path[0]) {
		unlink(vcam->mic_path);
		vcam->mic_path[0] = 0;
	}
}

/* Loads the module-pipe-source and opens its FIFO. On failure the Virtual
 * Camera runs video-only, as before. */
static void vmic_start(struct virtualcam_data *vcam)
{
	/* Only the user's private runtime directory: a predictable name in a
	 * shared directory such as /tmp could be raced by another user. */
	const char *runtime = getenv("XDG_RUNTIME_DIR");
	if (!runtime || !*runtime) {
		blog(LOG_WARNING, "Virtual microphone unavailable (XDG_RUNTIME_DIR is not set); "
				  "the Virtual Camera sends video only");
		return;
	}
	snprintf(vcam->mic_path, sizeof(vcam->mic_path), "%s/obs-virtual-microphone-%d", runtime, (int)getpid());
	unlink(vcam->mic_path);

	uint32_t rate = audio_output_get_sample_rate(obs_get_audio());
	struct dstr args;
	dstr_init(&args);
	dstr_printf(&args,
		    "load-module module-pipe-source source_name=" VMIC_SOURCE_NAME
		    " file='%s' format=float32le rate=%u channels=%d"
		    /* The whole property list is quoted, or the server keeps only "OBS". */
		    " 'source_properties=\"device.description=\\\"OBS Virtual Microphone\\\"\"'",
		    vcam->mic_path, rate, VMIC_CHANNELS);
	char *reply = run_pactl(args.array);
	dstr_free(&args);

	vcam->mic_loaded = vmic_parse_module_id(reply, &vcam->mic_module);
	bfree(reply);
	if (!vcam->mic_loaded) {
		blog(LOG_WARNING, "Virtual microphone unavailable (pactl load-module module-pipe-source failed); "
				  "the Virtual Camera sends video only");
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

	struct audio_convert_info aci = {0};
	aci.format = AUDIO_FORMAT_FLOAT;
	aci.speakers = SPEAKERS_STEREO;
	aci.samples_per_sec = rate;
	obs_output_set_audio_conversion(vcam->output, &aci);
	/* Track 1, the mix the stream and the default recording use. */
	obs_output_set_mixer(vcam->output, 0);

	blog(LOG_INFO, "Virtual microphone started (module %u, %u Hz)", vcam->mic_module, rate);
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
#endif

static void *virtualcam_create(obs_data_t *settings, obs_output_t *output)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)bzalloc(sizeof(*vcam));
	vcam->output = output;
	vcam->device = -1;
	vcam->mic_fifo = -1;

	UNUSED_PARAMETER(settings);
	return vcam;
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

static void virtual_video(void *param, struct video_data *frame)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)param;
	uint32_t frame_size = vcam->frame_size;
	const uint8_t *frame_data = frame->data[0];
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

static void virtual_audio(void *param, struct audio_data *frames)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)param;
	size_t size = (size_t)frames->frames * VMIC_CHANNELS * sizeof(float);
	vmic_write_packet(vcam->mic_fifo, vcam->mic_pipe_size, frames->data[0], size);
}

struct obs_output_info virtualcam_info = {
	.id = "virtualcam_output",
	.flags = OBS_OUTPUT_AV,
	.get_name = virtualcam_name,
	.create = virtualcam_create,
	.destroy = virtualcam_destroy,
	.start = virtualcam_start,
	.stop = virtualcam_stop,
	.raw_video = virtual_video,
	.raw_audio = virtual_audio,
};

#undef VMIC_SOURCE_NAME
#undef VMIC_CHANNELS
#undef VMIC_PIPE_SIZE
