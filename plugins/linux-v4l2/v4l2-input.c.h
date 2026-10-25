/*
Copyright (C) 2014 by Leonhard Oelke <leonhard@in-verted.de>

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 2 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/
#include "plugins/linux-v4l2/build/linux-v4l2-nix.pch.h"

#define FALLBACK_FRAMERATE 30

/* The new dv timing api was introduced in Linux 3.4
 * Currently we simply disable dv timings when this is not defined */
#if !defined(VIDIOC_ENUM_DV_TIMINGS) || !defined(V4L2_IN_CAP_DV_TIMINGS)
#define V4L2_IN_CAP_DV_TIMINGS 0
#endif

#define V4L2_DATA(voidptr) struct v4l2_data *data = voidptr;

#define timeval2ns(tv) (((uint64_t)tv.tv_sec * 1000000000) + ((uint64_t)tv.tv_usec * 1000))

#define V4L2_FOURCC_STR(code)                                                                \
	(char[5])                                                                            \
	{                                                                                    \
		code & 0xFF, (code >> 8) & 0xFF, (code >> 16) & 0xFF, (code >> 24) & 0xFF, 0 \
	}

#define blog(level, msg, ...) blog(level, "v4l2-input: " msg, ##__VA_ARGS__)

/**
 * Data structure for the v4l2 source
 */
struct v4l2_data {
	/* settings */
	char *device_id;
	int input;
	int pixfmt;
	int standard;
	int dv_timing;
	int64_t resolution;
	int64_t framerate;
	int color_range;

	/* internal data */
	obs_source_t *source;
	pthread_t thread;
	os_event_t *event;
	struct v4l2_decoder decoder;

	bool framerate_unchanged;
	bool resolution_unchanged;
	/* the framerate setting v4l2_init() mapped a saved value to, for
	 * v4l2_update() to store; 0 if none */
	int64_t framerate_setting;
	int_fast32_t dev;
	int width;
	int height;
	int linesize;
	struct v4l2_buffer_data buffers;

	bool auto_reset;
	int timeout_frames;

	/* the settings of the last update, for the saved camera controls */
	obs_data_t *controls;
	/* write the saved camera controls when the device opens */
	bool restore_controls;
	/* the device the saved controls were set on, and the open device
	 * (card and bus info); empty when not known */
	struct dstr controls_device;
	struct dstr dev_identity;
	/* the above are also used from the udev thread and on save */
	pthread_mutex_t controls_mutex;

	/* The camera the source captures (obs-rust/obs-studio#2262): the
	 * settings name it next to device_id, because the kernel numbers the
	 * nodes in the order the devices appear, so a replugged or rebooted
	 * camera can get another /dev/videoN. device_id is the node it was last
	 * found at. Unknown until the device first opens for settings from
	 * before. device_id, identity and match are also used from the udev
	 * thread. */
	pthread_mutex_t device_mutex;
	struct v4l2_identity identity;
	enum v4l2_identity_match match;
	/* the node and camera of the settings at the last update or sync, to
	 * tell a new choice from the node the source moved to (UI thread) */
	char *settings_device_id;
	struct v4l2_identity settings_identity;
	/* what was reported about the absent camera: 0 nothing (it was
	 * found), 1 that it is away, 2 that another camera has its node */
	int absent_reported;

	/* Another process has the device open (EBUSY, obs-rust/obs-studio#799):
	 * the source names it, offers to close it, and retries until the
	 * device is free. Also used from the graphics and udev threads. */
	pthread_mutex_t busy_mutex;
	bool busy;
	bool busy_holder_known;
	struct v4l2_holder busy_holder;
	uint32_t busy_attempt;
	/* next full retry, and next check whether the holder let go */
	uint64_t busy_retry_ns;
	uint64_t busy_check_ns;
	bool busy_task_queued;
};

/* forward declarations */
static void v4l2_init(struct v4l2_data *data, const char *hint);
static int v4l2_setup_device(struct v4l2_data *data);
static void v4l2_close_device(struct v4l2_data *data);
static int v4l2_open_saved_camera(struct v4l2_data *data);
static void v4l2_terminate(struct v4l2_data *data);
static void v4l2_update(void *vptr, obs_data_t *settings);

/**
 * Prepare the output frame structure for obs and compute plane offsets
 * For encoded formats (mjpeg) this clears the frame and plane offsets,
 * which will be filled in after decoding.
 *
 * Basically all data apart from memory pointers and the timestamp is known
 * before the capture starts. This function prepares the obs_source_frame
 * struct with all the data that is already known.
 *
 * v4l2 uses a continuous memory segment for all planes so we simply compute
 * offsets to add to the start address in order to give obs the correct data
 * pointers for the individual planes.
 *
 */
static void v4l2_prep_obs_frame(struct v4l2_data *data, struct obs_source_frame *frame, size_t *plane_offsets)
{
	memset(frame, 0, sizeof(struct obs_source_frame));
	memset(plane_offsets, 0, sizeof(size_t) * MAX_AV_PLANES);

	const enum video_format format = v4l2_to_obs_video_format(data->pixfmt);

	frame->flags = OBS_SOURCE_FRAME_LINEAR_ALPHA;
	frame->width = data->width;
	frame->height = data->height;
	frame->format = format;
	video_format_get_parameters_for_format(VIDEO_CS_DEFAULT, data->color_range, format, frame->color_matrix,
					       frame->color_range_min, frame->color_range_max);

	switch (data->pixfmt) {
	case V4L2_PIX_FMT_NV12:
		frame->linesize[0] = data->linesize;
		frame->linesize[1] = data->linesize;
		plane_offsets[1] = data->linesize * data->height;
		break;
	case V4L2_PIX_FMT_YVU420:
		frame->linesize[0] = data->linesize;
		frame->linesize[1] = data->linesize / 2;
		frame->linesize[2] = data->linesize / 2;
		plane_offsets[1] = data->linesize * data->height * 5 / 4;
		plane_offsets[2] = data->linesize * data->height;
		break;
	case V4L2_PIX_FMT_YUV420:
		frame->linesize[0] = data->linesize;
		frame->linesize[1] = data->linesize / 2;
		frame->linesize[2] = data->linesize / 2;
		plane_offsets[1] = data->linesize * data->height;
		plane_offsets[2] = data->linesize * data->height * 5 / 4;
		break;
	default:
		frame->linesize[0] = data->linesize;
		break;
	}
}

/*
 * Capture from the open device until the source stops it (returns true) or
 * the device fails (returns false). Sets *attempt to 0 once the device
 * delivers a buffer.
 */
static bool v4l2_capture(struct v4l2_data *data, uint32_t *attempt)
{
	int r;
	fd_set fds;
	uint8_t *start;
	uint64_t frames;
	uint64_t first_ts;
	bool have_first_ts;
	uint64_t dropped;
	bool stopped = false;
	struct timeval tv;
	struct v4l2_buffer buf;
	struct obs_source_frame out;
	size_t plane_offsets[MAX_AV_PLANES];
	int fps_num, fps_denom;
	float ffps;
	uint64_t timeout_usec;

	/* Get framerate and calculate appropriate select timeout value. */
	v4l2_unpack_tuple(&fps_num, &fps_denom, data->framerate);
	ffps = (float)fps_denom / fps_num;
	blog(LOG_DEBUG, "%s: framerate: %.2f fps", data->device_id, (double)ffps);
	/* Timeout set to 5 frame periods. */
	timeout_usec = (1000000 * data->timeout_frames) / ffps;
	blog(LOG_INFO, "%s: select timeout set to %" PRIu64 " (%dx frame periods)", data->device_id, timeout_usec,
	     data->timeout_frames);

	if (v4l2_start_capture(data->dev, &data->buffers) < 0)
		goto exit;

	blog(LOG_DEBUG, "%s: new capture started", data->device_id);

	frames = 0;
	first_ts = 0;
	have_first_ts = false;
	dropped = 0;
	v4l2_prep_obs_frame(data, &out, plane_offsets);

	blog(LOG_DEBUG, "%s: obs frame prepared", data->device_id);

	while (!(stopped = os_event_try(data->event) != EAGAIN)) {
		FD_ZERO(&fds);
		FD_SET(data->dev, &fds);

		/* Set timeout timevalue. */
		tv.tv_sec = 0;
		tv.tv_usec = timeout_usec;

		r = select(data->dev + 1, &fds, NULL, NULL, &tv);
		if (r < 0) {
			if (errno == EINTR)
				continue;
			blog(LOG_ERROR, "%s: select failed: %s", data->device_id, strerror(errno));
			break;
		} else if (r == 0) {
			blog(LOG_ERROR, "%s: select timed out", data->device_id);

#ifdef _DEBUG
			v4l2_query_all_buffers(data->dev, &data->buffers);
#endif

			if (v4l2_ioctl(data->dev, VIDIOC_LOG_STATUS) < 0) {
				blog(LOG_ERROR, "%s: failed to log status", data->device_id);
			}

			if (data->auto_reset) {
				if (v4l2_reset_capture(data->dev, &data->buffers) == 0)
					blog(LOG_INFO, "%s: stream reset successful", data->device_id);
				else
					blog(LOG_ERROR, "%s: failed to reset", data->device_id);
			}

			continue;
		}

		buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
		buf.memory = V4L2_MEMORY_MMAP;

		if (v4l2_ioctl(data->dev, VIDIOC_DQBUF, &buf) < 0) {
			if (errno == EAGAIN) {
				blog(LOG_DEBUG, "%s: ioctl dqbuf eagain", data->device_id);
				continue;
			}
			blog(LOG_ERROR, "%s: failed to dequeue buffer: %s", data->device_id, strerror(errno));
			break;
		}
		*attempt = 0;

		blog(LOG_DEBUG, "%s: ts: %06ld buf id #%d, flags 0x%08X, seq #%d, len %d, used %d", data->device_id,
		     buf.timestamp.tv_usec, buf.index, buf.flags, buf.sequence, buf.length, buf.bytesused);

		start = (uint8_t *)data->buffers.info[buf.index].start;

		switch (v4l2_frame_action(&out, data->pixfmt, &buf, start, plane_offsets, &data->decoder)) {
		case V4L2_FRAME_OUTPUT:
			if (dropped) {
				blog(LOG_INFO, "%s: decoding again after dropping %" PRIu64 " frames", data->device_id,
				     dropped);
				dropped = 0;
			}
			break;
		case V4L2_FRAME_SKIP:
			blog(LOG_DEBUG, "%s: no new picture in buffer (error flag, empty or decoder waiting)",
			     data->device_id);
			goto continue_queue_buffer;
		case V4L2_FRAME_DROP:
			if (!dropped++)
				blog(LOG_WARNING, "%s: dropping frames that cannot be decoded", data->device_id);
			goto continue_queue_buffer;
		}

		out.timestamp = timeval2ns(buf.timestamp);
		if (!have_first_ts) {
			first_ts = out.timestamp;
			have_first_ts = true;
		}
		out.timestamp -= first_ts;

		obs_source_output_video(data->source, &out);

	continue_queue_buffer:
		if (v4l2_ioctl(data->dev, VIDIOC_QBUF, &buf) < 0) {
			blog(LOG_ERROR, "%s: failed to enqueue buffer: %s", data->device_id, strerror(errno));
			break;
		}

		frames++;
	}

	blog(LOG_INFO, "%s: Stopped capture after %" PRIu64 " frames", data->device_id, frames);

exit:
	v4l2_stop_capture(data->dev);
	return stopped;
}

/*
 * The device failed while it stays registered: no udev "remove" comes for a
 * camera whose firmware hangs, a driver that fails with EIO or ENODEV after
 * resume, or a stream another process pulled (obs-rust/obs-studio#2264,
 * upstream obsproject/obs-studio#9434). Let go of the device, which some
 * drivers need before they recover, and open it again after a growing delay
 * until it captures or the source stops. A real unplug ends this through
 * device_removed(), and the replug starts the capture through device_added().
 *
 * Returns true with the device open again, false when the source stopped.
 */
static bool v4l2_reopen(struct v4l2_data *data, uint32_t *attempt)
{
	v4l2_close_device(data);
	if (*attempt == 0)
		blog(LOG_WARNING, "%s: the device failed, closing it and opening it again until it works",
		     data->device_id);

	for (;;) {
		/* Returns at once when the source stops (v4l2_terminate) */
		if (os_event_timedwait(data->event, v4l2_reopen_delay_ms(*attempt)) != ETIMEDOUT)
			return false;
		(*attempt)++;

		/* Only the source's own camera: another one may have taken
		 * the node meanwhile (obs-rust/obs-studio#2262) */
		pthread_mutex_lock(&data->device_mutex);
		bool known = v4l2_identity_known(&data->identity);
		pthread_mutex_unlock(&data->device_mutex);
		int dev = known ? v4l2_open_saved_camera(data)
				: v4l2_open(data->device_id, O_RDWR | O_NONBLOCK | O_CLOEXEC);
		const char *why = known ? "the camera is not at the node, or it cannot be opened" : strerror(errno);
		if (dev != -1) {
			pthread_mutex_lock(&data->controls_mutex);
			data->dev = dev;
			pthread_mutex_unlock(&data->controls_mutex);
			if (v4l2_setup_device(data) == 0) {
				blog(LOG_INFO, "%s: device reopened (attempt %" PRIu32 ")", data->device_id, *attempt);
				return true;
			}
			why = strerror(errno);
			v4l2_close_device(data);
		}
		blog(LOG_INFO, "%s: cannot reopen the device (attempt %" PRIu32 "): %s; next attempt in %" PRIu32 " s",
		     data->device_id, *attempt, why, v4l2_reopen_delay_ms(*attempt) / 1000);
	}
}

/*
 * Worker thread to get video data
 */
static void *v4l2_thread(void *vptr)
{
	V4L2_DATA(vptr);
	/* reopen attempts since the device last delivered a buffer */
	uint32_t attempt = 0;

	blog(LOG_DEBUG, "%s: new capture thread", data->device_id);
	os_set_thread_name("v4l2: capture");

	while (!v4l2_capture(data, &attempt)) {
		if (!v4l2_reopen(data, &attempt))
			break;
	}
	return NULL;
}

static const char *v4l2_getname(void *unused)
{
	UNUSED_PARAMETER(unused);
	return obs_module_text("V4L2Input");
}

static void v4l2_defaults(obs_data_t *settings)
{
	obs_data_set_default_int(settings, "input", -1);
	obs_data_set_default_int(settings, "pixelformat", -1);
	obs_data_set_default_int(settings, "standard", -1);
	obs_data_set_default_int(settings, "dv_timing", -1);
	obs_data_set_default_int(settings, "resolution", -1);
	obs_data_set_default_int(settings, "framerate", -1);
	obs_data_set_default_int(settings, "color_range", VIDEO_RANGE_DEFAULT);
	obs_data_set_default_bool(settings, "buffering", true);
	obs_data_set_default_bool(settings, "auto_reset", false);
	obs_data_set_default_int(settings, "timeout_frames", 5);
	obs_data_set_default_bool(settings, "restore_controls", true);
}

/*
 * The camera controls group: the option to restore them, then the device's
 * controls.
 */
static obs_properties_t *v4l2_controls_properties(void)
{
	obs_properties_t *ctrl_props = obs_properties_create();
	obs_property_t *restore =
		obs_properties_add_bool(ctrl_props, "restore_controls", obs_module_text("RestoreCameraCtrls"));
	obs_property_set_long_description(restore, obs_module_text("RestoreCameraCtrls.ToolTip"));
	return ctrl_props;
}

/**
 * Enable/Disable all properties for the source.
 *
 * @note A property that should be ignored can be specified
 *
 * @param props the source properties
 * @param ignore ignore this property
 * @param enable enable/disable all properties
 */
static void v4l2_props_set_enabled(obs_properties_t *props, obs_property_t *ignore, bool enable)
{
	if (!props)
		return;

	for (obs_property_t *prop = obs_properties_first(props); prop != NULL; obs_property_next(&prop)) {
		if (prop == ignore)
			continue;

		obs_property_set_enabled(prop, enable);
	}
}

/*
 * List available devices
 */
static void v4l2_device_list(obs_property_t *prop, obs_data_t *settings)
{
	DIR *dirp;
	struct dirent *dp;
	struct dstr device;
	bool cur_device_found;
	size_t cur_device_index;
	const char *cur_device_name;
	struct v4l2_identity cur_identity;

#ifdef __FreeBSD__
	dirp = opendir("/dev");
#else
	dirp = opendir(V4L2_SYSFS_ROOT "/class/video4linux");
#endif
	if (!dirp)
		return;

	cur_device_found = false;
	cur_device_name = obs_data_get_string(settings, "device_id");
	v4l2_identity_load(settings, &cur_identity);

	obs_property_list_clear(prop);

	dstr_init_copy(&device, V4L2_DEV_DIR "/");
	const size_t dir_len = device.len;

	while ((dp = readdir(dirp)) != NULL) {
		int fd;
		uint32_t caps;
		struct v4l2_capability video_cap;

#ifdef __FreeBSD__
		if (strstr(dp->d_name, "video") == NULL)
			continue;
#endif

		if (dp->d_type == DT_DIR)
			continue;

		dstr_resize(&device, dir_len);
		dstr_cat(&device, dp->d_name);

		if ((fd = v4l2_open(device.array, O_RDWR | O_NONBLOCK | O_CLOEXEC)) == -1) {
			blog(LOG_INFO, "Unable to open %s", device.array);
			continue;
		}

		if (v4l2_ioctl(fd, VIDIOC_QUERYCAP, &video_cap) == -1) {
			blog(LOG_INFO, "Failed to query capabilities for %s", device.array);
			v4l2_close(fd);
			continue;
		}

#ifndef V4L2_CAP_DEVICE_CAPS
		caps = video_cap.capabilities;
#else
		/* ... since Linux 3.3 */
		caps = (video_cap.capabilities & V4L2_CAP_DEVICE_CAPS) ? video_cap.device_caps : video_cap.capabilities;
#endif

		if (!(caps & V4L2_CAP_VIDEO_CAPTURE)) {
			blog(LOG_INFO, "%s seems to not support video capture", device.array);
			v4l2_close(fd);
			continue;
		}

		/* make sure device names are unique */
		char unique_device_name[68];
		int ret = snprintf(unique_device_name, sizeof(unique_device_name), "%s (%s)", video_cap.card,
				   video_cap.bus_info);
		if (ret >= (int)sizeof(unique_device_name))
			blog(LOG_DEBUG, "linux-v4l2: A format truncation may have occurred."
					" This can be ignored since it is quite improbable.");

		obs_property_list_add_string(prop, unique_device_name, device.array);
		blog(LOG_INFO, "Found device '%s' at %s", video_cap.card, device.array);

		/* check if this is the currently used device: the source's
		 * camera at its node, not another camera that took the number */
		if (cur_device_name && !strcmp(cur_device_name, device.array)) {
			struct v4l2_identity identity;
			cur_device_found = !v4l2_identity_known(&cur_identity) ||
					   (v4l2_identity_query(fd, device.array, &identity) &&
					    v4l2_identity_match(&cur_identity, &identity) != V4L2_MATCH_NONE);
		}

		v4l2_close(fd);
	}

	/* add currently selected device if not present, but disable it ... */
	if (!cur_device_found && cur_device_name && strlen(cur_device_name)) {
		if (v4l2_identity_known(&cur_identity)) {
			/* first, so it shows as selected rather than the other
			 * camera now at its node */
			struct dstr label = {0};
			v4l2_identity_label(&cur_identity, &label);
			obs_property_list_insert_string(prop, 0, label.array, cur_device_name);
			obs_property_list_item_disable(prop, 0, true);
			dstr_free(&label);
		} else {
			cur_device_index = obs_property_list_add_string(prop, cur_device_name, cur_device_name);
			obs_property_list_item_disable(prop, cur_device_index, true);
		}
	}

	closedir(dirp);
	dstr_free(&device);
}

/*
 * List inputs for device
 */
static void v4l2_input_list(int_fast32_t dev, obs_property_t *prop)
{
	struct v4l2_input in;
	memset(&in, 0, sizeof(in));

	obs_property_list_clear(prop);

	while (v4l2_ioctl(dev, VIDIOC_ENUMINPUT, &in) == 0) {
		obs_property_list_add_int(prop, (char *)in.name, in.index);
		blog(LOG_INFO, "Found input '%s' (Index %d)", in.name, in.index);
		in.index++;
	}
}

/*
 * List formats for device
 */
static void v4l2_format_list(int dev, obs_property_t *prop)
{
	struct v4l2_fmtdesc fmt;
	fmt.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	fmt.index = 0;
	struct dstr buffer;
	dstr_init(&buffer);

	obs_property_list_clear(prop);

	while (v4l2_ioctl(dev, VIDIOC_ENUM_FMT, &fmt) == 0) {
		dstr_copy(&buffer, (char *)fmt.description);
		if (fmt.flags & V4L2_FMT_FLAG_EMULATED)
			dstr_cat(&buffer, " (Emulated)");

		if (v4l2_to_obs_video_format(fmt.pixelformat) != VIDEO_FORMAT_NONE ||
		    fmt.pixelformat == V4L2_PIX_FMT_MJPEG || fmt.pixelformat == V4L2_PIX_FMT_H264) {
			obs_property_list_add_int(prop, buffer.array, fmt.pixelformat);
			blog(LOG_INFO, "Pixelformat: %s (available)", buffer.array);
		} else {
			blog(LOG_INFO, "Pixelformat: %s (unavailable)", buffer.array);
		}
		fmt.index++;
	}

	dstr_free(&buffer);
}

/*
 * List video standards for the device
 */
static void v4l2_standard_list(int dev, obs_property_t *prop)
{
	struct v4l2_standard std;
	std.index = 0;

	obs_property_list_clear(prop);

	obs_property_list_add_int(prop, obs_module_text("LeaveUnchanged"), -1);

	while (v4l2_ioctl(dev, VIDIOC_ENUMSTD, &std) == 0) {
		obs_property_list_add_int(prop, (char *)std.name, std.id);
		std.index++;
	}
}

/*
 * List dv timings for the device
 */
static void v4l2_dv_timing_list(int dev, obs_property_t *prop)
{
	struct v4l2_dv_timings dvt;
	struct dstr buf;
	int index = 0;
	dstr_init(&buf);

	obs_property_list_clear(prop);

	obs_property_list_add_int(prop, obs_module_text("LeaveUnchanged"), -1);

	while (v4l2_enum_dv_timing(dev, &dvt, index) == 0) {
		/* i do not pretend to understand, this is from qv4l2 ... */
		double h = (double)dvt.bt.height + dvt.bt.vfrontporch + dvt.bt.vsync + dvt.bt.vbackporch +
			   dvt.bt.il_vfrontporch + dvt.bt.il_vsync + dvt.bt.il_vbackporch;
		double w = (double)dvt.bt.width + dvt.bt.hfrontporch + dvt.bt.hsync + dvt.bt.hbackporch;
		double i = (dvt.bt.interlaced) ? 2.0f : 1.0f;
		double rate = (double)dvt.bt.pixelclock / (w * (h / i));

		dstr_printf(&buf, "%ux%u%c %.2f", dvt.bt.width, dvt.bt.height, (dvt.bt.interlaced) ? 'i' : 'p', rate);

		obs_property_list_add_int(prop, buf.array, index);

		index++;
	}

	dstr_free(&buf);
}

/*
 * List resolutions for device and format
 */
static void v4l2_resolution_list(int dev, uint_fast32_t pixelformat, obs_property_t *prop)
{
	struct v4l2_frmsizeenum frmsize;
	frmsize.pixel_format = pixelformat;
	frmsize.index = 0;
	struct dstr buffer;
	dstr_init(&buffer);

	obs_property_list_clear(prop);

	obs_property_list_add_int(prop, obs_module_text("LeaveUnchanged"), -1);

	v4l2_ioctl(dev, VIDIOC_ENUM_FRAMESIZES, &frmsize);

	switch (frmsize.type) {
	case V4L2_FRMSIZE_TYPE_DISCRETE:
		while (v4l2_ioctl(dev, VIDIOC_ENUM_FRAMESIZES, &frmsize) == 0) {
			dstr_printf(&buffer, "%dx%d", frmsize.discrete.width, frmsize.discrete.height);
			obs_property_list_add_int(prop, buffer.array,
						  v4l2_pack_tuple(frmsize.discrete.width, frmsize.discrete.height));
			frmsize.index++;
		}
		break;
	default:
		blog(LOG_INFO, "Stepwise and Continuous framesizes "
			       "are currently hardcoded");

		for (const int64_t *packed = v4l2_framesizes; *packed; ++packed) {
			int width;
			int height;
			v4l2_unpack_tuple(&width, &height, *packed);
			dstr_printf(&buffer, "%dx%d", width, height);
			obs_property_list_add_int(prop, buffer.array, *packed);
		}
		break;
	}

	dstr_free(&buffer);
}

/*
 * List framerates for device and resolution
 */
static void v4l2_framerate_list(int dev, uint_fast32_t pixelformat, uint_fast32_t width, uint_fast32_t height,
				obs_property_t *prop)
{
	struct v4l2_frmivalenum frmival;
	frmival.pixel_format = pixelformat;
	frmival.width = width;
	frmival.height = height;
	frmival.index = 0;
	struct dstr buffer;
	dstr_init(&buffer);

	obs_property_list_clear(prop);

	obs_property_list_add_int(prop, obs_module_text("LeaveUnchanged"), -1);

	v4l2_ioctl(dev, VIDIOC_ENUM_FRAMEINTERVALS, &frmival);

	switch (frmival.type) {
	case V4L2_FRMIVAL_TYPE_DISCRETE:
		while (v4l2_ioctl(dev, VIDIOC_ENUM_FRAMEINTERVALS, &frmival) == 0) {
			v4l2_framerate_list_add_discrete(prop, &buffer, frmival.discrete.numerator,
							 frmival.discrete.denominator);
			frmival.index++;
		}
		break;
	default:
		blog(LOG_INFO, "Stepwise and Continuous framerates "
			       "are currently hardcoded");

		for (const int64_t *packed = v4l2_framerates; *packed; ++packed) {
			int num;
			int denom;
			v4l2_unpack_tuple(&num, &denom, *packed);
			float fps = (float)denom / num;
			dstr_printf(&buffer, "%.2f", (double)fps);
			obs_property_list_add_int(prop, buffer.array, *packed);
		}
		break;
	}

	dstr_free(&buffer);
}

/* The parameter of a properties object: the source, and whether the device
 * list was filled in from the settings yet */
struct v4l2_props_param {
	struct v4l2_data *data;
	bool device_listed;
};

/*
 * Device selected callback
 */
static bool device_selected(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	struct v4l2_props_param *param = obs_properties_get_param(props);
	struct v4l2_data *data = param ? param->data : NULL;
	/* The first call fills the window from the settings; the next ones
	 * are the user picking a device. */
	bool picked = param && param->device_listed;
	if (param)
		param->device_listed = true;

	const char *device_id = obs_data_get_string(settings, "device_id");
	int dev = v4l2_open(device_id, O_RDWR | O_NONBLOCK | O_CLOEXEC);

	v4l2_props_set_enabled(props, p, (dev == -1) ? false : true);

	if (dev == -1)
		return false;

	obs_property_t *prop = obs_properties_get(props, "input");
	obs_properties_t *ctrl_props_new = v4l2_controls_properties();

	/* The sliders show the controls saved for this device: those of
	 * another device are put aside for it, so they are not written here.
	 * Settings from before this was recorded belong to the device the
	 * source first opened. */
	struct dstr identity = {0};
	struct dstr saved_for = {0};
	v4l2_device_identity(dev, &identity);

	/* The pick stores the camera with its node (obs-rust/obs-studio#2262).
	 * Opening the window does not: with the camera away and another one at
	 * its node, the source keeps waiting for its camera. */
	struct v4l2_identity camera, saved;
	v4l2_identity_load(settings, &saved);
	if (v4l2_identity_query(dev, device_id, &camera) && (picked || !v4l2_identity_known(&saved)) &&
	    !v4l2_identity_equal(&camera, &saved))
		v4l2_identity_save(settings, &camera);

	dstr_copy(&saved_for, obs_data_get_string(settings, V4L2_CONTROLS_DEVICE));
	if (dstr_is_empty(&saved_for) && data) {
		pthread_mutex_lock(&data->controls_mutex);
		dstr_copy_dstr(&saved_for, &data->controls_device);
		pthread_mutex_unlock(&data->controls_mutex);
	}
	v4l2_controls_use_device(dev, settings, saved_for.array ? saved_for.array : "",
				 identity.array ? identity.array : "");
	dstr_free(&saved_for);
	dstr_free(&identity);

	obs_properties_remove_by_name(props, "controls");

	v4l2_input_list(dev, prop);
	v4l2_update_controls(dev, ctrl_props_new, settings);
	v4l2_close(dev);

	obs_properties_add_group(props, "controls", obs_module_text("CameraCtrls"), OBS_GROUP_NORMAL, ctrl_props_new);

	obs_property_modified(prop, settings);

	return true;
}

/*
 * Input selected callback
 */
static bool input_selected(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	UNUSED_PARAMETER(p);
	int dev = v4l2_open(obs_data_get_string(settings, "device_id"), O_RDWR | O_NONBLOCK | O_CLOEXEC);
	if (dev == -1)
		return false;

	obs_property_t *prop = obs_properties_get(props, "pixelformat");
	v4l2_format_list(dev, prop);
	v4l2_close(dev);

	obs_property_modified(prop, settings);

	return true;
}

static int input_caps_open(const char *device)
{
	return v4l2_open(device, O_RDWR | O_NONBLOCK | O_CLOEXEC);
}

static int input_caps_close(int dev)
{
	return v4l2_close(dev);
}

static int input_caps_get(int dev, int input, uint32_t *caps)
{
	return (int)v4l2_get_input_caps(dev, input, caps);
}

static const struct v4l2_input_caps_ops input_caps_ops = {
	.open = input_caps_open,
	.close = input_caps_close,
	.get_input_caps = input_caps_get,
};

/*
 * Format selected callback
 */
static bool format_selected(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	UNUSED_PARAMETER(p);
	uint32_t caps = 0;
	int dev = v4l2_open_input_caps(&input_caps_ops, obs_data_get_string(settings, "device_id"),
				       (int)obs_data_get_int(settings, "input"), &caps);
	if (dev == -1)
		return false;

	obs_property_t *resolution = obs_properties_get(props, "resolution");
	obs_property_t *framerate = obs_properties_get(props, "framerate");
	obs_property_t *standard = obs_properties_get(props, "standard");
	obs_property_t *dv_timing = obs_properties_get(props, "dv_timing");

	obs_property_set_visible(resolution, (!caps) ? true : false);
	obs_property_set_visible(framerate, (!caps) ? true : false);
	obs_property_set_visible(standard, (caps & V4L2_IN_CAP_STD) ? true : false);
	obs_property_set_visible(dv_timing, (caps & V4L2_IN_CAP_DV_TIMINGS) ? true : false);

	if (!caps) {
		v4l2_resolution_list(dev, obs_data_get_int(settings, "pixelformat"), resolution);
	}
	if (caps & V4L2_IN_CAP_STD)
		v4l2_standard_list(dev, standard);
	if (caps & V4L2_IN_CAP_DV_TIMINGS)
		v4l2_dv_timing_list(dev, dv_timing);

	v4l2_close(dev);

	if (!caps)
		obs_property_modified(resolution, settings);
	if (caps & V4L2_IN_CAP_STD)
		obs_property_modified(standard, settings);
	if (caps & V4L2_IN_CAP_DV_TIMINGS)
		obs_property_modified(dv_timing, settings);

	return true;
}

/*
 * Resolution selected callback
 */
static bool resolution_selected(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	UNUSED_PARAMETER(p);
	int width, height;
	int dev = v4l2_open(obs_data_get_string(settings, "device_id"), O_RDWR | O_NONBLOCK | O_CLOEXEC);
	if (dev == -1)
		return false;

	obs_property_t *prop = obs_properties_get(props, "framerate");
	v4l2_unpack_tuple(&width, &height, obs_data_get_int(settings, "resolution"));
	v4l2_framerate_list(dev, obs_data_get_int(settings, "pixelformat"), width, height, prop);
	v4l2_close(dev);

	obs_property_modified(prop, settings);

	return true;
}

#define MAX_CANDIDATES 64

/*
 * Find the source's camera before opening it: the node may now be another
 * camera's, and the camera may have another node. Hint is a node that just
 * appeared. Without a known camera, the node in the settings, as before.
 *
 * Returns false when the camera is not there; the source then waits for it.
 */
static bool v4l2_find_device(struct v4l2_data *data, const char *hint)
{
	struct v4l2_identity saved;
	enum v4l2_identity_match match;

	pthread_mutex_lock(&data->device_mutex);
	saved = data->identity;
	char *path = bstrdup(data->device_id);
	pthread_mutex_unlock(&data->device_mutex);

	if (!v4l2_identity_known(&saved) || !path || !*path) {
		bfree(path);
		return true;
	}

	/* the saved node, the node that appeared, then every capture node */
	struct v4l2_candidate *list = bzalloc(sizeof(*list) * MAX_CANDIDATES);
	size_t count = 0;
	bool at_path = v4l2_candidate_at(path, &list[count]);
	count += at_path;
	if (hint && strcmp(hint, path) != 0)
		count += v4l2_candidate_at(hint, &list[count]);
	int pick = v4l2_identity_pick(&saved, path, list, count, &match);
	if (pick < 0 || match != V4L2_MATCH_EXACT) {
		count = v4l2_candidates_scan(list, count, MAX_CANDIDATES);
		pick = v4l2_identity_pick(&saved, path, list, count, &match);
	}

	struct dstr label = {0};
	v4l2_identity_label(&saved, &label);
	bool found = pick >= 0;
	if (!found) {
		int report = at_path ? 2 : 1;
		if (data->absent_reported != report) {
			if (at_path) {
				struct dstr other = {0};
				v4l2_identity_label(&list[0].identity, &other);
				blog(LOG_WARNING, "Not capturing %s: it is now %s. Waiting for %s to be connected",
				     path, other.array, label.array);
				dstr_free(&other);
			} else {
				blog(LOG_INFO, "Waiting for %s (last at %s) to be connected", label.array, path);
			}
		}
		data->absent_reported = report;
	} else {
		const struct v4l2_candidate *c = &list[pick];
		struct dstr old_label = {0}, new_label = {0};
		if (strcmp(c->path, path) != 0)
			blog(LOG_INFO, "%s is now at %s (was %s)", label.array, c->path, path);

		pthread_mutex_lock(&data->device_mutex);
		bfree(data->device_id);
		data->device_id = bstrdup(c->path);
		data->match = match;
		/* The same serial number on another port: the camera moved,
		 * remember its new port. A camera told only by its model keeps
		 * the old one, so the camera itself wins when it comes back. */
		if (match == V4L2_MATCH_SERIAL) {
			v4l2_identity_label(&data->identity, &old_label);
			snprintf(data->identity.bus_info, sizeof(data->identity.bus_info), "%s", c->identity.bus_info);
			v4l2_identity_label(&data->identity, &new_label);
		}
		pthread_mutex_unlock(&data->device_mutex);

		/* and its saved camera controls go with it */
		if (match == V4L2_MATCH_SERIAL) {
			pthread_mutex_lock(&data->controls_mutex);
			if (dstr_cmp(&data->controls_device, old_label.array) == 0)
				dstr_copy_dstr(&data->controls_device, &new_label);
			pthread_mutex_unlock(&data->controls_mutex);
		}
		dstr_free(&old_label);
		dstr_free(&new_label);
		data->absent_reported = 0;
	}

	dstr_free(&label);
	bfree(list);
	bfree(path);
	return found;
}

/* Open the saved camera at its node, -1 if it is not there (or not known) */
static int v4l2_open_saved_camera(struct v4l2_data *data)
{
	pthread_mutex_lock(&data->device_mutex);
	struct v4l2_identity saved = data->identity;
	char *path = bstrdup(data->device_id);
	pthread_mutex_unlock(&data->device_mutex);

	int fd = -1;
	struct v4l2_identity camera;
	if (v4l2_identity_known(&saved) && path && *path) {
		/* Close-on-exec, as in v4l2_init() */
		fd = v4l2_open(path, O_RDWR | O_NONBLOCK | O_CLOEXEC);
		if (fd != -1 && (!v4l2_identity_query(fd, path, &camera) ||
				 v4l2_identity_match(&saved, &camera) != V4L2_MATCH_EXACT)) {
			v4l2_close(fd);
			fd = -1;
		}
	}
	bfree(path);

	if (fd != -1) {
		pthread_mutex_lock(&data->device_mutex);
		data->match = V4L2_MATCH_EXACT;
		pthread_mutex_unlock(&data->device_mutex);
		data->absent_reported = 0;
	}
	return fd;
}

/* The camera was opened for the first time: record it (settings from
 * before it was saved, or a node picked without it) */
static void v4l2_record_device(struct v4l2_data *data)
{
	pthread_mutex_lock(&data->device_mutex);
	if (!v4l2_identity_known(&data->identity) && v4l2_identity_query(data->dev, data->device_id, &data->identity))
		data->match = V4L2_MATCH_EXACT;
	pthread_mutex_unlock(&data->device_mutex);
}

/* Write the node the camera was found at, and the camera, to the settings,
 * so obs-websocket, scripts and the properties see them (UI thread) */
static void v4l2_sync_settings(struct v4l2_data *data, obs_data_t *settings)
{
	if (!settings)
		return;

	pthread_mutex_lock(&data->device_mutex);
	if (v4l2_identity_known(&data->identity) && data->device_id) {
		struct v4l2_identity saved;
		if (strcmp(obs_data_get_string(settings, "device_id"), data->device_id) != 0)
			obs_data_set_string(settings, "device_id", data->device_id);
		v4l2_identity_load(settings, &saved);
		if (!v4l2_identity_equal(&saved, &data->identity))
			v4l2_identity_save(settings, &data->identity);
		bfree(data->settings_device_id);
		data->settings_device_id = bstrdup(data->device_id);
		data->settings_identity = data->identity;
	}
	pthread_mutex_unlock(&data->device_mutex);
}

static void v4l2_sync_task(void *param)
{
	obs_weak_source_t *weak = param;
	obs_source_t *source = obs_weak_source_get_source(weak);
	obs_weak_source_release(weak);
	if (!source)
		return;

	obs_data_t *settings = obs_source_get_settings(source);
	v4l2_sync_settings(obs_obj_get_data(source), settings);
	obs_data_release(settings);
	obs_source_release(source);
}

/* From the udev thread */
static void v4l2_queue_sync(struct v4l2_data *data)
{
	if (data->source)
		obs_queue_task(OBS_TASK_UI, v4l2_sync_task, obs_source_get_weak_source(data->source), false);
}

#ifdef HAVE_UDEV
/**
 * Device added callback
 *
 * If everything went fine we can start capturing again when the device is
 * reconnected
 */
static void device_added(void *vptr, calldata_t *calldata)
{
	V4L2_DATA(vptr);

	obs_source_update_properties(data->source);

	const char *dev;
	calldata_get_string(calldata, "device", &dev);
	if (!dev)
		return;

	pthread_mutex_lock(&data->device_mutex);
	struct v4l2_identity saved = data->identity;
	bool at_node = data->device_id && strcmp(data->device_id, dev) == 0;
	enum v4l2_identity_match current = data->match;
	pthread_mutex_unlock(&data->device_mutex);

	/* A camera not recorded yet: its node, as before */
	if (!v4l2_identity_known(&saved)) {
		if (!at_node)
			return;
		blog(LOG_INFO, "Device %s reconnected", dev);
		v4l2_init(data, NULL);
		return;
	}

	/* Capturing: only the camera itself, back on its own port, replaces
	 * a camera taken for it by model */
	if (data->dev != -1 && current != V4L2_MATCH_MODEL)
		return;

	/* At its node, v4l2_init() checks the camera there. Elsewhere, the
	 * node is opened only to see which camera it is. */
	enum v4l2_identity_match match = V4L2_MATCH_NONE;
	struct v4l2_candidate candidate;
	if ((!at_node || data->dev != -1) && v4l2_candidate_at(dev, &candidate))
		match = v4l2_identity_match(&saved, &candidate.identity);

	if (data->dev != -1) {
		if (match != V4L2_MATCH_EXACT)
			return;
		blog(LOG_INFO, "Device %s is the camera itself, switching to it", dev);
		v4l2_terminate(data);
	} else if (!at_node && match == V4L2_MATCH_NONE) {
		return;
	} else {
		blog(LOG_INFO, "Device %s reconnected", dev);
	}

	v4l2_init(data, dev);
	v4l2_queue_sync(data);
}
/**
 * Device removed callback
 *
 * We stop recording here so we don't block the device node
 */
static void device_removed(void *vptr, calldata_t *calldata)
{
	V4L2_DATA(vptr);

	obs_source_update_properties(data->source);

	const char *dev;
	calldata_get_string(calldata, "device", &dev);

	pthread_mutex_lock(&data->device_mutex);
	bool at_node = dev && data->device_id && strcmp(data->device_id, dev) == 0;
	pthread_mutex_unlock(&data->device_mutex);
	if (!at_node)
		return;

	blog(LOG_INFO, "Device %s disconnected", dev);

	v4l2_terminate(data);
}

#endif

#define BUSY_CHECK_NS 1000000000ULL
#define FORCE_TIMEOUT_MS 5000

/*
 * Tell the frontend, which shows a "Camera is in use" popup with a Force
 * Camera button calling the source's force_camera proc. The frontend declares
 * the signal on the global handler; without a frontend nobody has, and
 * signalling an undeclared signal does nothing.
 *
 * void source_device_busy(ptr source, string device, string holder, int pid,
 *                         bool is_obs, bool can_force)
 */
static void v4l2_busy_announce(struct v4l2_data *data, const struct v4l2_holder *holder)
{
	signal_handler_t *sh = obs_get_signal_handler();
	if (!sh || !data->source)
		return;

	calldata_t cd = {0};
	calldata_set_ptr(&cd, "source", data->source);
	calldata_set_string(&cd, "device", data->device_id);
	calldata_set_string(&cd, "holder", holder ? holder->name : "");
	calldata_set_int(&cd, "pid", holder ? holder->pid : 0);
	calldata_set_bool(&cd, "is_obs", holder && v4l2_holder_is_obs(holder));
	calldata_set_bool(&cd, "can_force", holder && holder->uid == getuid());
	signal_handler_signal(sh, "source_device_busy", &cd);
	calldata_free(&cd);
}

/*
 * Record whether the last v4l2_init() failed because another process has the
 * device, and who that is. Called with the errno of the failure, or 0.
 */
static void v4l2_busy_update(struct v4l2_data *data, int err)
{
	bool busy = v4l2_busy_errno(err);
	struct v4l2_holder holder;
	bool known = busy && v4l2_find_holders("/proc", data->device_id, getpid(), &holder, 1) == 1;

	pthread_mutex_lock(&data->busy_mutex);
	bool was_busy = data->busy;
	data->busy = busy;
	if (busy) {
		data->busy_holder_known = known;
		if (known)
			data->busy_holder = holder;
		data->busy_attempt = was_busy ? data->busy_attempt + 1 : 0;
		uint64_t now = os_gettime_ns();
		data->busy_retry_ns = now + (uint64_t)v4l2_busy_retry_delay_ms(data->busy_attempt) * 1000000ULL;
		data->busy_check_ns = now + BUSY_CHECK_NS;
	}
	pthread_mutex_unlock(&data->busy_mutex);

	if (busy && !was_busy) {
		struct dstr text = {0};
		v4l2_busy_describe(&text, data->device_id, known ? &holder : NULL);
		blog(LOG_WARNING, "%s%s; retrying until it is free", text.array,
		     known && v4l2_holder_is_obs(&holder) ? ", another OBS instance" : "");
		dstr_free(&text);
		/* once per busy episode, not on every retry */
		v4l2_busy_announce(data, known ? &holder : NULL);
	}
	if (busy != was_busy || (busy && known))
		obs_source_update_properties(data->source);
}

/* UI task: retry the device once its holder let go, or when the backoff
 * delay has passed (holders of other users cannot be seen). */
static void v4l2_busy_task(void *param)
{
	obs_weak_source_t *weak = param;
	obs_source_t *source = obs_weak_source_get_source(weak);
	obs_weak_source_release(weak);
	if (!source)
		return;

	struct v4l2_data *data = obs_obj_get_data(source);

	pthread_mutex_lock(&data->busy_mutex);
	data->busy_task_queued = false;
	bool busy = data->busy;
	bool known = data->busy_holder_known;
	struct v4l2_holder holder = data->busy_holder;
	bool retry_due = os_gettime_ns() >= data->busy_retry_ns;
	pthread_mutex_unlock(&data->busy_mutex);

	if (busy && data->dev == -1) {
		struct v4l2_holder now_holding;
		bool released = known && (v4l2_find_holders("/proc", data->device_id, getpid(), &now_holding, 1) == 0 ||
					  now_holding.pid != holder.pid);
		if (released)
			blog(LOG_INFO, "%s (PID %d) released %s", holder.name, (int)holder.pid, data->device_id);
		if (released || retry_due)
			v4l2_init(data, NULL);
	}

	obs_source_release(source);
}

static void v4l2_video_tick(void *vptr, float seconds)
{
	V4L2_DATA(vptr);
	UNUSED_PARAMETER(seconds);

	pthread_mutex_lock(&data->busy_mutex);
	uint64_t now = os_gettime_ns();
	bool due = data->busy && !data->busy_task_queued && now >= data->busy_check_ns;
	if (due) {
		data->busy_task_queued = true;
		data->busy_check_ns = now + BUSY_CHECK_NS;
	}
	pthread_mutex_unlock(&data->busy_mutex);

	/* /proc is scanned and the device opened on the UI thread, which also
	 * runs v4l2_update(), not on the graphics thread */
	if (due)
		obs_queue_task(OBS_TASK_UI, v4l2_busy_task, obs_source_get_weak_source(data->source), false);
}

/* "Force Camera": ask the holder to close, then open the device. Returns
 * whether the holder let go. */
static bool v4l2_force_camera_now(struct v4l2_data *data)
{
	pthread_mutex_lock(&data->busy_mutex);
	bool can_force = data->busy && data->busy_holder_known;
	struct v4l2_holder holder = data->busy_holder;
	pthread_mutex_unlock(&data->busy_mutex);
	if (!can_force)
		return false;

	blog(LOG_INFO, "Force Camera: asking %s (PID %d) to close", holder.name, (int)holder.pid);
	enum v4l2_force_result result =
		v4l2_force_release("/proc", data->device_id, &holder, getuid(), FORCE_TIMEOUT_MS);
	switch (result) {
	case V4L2_FORCE_RELEASED:
		v4l2_init(data, NULL);
		break;
	case V4L2_FORCE_NOT_OWNER:
		blog(LOG_WARNING, "Force Camera: %s (PID %d) belongs to another user, not closed", holder.name,
		     (int)holder.pid);
		break;
	case V4L2_FORCE_TIMEOUT:
		blog(LOG_WARNING, "Force Camera: %s (PID %d) did not close within %d s; it was not killed", holder.name,
		     (int)holder.pid, FORCE_TIMEOUT_MS / 1000);
		break;
	case V4L2_FORCE_ERROR:
		blog(LOG_WARNING, "Force Camera: could not signal %s (PID %d): %s", holder.name, (int)holder.pid,
		     strerror(errno));
		break;
	}
	return result == V4L2_FORCE_RELEASED;
}

/* The Force Camera button in the source properties */
static bool v4l2_force_camera(obs_properties_t *props, obs_property_t *property, void *vptr)
{
	UNUSED_PARAMETER(props);
	UNUSED_PARAMETER(property);
	v4l2_force_camera_now(vptr);
	return true;
}

/* proc "void force_camera(out bool released)", for the frontend's popup */
static void v4l2_force_camera_proc(void *vptr, calldata_t *cd)
{
	calldata_set_bool(cd, "released", v4l2_force_camera_now(vptr));
}

/* While the device is busy: who has it, and the Force Camera button */
static void v4l2_busy_properties(struct v4l2_data *data, obs_properties_t *props)
{
	pthread_mutex_lock(&data->busy_mutex);
	bool busy = data->busy;
	bool known = data->busy_holder_known;
	struct v4l2_holder holder = data->busy_holder;
	pthread_mutex_unlock(&data->busy_mutex);
	if (!busy)
		return;

	struct dstr text = {0};
	v4l2_busy_describe(&text, data->device_id, known ? &holder : NULL);
	dstr_cat(&text, ". ");
	if (known && v4l2_holder_is_obs(&holder)) {
		dstr_cat(&text, obs_module_text("DeviceBusy.OtherOBS"));
		dstr_cat(&text, " ");
	}
	dstr_cat(&text, obs_module_text("DeviceBusy.Retrying"));

	obs_property_t *info = obs_properties_add_text(props, "device_busy", text.array, OBS_TEXT_INFO);
	obs_property_text_set_info_type(info, OBS_TEXT_INFO_WARNING);

	/* Only another process of this user can be closed */
	if (known && holder.uid == getuid()) {
		obs_property_t *button = obs_properties_add_button2(
			props, "force_camera", obs_module_text("ForceCamera"), v4l2_force_camera, data);
		char pid[16];
		snprintf(pid, sizeof(pid), "%d", (int)holder.pid);
		dstr_copy(&text, obs_module_text("ForceCamera.ToolTip"));
		dstr_replace(&text, "%1", holder.name);
		dstr_replace(&text, "%2", pid);
		obs_property_set_long_description(button, text.array);
	}
	dstr_free(&text);
}

static obs_properties_t *v4l2_properties(void *vptr)
{
	V4L2_DATA(vptr);

	obs_properties_t *props = obs_properties_create();
	struct v4l2_props_param *param = bzalloc(sizeof(*param));
	param->data = data;
	obs_properties_set_param(props, param, bfree);

	v4l2_busy_properties(data, props);

	obs_property_t *device_list = obs_properties_add_list(props, "device_id", obs_module_text("Device"),
							      OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_STRING);

	obs_property_t *input_list = obs_properties_add_list(props, "input", obs_module_text("Input"),
							     OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);

	obs_property_t *format_list = obs_properties_add_list(props, "pixelformat", obs_module_text("VideoFormat"),
							      OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);

	obs_property_t *standard_list = obs_properties_add_list(props, "standard", obs_module_text("VideoStandard"),
								OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);
	obs_property_set_visible(standard_list, false);

	obs_property_t *dv_timing_list = obs_properties_add_list(props, "dv_timing", obs_module_text("DVTiming"),
								 OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);
	obs_property_set_visible(dv_timing_list, false);

	obs_property_t *resolution_list = obs_properties_add_list(props, "resolution", obs_module_text("Resolution"),
								  OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);

	obs_properties_add_list(props, "framerate", obs_module_text("FrameRate"), OBS_COMBO_TYPE_LIST,
				OBS_COMBO_FORMAT_INT);

	obs_property_t *color_range_list = obs_properties_add_list(props, "color_range", obs_module_text("ColorRange"),
								   OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);
	obs_property_list_add_int(color_range_list, obs_module_text("ColorRange.Default"), VIDEO_RANGE_DEFAULT);
	obs_property_list_add_int(color_range_list, obs_module_text("ColorRange.Partial"), VIDEO_RANGE_PARTIAL);
	obs_property_list_add_int(color_range_list, obs_module_text("ColorRange.Full"), VIDEO_RANGE_FULL);

	obs_properties_add_bool(props, "buffering", obs_module_text("UseBuffering"));

	obs_properties_add_bool(props, "auto_reset", obs_module_text("AutoresetOnTimeout"));

	obs_properties_add_int(props, "timeout_frames", obs_module_text("FramesUntilTimeout"), 2, 120, 1);

	// a group to contain the camera control
	obs_properties_t *ctrl_props = v4l2_controls_properties();
	obs_properties_add_group(props, "controls", obs_module_text("CameraCtrls"), OBS_GROUP_NORMAL, ctrl_props);

	obs_data_t *settings = obs_source_get_settings(data->source);
	/* the list selects the node the camera is at now */
	v4l2_sync_settings(data, settings);
	v4l2_device_list(device_list, settings);
	obs_data_release(settings);

	obs_property_set_modified_callback(device_list, device_selected);
	obs_property_set_modified_callback(input_list, input_selected);
	obs_property_set_modified_callback(format_list, format_selected);
	obs_property_set_modified_callback(resolution_list, resolution_selected);

	return props;
}

static void v4l2_terminate(struct v4l2_data *data)
{
	if (data->thread) {
		os_event_signal(data->event);
		pthread_join(data->thread, NULL);
		os_event_destroy(data->event);
		data->thread = 0;
	}

	v4l2_close_device(data);
}

/*
 * Free what v4l2_setup_device() set up and close the device, once no capture
 * runs on it
 */
static void v4l2_close_device(struct v4l2_data *data)
{
	if (data->pixfmt == V4L2_PIX_FMT_MJPEG || data->pixfmt == V4L2_PIX_FMT_H264) {
		v4l2_destroy_decoder(&data->decoder);
	}
	v4l2_destroy_mmap(&data->buffers);

	if (data->dev != -1) {
		/* v4l2_restore_controls() uses the device from other threads */
		pthread_mutex_lock(&data->controls_mutex);
		v4l2_close(data->dev);
		data->dev = -1;
		pthread_mutex_unlock(&data->controls_mutex);
	}
}

static void v4l2_destroy(void *vptr)
{
	V4L2_DATA(vptr);

	if (!data)
		return;

	v4l2_terminate(data);

	if (data->device_id)
		bfree(data->device_id);
	bfree(data->settings_device_id);
	pthread_mutex_destroy(&data->device_mutex);
	obs_data_release(data->controls);
	dstr_free(&data->controls_device);
	dstr_free(&data->dev_identity);
	pthread_mutex_destroy(&data->controls_mutex);
	pthread_mutex_destroy(&data->busy_mutex);

#ifdef HAVE_UDEV
	signal_handler_t *sh = v4l2_get_udev_signalhandler();

	signal_handler_disconnect(sh, "device_added", device_added, data);
	signal_handler_disconnect(sh, "device_removed", device_removed, data);

	v4l2_unref_udev();
#endif

	bfree(data);
}

/*
 * Write the saved camera controls, if the user wants them restored and they
 * were set on the open device. A source switched to another camera, or
 * another camera that took the source's /dev/videoN node (the numbers follow
 * the order the cameras are found in, after a reboot or a replug), must not
 * get them.
 */
static void v4l2_restore_controls(struct v4l2_data *data)
{
	bool same_device;

	pthread_mutex_lock(&data->controls_mutex);
	if (dstr_is_empty(&data->dev_identity)) {
		/* A device that cannot tell what it is: as before */
		same_device = dstr_is_empty(&data->controls_device);
	} else if (dstr_is_empty(&data->controls_device)) {
		/* Settings saved before the device was recorded: they belong
		 * to the first device opened. */
		dstr_copy_dstr(&data->controls_device, &data->dev_identity);
		same_device = true;
	} else {
		same_device = dstr_cmp(&data->controls_device, data->dev_identity.array) == 0;
	}

	if (!same_device)
		blog(LOG_INFO, "Not restoring the camera controls, they were set on %s", data->controls_device.array);
	else if (data->restore_controls)
		v4l2_apply_controls(data->dev, data->controls);
	pthread_mutex_unlock(&data->controls_mutex);
}

/*
 * The framerate a saved setting asks for at the device's format and size
 */
static int64_t v4l2_saved_framerate(int_fast32_t dev, uint32_t pixelformat, int width, int height, int64_t setting)
{
	struct v4l2_fract intervals[64];
	size_t count = 0;
	struct v4l2_frmivalenum frmival = {
		.pixel_format = pixelformat,
		.width = width,
		.height = height,
	};

	while (count < sizeof(intervals) / sizeof(intervals[0]) &&
	       v4l2_ioctl(dev, VIDIOC_ENUM_FRAMEINTERVALS, &frmival) == 0 &&
	       frmival.type == V4L2_FRMIVAL_TYPE_DISCRETE) {
		intervals[count++] = frmival.discrete;
		frmival.index++;
	}

	return v4l2_framerate_saved(setting, intervals, count);
}

/**
 * Initialize the v4l2 device
 *
 * This function:
 * - tries to open the device
 * - sets pixelformat and requested resolution
 * - sets the requested framerate
 * - maps the buffers
 * - starts the capture thread
 */
static void v4l2_init(struct v4l2_data *data, const char *hint)
{
	/* A capture thread still waiting to reopen its failed device has no
	 * device open (data->dev is -1), so device_added() takes the source
	 * for stopped: stop that thread before starting another. */
	if (data->thread)
		v4l2_terminate(data);

	/* The camera at its node opens right away; otherwise it is looked
	 * for, and when it is not there the source waits for it. */
	int fd = v4l2_open_saved_camera(data);
	if (fd == -1 && !v4l2_find_device(data, hint)) {
		v4l2_busy_update(data, 0);
		return;
	}

	blog(LOG_INFO, "Start capture from %s", data->device_id);
	/* Close-on-exec, so processes OBS starts (obs-ffmpeg-mux while recording)
	 * do not keep the device busy after the source closes it. */
	data->dev = fd != -1 ? fd : v4l2_open(data->device_id, O_RDWR | O_NONBLOCK | O_CLOEXEC);
	if (data->dev == -1) {
		blog(LOG_ERROR, "Unable to open device");
		goto fail;
	}
	if (v4l2_setup_device(data) < 0)
		goto fail;

	/* start the capture thread */
	if (os_event_init(&data->event, OS_EVENT_TYPE_MANUAL) != 0)
		goto fail;
	if (pthread_create(&data->thread, NULL, v4l2_thread, data) != 0)
		goto fail;
	v4l2_busy_update(data, 0);
	return;
fail:;
	int err = errno;
	blog(LOG_ERROR, "Initialization failed, errno: %s", strerror(err));
	v4l2_terminate(data);
	v4l2_busy_update(data, err);
}

/*
 * Set up the open device for capture: input, standard or dv timing, format,
 * framerate, the saved controls, the buffers and the decoder. Also used when
 * the capture thread reopens a failed device. Returns negative on failure,
 * with errno set; v4l2_close_device() frees what was set up.
 */
static int v4l2_setup_device(struct v4l2_data *data)
{
	uint32_t input_caps;
	int fps_num, fps_denom;
	int64_t saved;

	pthread_mutex_lock(&data->controls_mutex);
	v4l2_device_identity(data->dev, &data->dev_identity);
	pthread_mutex_unlock(&data->controls_mutex);
	v4l2_record_device(data);

	/* set input */
	if (v4l2_set_input(data->dev, &data->input) < 0) {
		blog(LOG_ERROR, "Unable to set input %d", data->input);
		return -1;
	}
	blog(LOG_INFO, "Input: %d", data->input);
	if (v4l2_get_input_caps(data->dev, -1, &input_caps) < 0) {
		blog(LOG_ERROR, "Unable to get input capabilities");
		return -1;
	}

	/* set video standard if supported */
	if (input_caps & V4L2_IN_CAP_STD) {
		if (v4l2_set_standard(data->dev, &data->standard) < 0) {
			blog(LOG_ERROR, "Unable to set video standard");
			return -1;
		}
		data->resolution = -1;
		data->framerate = -1;
	}
	/* set dv timing if supported */
	if (input_caps & V4L2_IN_CAP_DV_TIMINGS) {
		if (v4l2_set_dv_timing(data->dev, &data->dv_timing) < 0) {
			blog(LOG_ERROR, "Unable to set dv timing");
			return -1;
		}
		data->resolution = -1;
		data->framerate = -1;
	}

	/* set pixel format and resolution */
	if (v4l2_set_format(data->dev, &data->resolution, &data->pixfmt, &data->linesize) < 0) {
		blog(LOG_ERROR, "Unable to set format");
		return -1;
	}
	if (v4l2_to_obs_video_format(data->pixfmt) == VIDEO_FORMAT_NONE && data->pixfmt != V4L2_PIX_FMT_MJPEG &&
	    data->pixfmt != V4L2_PIX_FMT_H264) {
		blog(LOG_ERROR, "Selected video format not supported");
		return -1;
	}
	v4l2_unpack_tuple(&data->width, &data->height, data->resolution);
	blog(LOG_INFO, "Resolution: %dx%d", data->width, data->height);
	blog(LOG_INFO, "Pixelformat: %s", V4L2_FOURCC_STR(data->pixfmt));
	blog(LOG_INFO, "Linesize: %d Bytes", data->linesize);

	/* set framerate */
	saved = v4l2_saved_framerate(data->dev, data->pixfmt, data->width, data->height, data->framerate);
	if (saved != data->framerate) {
		blog(LOG_INFO, "Framerate setting %" PRId64 " was saved by an older version, using %" PRId64,
		     data->framerate, saved);
		data->framerate = saved;
		data->framerate_setting = saved;
	}
	if (v4l2_set_framerate(data->dev, &data->framerate) < 0) {
		blog(LOG_ERROR, "Unable to set framerate");
		return -1;
	}
	if (data->framerate == 0) {
		blog(LOG_ERROR, "Framerate is not set, falling back to %i", FALLBACK_FRAMERATE);
		data->framerate = v4l2_pack_tuple(1, FALLBACK_FRAMERATE);
	}
	v4l2_unpack_tuple(&fps_num, &fps_denom, data->framerate);
	blog(LOG_INFO, "Framerate: %.2f fps", (double)((float)fps_denom / fps_num));

	/* The camera starts on the driver's values when it is plugged in, and
	 * other programs can change them: write the saved controls, unless the
	 * user leaves them to another program (an external PTZ controller). */
	v4l2_restore_controls(data);

	/* map buffers */
	if (v4l2_create_mmap(data->dev, &data->buffers) < 0) {
		blog(LOG_ERROR, "Failed to map buffers");
		return -1;
	}

	if (data->pixfmt == V4L2_PIX_FMT_MJPEG || data->pixfmt == V4L2_PIX_FMT_H264) {
		if (v4l2_init_decoder(&data->decoder, data->pixfmt) < 0) {
			blog(LOG_ERROR, "Failed to initialize decoder");
			return -1;
		}
	}

	return 0;
}

/** Update source flags depending on the settings */
static void v4l2_update_source_flags(struct v4l2_data *data, obs_data_t *settings)
{
	obs_source_set_async_unbuffered(data->source, !obs_data_get_bool(settings, "buffering"));
}

/**
 * Checking if any of the settings have changed so that we can restart the
 * stream
 */
static bool v4l2_settings_changed(struct v4l2_data *data, obs_data_t *settings)
{
	bool res = false;

	if (obs_data_get_string(settings, "device_id") != NULL && data->settings_device_id != NULL) {
		/* another node or camera picked; not the node the source moved
		 * its camera to */
		struct v4l2_identity identity;
		v4l2_identity_load(settings, &identity);
		res |= strcmp(data->settings_device_id, obs_data_get_string(settings, "device_id")) != 0;
		res |= v4l2_identity_known(&identity) && !v4l2_identity_equal(&identity, &data->settings_identity);
		res |= data->input != obs_data_get_int(settings, "input");
		res |= data->pixfmt != obs_data_get_int(settings, "pixelformat");
		res |= data->standard != obs_data_get_int(settings, "standard");
		res |= data->dv_timing != obs_data_get_int(settings, "dv_timing");

		if (obs_data_get_int(settings, "resolution") == -1 && !data->resolution_unchanged) {
			data->resolution_unchanged = true;
			res |= true;
		} else if (obs_data_get_int(settings, "resolution") == -1 && data->resolution_unchanged) {
			res |= false;
		} else {
			data->resolution_unchanged = false;
			res |= (data->resolution != obs_data_get_int(settings, "resolution")) &&
			       (obs_data_get_int(settings, "resolution") != -1);
		}

		res |= v4l2_framerate_changed(data->framerate, obs_data_get_int(settings, "framerate"),
					      &data->framerate_unchanged);

		res |= data->color_range != obs_data_get_int(settings, "color_range");
	} else {
		res = true;
	}

	return res;
}

/*
 * The node and camera from the settings. The source keeps the camera it
 * found while the settings name the same node and camera: they may still
 * have the node it moved from. A node picked without naming the camera (a
 * script, obs-websocket) is the camera found there when it opens.
 */
static void v4l2_update_device(struct v4l2_data *data, obs_data_t *settings)
{
	const char *device_id = obs_data_get_string(settings, "device_id");
	struct v4l2_identity identity;
	v4l2_identity_load(settings, &identity);

	bool same_node = data->settings_device_id && strcmp(data->settings_device_id, device_id) == 0;
	bool same_camera = !v4l2_identity_known(&identity) || v4l2_identity_equal(&identity, &data->settings_identity);
	if (same_node && same_camera && data->device_id)
		return;
	if (!same_node && same_camera)
		v4l2_identity_clear(&identity);

	pthread_mutex_lock(&data->device_mutex);
	bfree(data->device_id);
	data->device_id = bstrdup(device_id);
	data->identity = identity;
	data->match = V4L2_MATCH_EXACT;
	pthread_mutex_unlock(&data->device_mutex);
	data->absent_reported = 0;

	bfree(data->settings_device_id);
	data->settings_device_id = bstrdup(device_id);
	data->settings_identity = identity;
}

/**
 * Update the settings for the v4l2 source
 *
 * There are a few settings that can be changed without restarting the stream
 * Whenever this is called the currently active stream (if exists) is stopped,
 * the settings are updated and finally the new stream is started.
 */
static void v4l2_update(void *vptr, obs_data_t *settings)
{

	V4L2_DATA(vptr);

	bool needs_restart = v4l2_settings_changed(data, settings);

	if (needs_restart)
		v4l2_terminate(data);

	/* The capture thread logs it: replace it only while stopped. Without a
	 * restart, v4l2_settings_changed found it unchanged (#2103). */
	if (needs_restart || !data->settings_device_id)
		v4l2_update_device(data, settings);
	data->input = obs_data_get_int(settings, "input");
	data->pixfmt = obs_data_get_int(settings, "pixelformat");
	data->standard = obs_data_get_int(settings, "standard");
	data->dv_timing = obs_data_get_int(settings, "dv_timing");
	data->resolution = obs_data_get_int(settings, "resolution");
	data->framerate = obs_data_get_int(settings, "framerate");
	data->color_range = obs_data_get_int(settings, "color_range");
	data->auto_reset = obs_data_get_bool(settings, "auto_reset");
	data->timeout_frames = obs_data_get_int(settings, "timeout_frames");

	/* Ticking the option writes nothing: it takes effect the next time
	 * the device opens or the settings change. */
	bool restore_was_on = data->restore_controls;
	data->restore_controls = obs_data_get_bool(settings, "restore_controls");

	/* A copy, as a replug reopens the device from the udev thread. Settings
	 * that do not say which device the controls belong to keep the one the
	 * source found when it first opened a device. */
	pthread_mutex_lock(&data->controls_mutex);
	obs_data_release(data->controls);
	data->controls = obs_data_create();
	obs_data_apply(data->controls, settings);
	const char *controls_device = obs_data_get_string(settings, V4L2_CONTROLS_DEVICE);
	if (*controls_device)
		dstr_copy(&data->controls_device, controls_device);
	pthread_mutex_unlock(&data->controls_mutex);

	v4l2_update_source_flags(data, settings);

	if (needs_restart) {
		v4l2_init(data, NULL);
		/* Store a framerate setting saved by an older version as the
		 * interval it stands for, so it matches the list and the
		 * running device from now on. */
		if (data->framerate_setting) {
			obs_data_set_int(settings, "framerate", data->framerate_setting);
			data->framerate_setting = 0;
		}
	} else if (data->dev != -1 && restore_was_on)
		v4l2_restore_controls(data);

	v4l2_sync_settings(data, settings);
}

/* Record the device found for settings that did not say it */
static void v4l2_save(void *vptr, obs_data_t *settings)
{
	V4L2_DATA(vptr);

	v4l2_sync_settings(data, settings);

	pthread_mutex_lock(&data->controls_mutex);
	if (!dstr_is_empty(&data->controls_device) && !*obs_data_get_string(settings, V4L2_CONTROLS_DEVICE))
		obs_data_set_string(settings, V4L2_CONTROLS_DEVICE, data->controls_device.array);
	pthread_mutex_unlock(&data->controls_mutex);
}

static void *v4l2_create(obs_data_t *settings, obs_source_t *source)
{
	struct v4l2_data *data = bzalloc(sizeof(struct v4l2_data));
	data->dev = -1;
	data->source = source;
	data->resolution_unchanged = false;
	data->framerate_unchanged = false;
	pthread_mutex_init(&data->controls_mutex, NULL);
	pthread_mutex_init(&data->busy_mutex, NULL);
	pthread_mutex_init(&data->device_mutex, NULL);
	v4l2_identity_clear(&data->identity);
	v4l2_identity_clear(&data->settings_identity);

	proc_handler_t *ph = obs_source_get_proc_handler(source);
	if (ph)
		proc_handler_add(ph, "void force_camera(out bool released)", v4l2_force_camera_proc, data);

	/* Bitch about build problems ... */
#ifndef V4L2_CAP_DEVICE_CAPS
	blog(LOG_WARNING, "Plugin built without device caps support!");
#endif
#if !defined(VIDIOC_ENUM_DV_TIMINGS) || !defined(V4L2_IN_CAP_DV_TIMINGS)
	blog(LOG_WARNING, "Plugin built without dv-timing support!");
#endif

	v4l2_update(data, settings);

#ifdef HAVE_UDEV
	v4l2_init_udev();
	signal_handler_t *sh = v4l2_get_udev_signalhandler();

	signal_handler_connect(sh, "device_added", &device_added, data);
	signal_handler_connect(sh, "device_removed", &device_removed, data);
#else
	blog(LOG_INFO, "Compiled without libudev, you can't reconnect devices");
#endif

	return data;
}

struct obs_source_info v4l2_input = {
	.id = "v4l2_input",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_DO_NOT_DUPLICATE,
	.get_name = v4l2_getname,
	.create = v4l2_create,
	.destroy = v4l2_destroy,
	.update = v4l2_update,
	.video_tick = v4l2_video_tick,
	.save = v4l2_save,
	.get_defaults = v4l2_defaults,
	.get_properties = v4l2_properties,
	.icon_type = OBS_ICON_TYPE_CAMERA,
};

/* File-local macros: keep them out of the next source of a unity build. */
#undef blog

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef V4L2_IN_CAP_DV_TIMINGS
#undef V4L2_DATA
#undef timeval2ns
#undef V4L2_FOURCC_STR
#undef BUSY_CHECK_NS
#undef FORCE_TIMEOUT_MS
#undef FALLBACK_FRAMERATE
#undef MAX_CANDIDATES
