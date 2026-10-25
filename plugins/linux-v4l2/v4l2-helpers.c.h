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

#define blog(level, msg, ...) blog(level, "v4l2-helpers: " msg, ##__VA_ARGS__)

int_fast32_t v4l2_start_capture(int_fast32_t dev, struct v4l2_buffer_data *buf)
{
	enum v4l2_buf_type type;
	struct v4l2_buffer enq;

	memset(&enq, 0, sizeof(enq));
	enq.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	enq.memory = V4L2_MEMORY_MMAP;

	for (enq.index = 0; enq.index < buf->count; ++enq.index) {
		if (v4l2_ioctl(dev, VIDIOC_QBUF, &enq) < 0) {
			blog(LOG_ERROR, "unable to queue buffer");
			return -1;
		}
	}

	type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	if (v4l2_ioctl(dev, VIDIOC_STREAMON, &type) < 0) {
		blog(LOG_ERROR, "unable to start stream");
		return -1;
	}

	return 0;
}

int_fast32_t v4l2_stop_capture(int_fast32_t dev)
{
	enum v4l2_buf_type type;

	type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	if (v4l2_ioctl(dev, VIDIOC_STREAMOFF, &type) < 0) {
		blog(LOG_ERROR, "unable to stop stream");
		return -1;
	}

	return 0;
}

int_fast32_t v4l2_reset_capture(int_fast32_t dev, struct v4l2_buffer_data *buf)
{
	blog(LOG_DEBUG, "attempting to reset capture");
	if (v4l2_stop_capture(dev) < 0)
		return -1;
	if (v4l2_start_capture(dev, buf) < 0)
		return -1;

	return 0;
}

#ifdef _DEBUG
int_fast32_t v4l2_query_all_buffers(int_fast32_t dev, struct v4l2_buffer_data *buf_data)
{
	struct v4l2_buffer buf;

	blog(LOG_DEBUG, "attempting to read buffer data for %" PRIuFAST32 " buffers", buf_data->count);

	for (uint_fast32_t i = 0; i < buf_data->count; i++) {
		buf.index = i;
		buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
		buf.memory = V4L2_MEMORY_MMAP;
		if (v4l2_ioctl(dev, VIDIOC_QUERYBUF, &buf) < 0) {
			blog(LOG_DEBUG, "failed to read buffer data for buffer #%" PRIuFAST32, i);
		} else {
			blog(LOG_DEBUG,
			     "query buf #%" PRIuFAST32
			     " info: ts: %06ld buf id #%d, flags 0x%08X, seq #%d, len %d, used %d",
			     i, buf.timestamp.tv_usec, buf.index, buf.flags, buf.sequence, buf.length, buf.bytesused);
		}
	}

	return 0;
}
#endif

int_fast32_t v4l2_create_mmap(int_fast32_t dev, struct v4l2_buffer_data *buf)
{
	struct v4l2_requestbuffers req;
	struct v4l2_buffer map;

	memset(&req, 0, sizeof(req));
	req.count = 4;
	req.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	req.memory = V4L2_MEMORY_MMAP;

	if (v4l2_ioctl(dev, VIDIOC_REQBUFS, &req) < 0) {
		blog(LOG_ERROR, "Request for buffers failed !");
		return -1;
	}

	if (req.count < 2) {
		blog(LOG_ERROR, "Device returned less than 2 buffers");
		return -1;
	}

	buf->count = req.count;
	buf->info = bzalloc(req.count * sizeof(struct v4l2_mmap_info));

	memset(&map, 0, sizeof(map));
	map.type = req.type;
	map.memory = req.memory;

	for (map.index = 0; map.index < req.count; ++map.index) {
		if (v4l2_ioctl(dev, VIDIOC_QUERYBUF, &map) < 0) {
			blog(LOG_ERROR, "Failed to query buffer details");
			return -1;
		}

		buf->info[map.index].length = map.length;
		buf->info[map.index].start =
			v4l2_mmap(NULL, map.length, PROT_READ | PROT_WRITE, MAP_SHARED, dev, map.m.offset);

		if (buf->info[map.index].start == MAP_FAILED) {
			blog(LOG_ERROR, "mmap for buffer failed");
			return -1;
		}
	}

	return 0;
}

int_fast32_t v4l2_destroy_mmap(struct v4l2_buffer_data *buf)
{
	for (uint_fast32_t i = 0; i < buf->count; ++i) {
		if (buf->info[i].start != MAP_FAILED && buf->info[i].start != 0)
			v4l2_munmap(buf->info[i].start, buf->info[i].length);
	}

	if (buf->count) {
		bfree(buf->info);
		buf->count = 0;
	}

	return 0;
}

int_fast32_t v4l2_set_input(int_fast32_t dev, int *input)
{
	if (!dev || !input)
		return -1;

	return (*input == -1) ? v4l2_ioctl(dev, VIDIOC_G_INPUT, input) : v4l2_ioctl(dev, VIDIOC_S_INPUT, input);
}

int_fast32_t v4l2_get_input_caps(int_fast32_t dev, int input, uint32_t *caps)
{
	if (!dev || !caps)
		return -1;

	if (input == -1) {
		if (v4l2_ioctl(dev, VIDIOC_G_INPUT, &input) < 0)
			return -1;
	}

	struct v4l2_input in;
	memset(&in, 0, sizeof(in));
	in.index = input;

	if (v4l2_ioctl(dev, VIDIOC_ENUMINPUT, &in) < 0)
		return -1;

	*caps = in.capabilities;
	return 0;
}

int_fast32_t v4l2_set_format(int_fast32_t dev, int64_t *resolution, int *pixelformat, int *bytesperline)
{
	bool set = false;
	int width, height;
	struct v4l2_format fmt;

	if (!dev || !resolution || !pixelformat || !bytesperline)
		return -1;

	/* We need to set the type in order to query the settings */
	fmt.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;

	if (v4l2_ioctl(dev, VIDIOC_G_FMT, &fmt) < 0)
		return -1;

	if (*resolution != -1) {
		v4l2_unpack_tuple(&width, &height, *resolution);
		fmt.fmt.pix.width = width;
		fmt.fmt.pix.height = height;
		set = true;
	}

	if (*pixelformat != -1) {
		fmt.fmt.pix.pixelformat = *pixelformat;
		set = true;
	}

	if (set && (v4l2_ioctl(dev, VIDIOC_S_FMT, &fmt) < 0))
		return -1;

	*resolution = v4l2_pack_tuple(fmt.fmt.pix.width, fmt.fmt.pix.height);
	*pixelformat = fmt.fmt.pix.pixelformat;
	*bytesperline = fmt.fmt.pix.bytesperline;
	return 0;
}

int_fast32_t v4l2_set_framerate(int_fast32_t dev, int64_t *framerate)
{
	bool set = false;
	int num, denom;
	struct v4l2_streamparm par;

	if (!dev || !framerate)
		return -1;

	/* We need to set the type in order to query the stream settings */
	par.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;

	if (v4l2_ioctl(dev, VIDIOC_G_PARM, &par) < 0)
		return -1;

	if (*framerate != -1) {
		v4l2_unpack_tuple(&num, &denom, *framerate);
		par.parm.capture.timeperframe.numerator = num;
		par.parm.capture.timeperframe.denominator = denom;
		set = true;
	}

	if (set && (v4l2_ioctl(dev, VIDIOC_S_PARM, &par) < 0))
		return -1;

	*framerate =
		v4l2_pack_tuple(par.parm.capture.timeperframe.numerator, par.parm.capture.timeperframe.denominator);
	return 0;
}

int_fast32_t v4l2_set_standard(int_fast32_t dev, int *standard)
{
	if (!dev || !standard)
		return -1;

	if (*standard == -1) {
		if (v4l2_ioctl(dev, VIDIOC_G_STD, standard) < 0)
			return -1;
	} else {
		if (v4l2_ioctl(dev, VIDIOC_S_STD, standard) < 0)
			return -1;
	}

	return 0;
}

int_fast32_t v4l2_enum_dv_timing(int_fast32_t dev, struct v4l2_dv_timings *dvt, int index)
{
#if !defined(VIDIOC_ENUM_DV_TIMINGS) || !defined(V4L2_IN_CAP_DV_TIMINGS)
	UNUSED_PARAMETER(dev);
	UNUSED_PARAMETER(dvt);
	UNUSED_PARAMETER(index);
	return -1;
#else
	if (!dev || !dvt)
		return -1;

	struct v4l2_enum_dv_timings iter;
	memset(&iter, 0, sizeof(iter));
	iter.index = index;

	if (v4l2_ioctl(dev, VIDIOC_ENUM_DV_TIMINGS, &iter) < 0)
		return -1;

	memcpy(dvt, &iter.timings, sizeof(struct v4l2_dv_timings));

	return 0;
#endif
}

int_fast32_t v4l2_set_dv_timing(int_fast32_t dev, int *timing)
{
	if (!dev || !timing)
		return -1;

	if (*timing == -1)
		return 0;

	struct v4l2_dv_timings dvt;

	if (v4l2_enum_dv_timing(dev, &dvt, *timing) < 0)
		return -1;

	if (v4l2_ioctl(dev, VIDIOC_S_DV_TIMINGS, &dvt) < 0)
		return -1;

	return 0;
}

void v4l2_identity_clear(struct v4l2_identity *identity)
{
	memset(identity, 0, sizeof(*identity));
	identity->index = -1;
}

bool v4l2_identity_known(const struct v4l2_identity *identity)
{
	return identity->card[0] != 0;
}

bool v4l2_identity_equal(const struct v4l2_identity *a, const struct v4l2_identity *b)
{
	return strcmp(a->card, b->card) == 0 && strcmp(a->bus_info, b->bus_info) == 0 &&
	       strcmp(a->serial, b->serial) == 0 && a->index == b->index;
}

/* The first line of a small sysfs file, without the newline */
static bool v4l2_read_sysfs(const char *path, char *buf, size_t size)
{
	FILE *f = fopen(path, "r");
	if (!f)
		return false;
	bool ok = fgets(buf, (int)size, f) != NULL;
	fclose(f);
	if (!ok)
		return false;
	buf[strcspn(buf, "\r\n")] = 0;
	return true;
}

/* The node's index and its USB device's serial number, from sysfs. Only for
 * nodes in V4L2_DEV_DIR, whose sysfs entry has the node's name. */
static void v4l2_identity_read_sysfs(const char *path, struct v4l2_identity *identity)
{
	char node[PATH_MAX], dev_dir[PATH_MAX], file[PATH_MAX + 64], text[32];

	if (!realpath(path, node) || !realpath(V4L2_DEV_DIR, dev_dir))
		return;
	size_t len = strlen(dev_dir);
	const char *name = node + len + 1;
	if (strncmp(node, dev_dir, len) != 0 || node[len] != '/' || strchr(name, '/'))
		return;

	snprintf(file, sizeof(file), V4L2_SYSFS_ROOT "/class/video4linux/%s/index", name);
	if (v4l2_read_sysfs(file, text, sizeof(text)))
		identity->index = atoi(text);

	/* The node's device is a USB interface; its parent, the USB device,
	 * has idVendor and, if the camera has one, serial. */
	char device[PATH_MAX];
	snprintf(file, sizeof(file), V4L2_SYSFS_ROOT "/class/video4linux/%s/device", name);
	if (!realpath(file, device))
		return;
	for (int level = 0; level < 4; level++) {
		snprintf(file, sizeof(file), "%s/idVendor", device);
		if (access(file, F_OK) == 0) {
			snprintf(file, sizeof(file), "%s/serial", device);
			if (!v4l2_read_sysfs(file, identity->serial, sizeof(identity->serial)))
				identity->serial[0] = 0;
			return;
		}
		char *slash = strrchr(device, '/');
		if (!slash || slash == device)
			return;
		*slash = 0;
	}
}

bool v4l2_identity_query(int_fast32_t dev, const char *path, struct v4l2_identity *identity)
{
	struct v4l2_capability cap;
	uint32_t caps;

	v4l2_identity_clear(identity);
	memset(&cap, 0, sizeof(cap));
	if (dev < 0 || v4l2_ioctl(dev, VIDIOC_QUERYCAP, &cap) != 0)
		return false;
#ifndef V4L2_CAP_DEVICE_CAPS
	caps = cap.capabilities;
#else
	caps = (cap.capabilities & V4L2_CAP_DEVICE_CAPS) ? cap.device_caps : cap.capabilities;
#endif
	if (!(caps & V4L2_CAP_VIDEO_CAPTURE) || !cap.card[0])
		return false;

	/* Bounded: the kernel's strings fill their arrays at most */
	snprintf(identity->card, sizeof(identity->card), "%.*s", (int)sizeof(cap.card), (char *)cap.card);
	snprintf(identity->bus_info, sizeof(identity->bus_info), "%.*s", (int)sizeof(cap.bus_info),
		 (char *)cap.bus_info);
	if (path)
		v4l2_identity_read_sysfs(path, identity);
	return true;
}

void v4l2_identity_load(obs_data_t *settings, struct v4l2_identity *identity)
{
	v4l2_identity_clear(identity);
	obs_data_t *saved = settings ? obs_data_get_obj(settings, V4L2_DEVICE_IDENTITY) : NULL;
	if (!saved)
		return;
	snprintf(identity->card, sizeof(identity->card), "%s", obs_data_get_string(saved, "card"));
	snprintf(identity->bus_info, sizeof(identity->bus_info), "%s", obs_data_get_string(saved, "bus_info"));
	snprintf(identity->serial, sizeof(identity->serial), "%s", obs_data_get_string(saved, "serial"));
	if (obs_data_has_user_value(saved, "index"))
		identity->index = (int)obs_data_get_int(saved, "index");
	obs_data_release(saved);
}

void v4l2_identity_save(obs_data_t *settings, const struct v4l2_identity *identity)
{
	obs_data_t *saved = obs_data_create();
	obs_data_set_string(saved, "card", identity->card);
	obs_data_set_string(saved, "bus_info", identity->bus_info);
	obs_data_set_string(saved, "serial", identity->serial);
	obs_data_set_int(saved, "index", identity->index);
	obs_data_set_obj(settings, V4L2_DEVICE_IDENTITY, saved);
	obs_data_release(saved);
}

void v4l2_identity_label(const struct v4l2_identity *identity, struct dstr *label)
{
	dstr_printf(label, "%s (%s)", identity->card, identity->bus_info);
}

static bool v4l2_bus_is_usb(const char *bus_info)
{
	return strncmp(bus_info, "usb-", 4) == 0;
}

enum v4l2_identity_match v4l2_identity_match(const struct v4l2_identity *saved, const struct v4l2_identity *camera)
{
	if (!v4l2_identity_known(saved) || !v4l2_identity_known(camera))
		return V4L2_MATCH_NONE;
	if (strcmp(saved->card, camera->card) != 0)
		return V4L2_MATCH_NONE;
	if (saved->index >= 0 && camera->index >= 0 && saved->index != camera->index)
		return V4L2_MATCH_NONE;
	if (strcmp(saved->serial, camera->serial) != 0)
		return V4L2_MATCH_NONE;

	if (strcmp(saved->bus_info, camera->bus_info) == 0)
		return V4L2_MATCH_EXACT;
	if (saved->serial[0])
		return V4L2_MATCH_SERIAL;
	/* Without a serial number, only the port tells two cameras of one
	 * model apart. A USB camera can be plugged into another port; the bus
	 * info of a device on another bus (PCI, platform) is the device. */
	if (v4l2_bus_is_usb(saved->bus_info) && v4l2_bus_is_usb(camera->bus_info))
		return V4L2_MATCH_MODEL;
	return V4L2_MATCH_NONE;
}

bool v4l2_candidate_at(const char *path, struct v4l2_candidate *candidate)
{
	if (!path || !*path || strlen(path) >= sizeof(candidate->path))
		return false;
	int fd = v4l2_open(path, O_RDWR | O_NONBLOCK | O_CLOEXEC);
	if (fd == -1)
		return false;
	bool ok = v4l2_identity_query(fd, path, &candidate->identity);
	v4l2_close(fd);
	if (ok)
		snprintf(candidate->path, sizeof(candidate->path), "%s", path);
	return ok;
}

size_t v4l2_candidates_scan(struct v4l2_candidate *list, size_t count, size_t max)
{
	DIR *dirp = opendir(V4L2_SYSFS_ROOT "/class/video4linux");
	struct dirent *dp;
	char path[sizeof(list->path)];

	if (!dirp)
		return count;
	while (count < max && (dp = readdir(dirp)) != NULL) {
		if (dp->d_name[0] == '.')
			continue;
		if (snprintf(path, sizeof(path), V4L2_DEV_DIR "/%s", dp->d_name) >= (int)sizeof(path))
			continue;
		bool listed = false;
		for (size_t i = 0; i < count && !listed; i++)
			listed = strcmp(list[i].path, path) == 0;
		if (!listed && v4l2_candidate_at(path, &list[count]))
			count++;
	}
	closedir(dirp);
	return count;
}

int v4l2_identity_pick(const struct v4l2_identity *saved, const char *saved_path, const struct v4l2_candidate *list,
		       size_t count, enum v4l2_identity_match *match)
{
	*match = V4L2_MATCH_NONE;
	for (enum v4l2_identity_match want = V4L2_MATCH_EXACT; want > V4L2_MATCH_NONE; want--) {
		int found = -1;
		size_t matches = 0;
		for (size_t i = 0; i < count; i++) {
			if (v4l2_identity_match(saved, &list[i].identity) != want)
				continue;
			/* among equals, the one at the saved node */
			if (!matches++ || (saved_path && strcmp(list[i].path, saved_path) == 0))
				found = (int)i;
		}
		if (matches == 1 || (matches > 1 && saved_path && strcmp(list[found].path, saved_path) == 0)) {
			*match = want;
			return found;
		}
		if (matches > 1)
			return -1;
	}
	return -1;
}

/* File-local macros: keep them out of the next source of a unity build. */
#undef blog
