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
	shm->rate = rate;
	shm->channels = channels;
	shm->capacity = capacity;
	shm->reserved = 0;
	atomic_store(&shm->write_frame, 0);
	atomic_store(&shm->read_frame, 0);
	atomic_store(&shm->write_host_time, 0);
	shm->version = VMIC_SHM_VERSION;
	atomic_thread_fence(memory_order_release);
	shm->magic = VMIC_SHM_MAGIC;
}

/* Whether a mapping holds a ring this reader understands. */
static inline bool vmic_shm_valid(const struct vmic_shm *shm, size_t mapped)
{
	if (mapped < sizeof(struct vmic_shm)) {
		return false;
	}
	return shm->magic == VMIC_SHM_MAGIC && shm->version == VMIC_SHM_VERSION && shm->channels > 0 &&
	       shm->capacity > 0 && vmic_shm_size(shm->channels, shm->capacity) <= mapped;
}

/* Appends `frames` interleaved frames; a packet that does not fit is dropped
 * whole (returns false), so channels never shift. */
static inline bool vmic_shm_write(struct vmic_shm *shm, const float *data, uint32_t frames, uint64_t host_time)
{
	uint64_t w = atomic_load_explicit(&shm->write_frame, memory_order_relaxed);
	uint64_t r = atomic_load_explicit(&shm->read_frame, memory_order_acquire);
	if (frames > shm->capacity - (w - r)) {
		return false;
	}
	for (uint32_t i = 0; i < frames; i++) {
		size_t slot = (size_t)((w + i) % shm->capacity) * shm->channels;
		memcpy(&shm->samples[slot], &data[(size_t)i * shm->channels], shm->channels * sizeof(float));
	}
	atomic_store_explicit(&shm->write_host_time, host_time, memory_order_relaxed);
	atomic_store_explicit(&shm->write_frame, w + frames, memory_order_release);
	return true;
}

/* Fills `frames` frames, padding what the ring lacks with silence. Returns
 * the frames that came from the ring. */
static inline uint32_t vmic_shm_read(struct vmic_shm *shm, float *out, uint32_t frames)
{
	uint64_t r = atomic_load_explicit(&shm->read_frame, memory_order_relaxed);
	uint64_t w = atomic_load_explicit(&shm->write_frame, memory_order_acquire);
	uint64_t avail = w - r;
	uint32_t n = avail < frames ? (uint32_t)avail : frames;
	for (uint32_t i = 0; i < n; i++) {
		size_t slot = (size_t)((r + i) % shm->capacity) * shm->channels;
		memcpy(&out[(size_t)i * shm->channels], &shm->samples[slot], shm->channels * sizeof(float));
	}
	memset(&out[(size_t)n * shm->channels], 0, (size_t)(frames - n) * shm->channels * sizeof(float));
	atomic_store_explicit(&shm->read_frame, r + n, memory_order_release);
	return n;
}
