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

#pragma once

#include <linux/videodev2.h>
#include <libv4l2.h>
#include <inttypes.h>

#include <obs-module.h>
#include <media-io/video-io.h>
#include <util/dstr.h>

#ifdef __cplusplus
extern "C" {
#endif

#define PACK64(a, b) (((uint64_t)a << 32) | ((uint64_t)b & 0xffffffff))

/**
 * Data structure for mapped buffers
 */
struct v4l2_mmap_info {
	/** length of the mapped buffer */
	size_t length;
	/** start address of the mapped buffer */
	void *start;
};

/**
 * Data structure for buffer info
 */
struct v4l2_buffer_data {
	/** number of mapped buffers */
	uint_fast32_t count;
	/** memory info for mapped buffers */
	struct v4l2_mmap_info *info;
};

/**
 * Convert v4l2 pixel format to obs video format
 *
 * @param format v4l2 format id
 *
 * @return obs video_format id
 */
static inline enum video_format v4l2_to_obs_video_format(uint_fast32_t format)
{
	switch (format) {
	case V4L2_PIX_FMT_YVYU:
		return VIDEO_FORMAT_YVYU;
	case V4L2_PIX_FMT_YUYV:
		return VIDEO_FORMAT_YUY2;
	case V4L2_PIX_FMT_UYVY:
		return VIDEO_FORMAT_UYVY;
	case V4L2_PIX_FMT_NV12:
		return VIDEO_FORMAT_NV12;
	case V4L2_PIX_FMT_YUV420:
		return VIDEO_FORMAT_I420;
	case V4L2_PIX_FMT_YVU420:
		return VIDEO_FORMAT_I420;
#ifdef V4L2_PIX_FMT_XBGR32
	case V4L2_PIX_FMT_XBGR32:
		return VIDEO_FORMAT_BGRX;
#endif
#ifdef V4L2_PIX_FMT_ABGR32
	case V4L2_PIX_FMT_ABGR32:
		return VIDEO_FORMAT_BGRA;
#endif
	case V4L2_PIX_FMT_BGR24:
		return VIDEO_FORMAT_BGR3;
	default:
		return VIDEO_FORMAT_NONE;
	}
}

/**
 * Fixed framesizes for devices that don't support enumerating discrete values.
 *
 * The framesizes in this array are packed, the width encoded in the high word
 * and the height in the low word.
 * The array is terminated with a zero.
 */
static const int64_t v4l2_framesizes[] = {
	/* 4:3 */
	PACK64(160, 120), PACK64(320, 240), PACK64(480, 320), PACK64(640, 480), PACK64(800, 600), PACK64(1024, 768),
	PACK64(1280, 960), PACK64(1440, 1050), PACK64(1440, 1080), PACK64(1600, 1200),

	/* 16:9 */
	PACK64(640, 360), PACK64(960, 540), PACK64(1280, 720), PACK64(1600, 900), PACK64(1920, 1080),
	PACK64(1920, 1200), PACK64(2560, 1440), PACK64(3840, 2160),

	/* 21:9 */
	PACK64(2560, 1080), PACK64(3440, 1440), PACK64(5120, 2160),

	/* tv */
	PACK64(432, 520), PACK64(480, 320), PACK64(480, 530), PACK64(486, 440), PACK64(576, 310), PACK64(576, 520),
	PACK64(576, 570), PACK64(720, 576), PACK64(1024, 576),

	0};

/**
 * Fixed framerates for devices that don't support enumerating discrete values.
 *
 * The framerates in this array are packed, the numerator encoded in the high
 * word and the denominator in the low word.
 * The array is terminated with a zero.
 */
static const int64_t v4l2_framerates[] = {PACK64(1, 60),
					  PACK64(1, 50),
					  PACK64(1, 30),
					  PACK64(1, 25),
					  PACK64(1, 20),
					  PACK64(1, 15),
					  PACK64(1, 10),
					  PACK64(1, 5),

					  0};

/**
 * Pack two integer values into one
 *
 * Obviously the input integers have to be truncated in order to fit into
 * one. The effective 16bits left are still enough to handle resolutions and
 * framerates just fine.
 *
 * @param a integer one
 * @param b integer two
 *
 * @return the packed integer
 */
static inline int64_t v4l2_pack_tuple(int32_t a, int32_t b)
{
	return PACK64(a, b);
}

/**
 * Unpack two integer values from one
 *
 * @see v4l2_pack_tuple
 *
 * @param a pointer to integer a
 * @param b pointer to integer b
 * @param packed the packed integer
 */
static void v4l2_unpack_tuple(int32_t *a, int32_t *b, int64_t packed)
{
	// Since we changed from 32 to 64 bits, handle old values too.
	if ((packed & 0xffffffff00000000) == 0) {
		*a = (int32_t)(packed >> 16);
		*b = (int32_t)(packed & 0xffff);
	} else {
		*a = (int32_t)(packed >> 32);
		*b = (int32_t)(packed & 0xffffffff);
	}
}

/**
 * Delay before the capture thread reopens a failed device
 *
 * 1 s before the first attempt, doubling to at most 10 s, so a device that
 * stays broken is not opened in a busy loop.
 *
 * @param attempt the attempts made since the device last delivered a buffer
 *
 * @return the delay in milliseconds
 */
static inline uint32_t v4l2_reopen_delay_ms(uint32_t attempt)
{
	const uint32_t first_ms = 1000;
	const uint32_t max_ms = 10000;

	if (attempt >= 4)
		return max_ms;
	uint32_t delay = first_ms << attempt;
	return delay < max_ms ? delay : max_ms;
}

/**
 * Start the video capture on the device.
 *
 * This enqueues the memory mapped buffers and instructs the device to start
 * the video stream.
 *
 * @param dev handle for the v4l2 device
 * @param buf buffer data
 *
 * @return negative on failure
 */
int_fast32_t v4l2_start_capture(int_fast32_t dev, struct v4l2_buffer_data *buf);

/**
 * Stop the video capture on the device.
 *
 * @param dev handle for the v4l2 device
 *
 * @return negative on failure
 */
int_fast32_t v4l2_stop_capture(int_fast32_t dev);

/**
 * Resets video capture on the device.
 *
 * This runs stop and start capture again. Stop dequeues the buffers and start
 * enqueues the memory mapped buffers and instructs the device to start
 * the video stream.
 *
 * @param dev handle for the v4l2 device
 * @param buf buffer data
 *
 * @return negative on failure
 */
int_fast32_t v4l2_reset_capture(int_fast32_t dev, struct v4l2_buffer_data *buf);

#ifdef _DEBUG
/**
 * Query the status of all buffers.
 * Only used for debug purposes.
 *
 * @param dev handle for the v4l2 device
 * @param buf_data buffer data
 *
 * @return negative on failure
 */
int_fast32_t v4l2_query_all_buffers(int_fast32_t dev, struct v4l2_buffer_data *buf_data);
#endif

/**
 * Create memory mapping for buffers
 *
 * This tries to map at least 2, preferably 4, buffers to application memory.
 *
 * @param dev handle for the v4l2 device
 * @param buf buffer data
 *
 * @return negative on failure
 */
int_fast32_t v4l2_create_mmap(int_fast32_t dev, struct v4l2_buffer_data *buf);

/**
 * Destroy the memory mapping for buffers
 *
 * @param buf buffer data
 *
 * @return negative on failure
 */
int_fast32_t v4l2_destroy_mmap(struct v4l2_buffer_data *buf);

/**
 * Set the video input on the device.
 *
 * If the action succeeds input is set to the currently selected input.
 *
 * @param dev handle for the v4l2 device
 * @param input index of the input or -1 to leave it as is
 *
 * @return negative on failure
 */
int_fast32_t v4l2_set_input(int_fast32_t dev, int *input);

/**
 * Get capabilities for an input.
 *
 * @param dev handle for the v4l2 device
 * @param input index of the input or -1 to use the currently selected
 * @param caps capabilities for the input
 *
 * @return negative on failure
 */
int_fast32_t v4l2_get_input_caps(int_fast32_t dev, int input, uint32_t *caps);

/**
 * Set the video format on the device.
 *
 * If the action succeeds resolution, pixelformat and bytesperline are set
 * to the used values.
 *
 * @param dev handle for the v4l2 device
 * @param resolution packed value of the resolution or -1 to leave as is
 * @param pixelformat index of the pixelformat or -1 to leave as is
 * @param bytesperline this will be set accordingly on success
 *
 * @return negative on failure
 */
int_fast32_t v4l2_set_format(int_fast32_t dev, int64_t *resolution, int *pixelformat, int *bytesperline);

/**
 * Set the framerate on the device.
 *
 * If the action succeeds framerate is set to the used value.
 *
 * @param dev handle to the v4l2 device
 * @param framerate packed value of the framerate or -1 to leave as is
 *
 * @return negative on failure
 */
int_fast32_t v4l2_set_framerate(int_fast32_t dev, int64_t *framerate);

/**
 * Set a video standard on the device.
 *
 * If the action succeeds standard is set to the used video standard id.
 *
 * @param dev handle to the v4l2 device
 * @param standard id of the standard to use or -1 to leave as is
 *
 * @return negative on failure
 */
int_fast32_t v4l2_set_standard(int_fast32_t dev, int *standard);

/**
 * Get the dv timing for an input with a specified index
 *
 * @param dev handle to the v4l2 device
 * @param dvt pointer to the timing structure to fill
 * @param index index of the dv timing to fetch
 *
 * @return negative on failure
 */
int_fast32_t v4l2_enum_dv_timing(int_fast32_t dev, struct v4l2_dv_timings *dvt, int index);
/**
 * Set a dv timing on the device
 *
 * Currently standard will not be changed on success or error.
 *
 * @param dev handle to the v4l2 device
 * @param timing index of the timing to use or -1 to leave as is
 *
 * @return negative on failure
 */
int_fast32_t v4l2_set_dv_timing(int_fast32_t dev, int *timing);

/* Where the device nodes and sysfs are; tests point them at fakes */
#ifndef V4L2_DEV_DIR
#define V4L2_DEV_DIR "/dev"
#endif
#ifndef V4L2_SYSFS_ROOT
#define V4L2_SYSFS_ROOT "/sys"
#endif

/** Settings: the camera the source captures, next to its node (device_id) */
#define V4L2_DEVICE_IDENTITY "device_identity"

/**
 * What tells one camera from another, whatever /dev/videoN it gets: the
 * card name and bus info of VIDIOC_QUERYCAP, the USB serial number if the
 * camera has one, and which of the device's nodes it is.
 */
struct v4l2_identity {
	char card[33];
	/* "usb-0000:00:14.0-1" (the USB port), "platform:vivid-000"... */
	char bus_info[33];
	/* the USB device's serial number, empty if it has none */
	char serial[128];
	/* the node's index on its device, -1 if not known */
	int index;
};

/** How a camera compares with the one a source saved */
enum v4l2_identity_match {
	/* another camera */
	V4L2_MATCH_NONE,
	/* the same model, without a serial number, on another USB port: the
	 * camera moved to another port, or its twin */
	V4L2_MATCH_MODEL,
	/* the same serial number on another USB port: the camera, moved */
	V4L2_MATCH_SERIAL,
	/* the camera, where it was */
	V4L2_MATCH_EXACT,
};

/** A capture node and the camera behind it */
struct v4l2_candidate {
	char path[256];
	struct v4l2_identity identity;
};

/** Set an identity to unknown */
void v4l2_identity_clear(struct v4l2_identity *identity);

/** Whether an identity names a camera */
bool v4l2_identity_known(const struct v4l2_identity *identity);

/** Whether two identities are the same */
bool v4l2_identity_equal(const struct v4l2_identity *a, const struct v4l2_identity *b);

/**
 * Get the identity of the capture device open at a node.
 *
 * @param dev handle to the v4l2 device
 * @param path the node it was opened at (sysfs is looked up for nodes in
 *             V4L2_DEV_DIR)
 * @param identity set to the identity
 *
 * @return false if the device cannot tell or does not capture video
 */
bool v4l2_identity_query(int_fast32_t dev, const char *path, struct v4l2_identity *identity);

/** Read the identity saved in the settings, unknown if there is none */
void v4l2_identity_load(obs_data_t *settings, struct v4l2_identity *identity);

/** Save an identity in the settings */
void v4l2_identity_save(obs_data_t *settings, const struct v4l2_identity *identity);

/** "card (bus_info)", as in the device list */
void v4l2_identity_label(const struct v4l2_identity *identity, struct dstr *label);

/** Compare a camera with the saved one */
enum v4l2_identity_match v4l2_identity_match(const struct v4l2_identity *saved, const struct v4l2_identity *camera);

/**
 * Open a node and get the identity of its camera.
 *
 * @return false if there is no capture device at the node
 */
bool v4l2_candidate_at(const char *path, struct v4l2_candidate *candidate);

/**
 * Add the capture nodes in V4L2_DEV_DIR that are not in the list yet.
 *
 * @return the number of candidates now in the list
 */
size_t v4l2_candidates_scan(struct v4l2_candidate *list, size_t count, size_t max);

/**
 * Pick the saved camera among the candidates: the camera where it was;
 * else the one with its serial number; else, for a USB camera without a
 * serial number, the only camera of its model. When two candidates fit
 * equally, the one at the saved node, else none.
 *
 * @return the candidate's index, -1 if the camera is not there
 */
int v4l2_identity_pick(const struct v4l2_identity *saved, const char *saved_path, const struct v4l2_candidate *list,
		       size_t count, enum v4l2_identity_match *match);

#ifdef __cplusplus
}
#endif
