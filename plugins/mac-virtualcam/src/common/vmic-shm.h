#pragma once

/* OBS Virtual Microphone on macOS (obs-rust/obs-studio#3184): the shared
 * memory ring between the mac-virtualcam output (writer) and the HAL
 * plug-in (reader). One writer and one reader; no locks, so the plug-in's
 * real-time IO thread never waits on OBS. Plain C11 so the cmocka test runs
 * on every host. */

#include <stdatomic.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define VMIC_SHM_NAME "/obs-virtual-microphone"
#define VMIC_SHM_MAGIC 0x4f42564du /* "OBVM" */
#define VMIC_SHM_VERSION 1u

struct vmic_shm {
	uint32_t magic;
	uint32_t version;
	uint32_t rate;
	uint32_t channels;
	uint32_t capacity; /* frames */
	uint32_t reserved;
	_Atomic uint64_t write_frame;
	_Atomic uint64_t read_frame;
	/* Host time (mach_absolute_time) of the newest written frame, so the
	 * reader can line the audio up with the camera frames. */
	_Atomic uint64_t write_host_time;
	float samples[];
};

/* Bytes to map for a ring of `capacity` frames. */
static inline size_t vmic_shm_size(uint32_t channels, uint32_t capacity)
{
	return sizeof(struct vmic_shm) + (size_t)channels * capacity * sizeof(float);
}

static inline void vmic_shm_init(struct vmic_shm *shm, uint32_t rate, uint32_t channels, uint32_t capacity)
{
	(void)shm;
	(void)rate;
	(void)channels;
	(void)capacity;
}

/* Whether a mapping holds a ring this reader understands. */
static inline bool vmic_shm_valid(const struct vmic_shm *shm, size_t mapped)
{
	(void)shm;
	(void)mapped;
	return false;
}

/* Appends `frames` interleaved frames; a packet that does not fit is dropped
 * whole (returns false), so channels never shift. */
static inline bool vmic_shm_write(struct vmic_shm *shm, const float *data, uint32_t frames, uint64_t host_time)
{
	(void)shm;
	(void)data;
	(void)frames;
	(void)host_time;
	return false;
}

/* Fills `frames` frames, padding what the ring lacks with silence. Returns
 * the frames that came from the ring. */
static inline uint32_t vmic_shm_read(struct vmic_shm *shm, float *out, uint32_t frames)
{
	(void)shm;
	(void)out;
	(void)frames;
	return 0;
}
