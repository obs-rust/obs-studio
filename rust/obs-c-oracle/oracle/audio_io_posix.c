/* Test-only oracle for libobs/media-io/audio-io.c: the real C file compiled
 * with its exported and static entry points renamed oracle_* so the ported
 * Rust unit can be diffed against it. Also supplies, for cargo-test links
 * only, the libobs symbols audio-io.c calls that no oracle source provides:
 * the profiler hooks (no-ops), obs_get_profiler_name_store (NULL),
 * audio_resampler_* (create fails NULL like an absent backend), and the two
 * platform-nix.c time helpers (real POSIX implementations, same semantics,
 * under private names so they do not collide with the Rust shims).
 * pthread/os_event/os_set_thread_name come from the real threading-posix.c
 * and blog from the real base-variadic.c already compiled here. */
#define _GNU_SOURCE
#define audio_resampler_create oracle_aio_resampler_create
#define audio_resampler_destroy oracle_aio_resampler_destroy
#define audio_resampler_resample oracle_aio_resampler_resample
#include <stddef.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>
#include <string.h>
#include <stdarg.h>
#include <stdio.h>

/* obs-util exports the Rust profiler (util/profiler.c port) under the libobs
 * names, so the no-op hooks get private names here to avoid a duplicate. */
#define profile_start oracle_aio_profile_start
#define profile_end oracle_aio_profile_end
#define profile_reenable_thread oracle_aio_profile_reenable_thread
/* audio-io.c calls os_gettime_ns. Keep that clock under a private name
 * so it does not collide with the Rust shim, or with the zero stub
 * macOS and Windows cargo tests link. */
#define os_gettime_ns oracle_aio_os_gettime_ns
#define os_sleepto_ns oracle_aio_os_sleepto_ns
/* The Rust platform-nix shim owns this name. audio-io keeps its own clock. */
#define os_sleepto_ns_fast oracle_aio_os_sleepto_ns_fast

#define audio_output_open oracle_audio_output_open
#define audio_output_close oracle_audio_output_close
#define audio_output_connect oracle_audio_output_connect
#define audio_output_disconnect oracle_audio_output_disconnect
#define audio_output_active oracle_audio_output_active
#define audio_output_get_block_size oracle_audio_output_get_block_size
#define audio_output_get_planes oracle_audio_output_get_planes
#define audio_output_get_channels oracle_audio_output_get_channels
#define audio_output_get_sample_rate oracle_audio_output_get_sample_rate
#define audio_output_get_info oracle_audio_output_get_info
#define input_and_output oracle_input_and_output
#define audio_thread oracle_audio_thread
#define audio_input_init oracle_audio_input_init
#define audio_get_input_idx oracle_audio_get_input_idx
#define do_audio_output oracle_do_audio_output
#define clamp_audio_output oracle_clamp_audio_output
#define resample_audio_output oracle_resample_audio_output
#define audio_input_free oracle_audio_input_free
#define valid_audio_params oracle_valid_audio_params

#include "media-io/audio-io.c.h"

/* ------------------------------------------------------------------ */
/* stubs/impls for libobs symbols absent from the oracle link          */

uint64_t os_gettime_ns(void)
{
	struct timespec ts;
	clock_gettime(CLOCK_MONOTONIC, &ts);
	return ((uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec);
}

bool os_sleepto_ns_fast(uint64_t time_target)
{
	uint64_t current = os_gettime_ns();
	if (time_target < current)
		return false;

	do {
		uint64_t remain_us = (time_target - current + 999) / 1000;
		unsigned long us = remain_us >= 1000000 ? 999999 : remain_us;
		struct timespec ts = {(time_t)(us / 1000000), (long)((us % 1000000) * 1000)};
		nanosleep(&ts, NULL);
		current = os_gettime_ns();
	} while (time_target > current);

	return true;
}

profiler_name_store_t *obs_get_profiler_name_store(void)
{
	return NULL;
}

const char *profile_store_name(profiler_name_store_t *store, const char *format, ...)
{
	(void)store;
	(void)format;
	return NULL;
}

void profile_start(const char *name)
{
	(void)name;
}
void profile_end(const char *name)
{
	(void)name;
}
void profile_reenable_thread(void) {}

/* Deterministic resampler so the parity suite exercises the real resample
 * path — swapped data pointers and a rate-scaled frame count (the NULL
 * stub made every resampled connect fail). Nearest-sample conversion by
 * the src:dst rate ratio; the same code drives oracle and port, so
 * outputs match bit-for-bit. A carried sub-frame remainder models
 * latency: a non-integer rate ratio yields a nonzero ts_offset.
 *
 * Rate-only test double: it copies source planes using the destination
 * plane count/stride, so a format/speaker-layout change would read the
 * input with the wrong layout (interleaved-to-planar stereo has no
 * input[1]). Such conversions are rejected at create time instead of
 * pretending full conversion support.
 *
 * The struct completes the opaque `struct audio_resampler` from
 * audio-resampler.h, so audio_resampler_t * / struct audio_resampler *
 * are the same type (GCC 14 errors on incompatible-pointer-types). */
struct audio_resampler {
	struct resample_info dst;
	struct resample_info src;
	uint8_t *buf[MAX_AV_PLANES];
	size_t buf_bytes[MAX_AV_PLANES];
	size_t planes;
	size_t frame_bytes;
	/* carried sub-frame remainder, like a real resampler's: keeps the
	 * deterministic out-frame count honest on non-integer rate ratios
	 * and feeds the reported ts_offset */
	uint64_t residue;
};

audio_resampler_t *audio_resampler_create(const struct resample_info *dst, const struct resample_info *src)
{
	struct audio_resampler *r = bzalloc(sizeof(*r));
	if (!r)
		return NULL;

	if (src->format != dst->format || src->speakers != dst->speakers) {
		bfree(r);
		return NULL;
	}

	r->dst = *dst;
	r->src = *src;
	r->planes = is_audio_planar(dst->format) ? get_audio_channels(dst->speakers) : 1;
	r->frame_bytes = (is_audio_planar(dst->format) ? 1 : get_audio_channels(dst->speakers)) *
			 get_audio_bytes_per_channel(dst->format);
	if (!r->planes || !r->frame_bytes || !dst->samples_per_sec || !src->samples_per_sec) {
		bfree(r);
		return NULL;
	}
	return r;
}

void audio_resampler_destroy(audio_resampler_t *resampler)
{
	struct audio_resampler *r = resampler;
	if (!r)
		return;
	for (size_t p = 0; p < MAX_AV_PLANES; p++)
		bfree(r->buf[p]);
	bfree(r);
}

bool audio_resampler_resample(audio_resampler_t *resampler, uint8_t *output[], uint32_t *out_frames,
			      uint64_t *ts_offset, const uint8_t *const input[], uint32_t in_frames)
{
	struct audio_resampler *r = resampler;
	uint64_t acc = r->residue + (uint64_t)in_frames * r->dst.samples_per_sec;
	uint32_t out = (uint32_t)(acc / r->src.samples_per_sec);
	r->residue = acc % r->src.samples_per_sec;

	for (size_t p = 0; p < r->planes; p++) {
		size_t need = (size_t)out * r->frame_bytes;
		if (r->buf_bytes[p] < need) {
			bfree(r->buf[p]);
			r->buf[p] = bzalloc(need);
			if (!r->buf[p])
				return false;
			r->buf_bytes[p] = need;
		}
		for (uint32_t i = 0; i < out; i++) {
			uint32_t src_frame = (uint32_t)((uint64_t)i * r->src.samples_per_sec / r->dst.samples_per_sec);
			memcpy(r->buf[p] + (size_t)i * r->frame_bytes, input[p] + (size_t)src_frame * r->frame_bytes,
			       r->frame_bytes);
		}
		output[p] = r->buf[p];
	}
	*out_frames = out;
	/* residue/src_rate of a dst frame, in ns — nonzero on non-integer
	 * ratios, which is what the separate ts-offset parity case wants */
	*ts_offset = r->residue * 1000000000ull / r->src.samples_per_sec / r->dst.samples_per_sec;
	return true;
}

/* ------------------------------------------------------------------ */
/* oracle-only extras: drive one tick + expose internal state          */

/* Test-only exports called from Rust over FFI. */
void oracle_audio_tick(audio_t *audio, uint64_t audio_time, uint64_t prev_time);
int oracle_audio_output_open_no_thread(audio_t **audio, struct audio_output_info *info);
void oracle_audio_output_close_no_thread(audio_t *audio);
size_t oracle_audio_mix_input_count(audio_t *audio, size_t mix_idx);
bool oracle_audio_mix_input_resampled(audio_t *audio, size_t mix_idx, size_t input_idx);
const struct audio_convert_info *oracle_audio_mix_input_conversion(audio_t *audio, size_t mix_idx, size_t input_idx);

void oracle_audio_tick(audio_t *audio, uint64_t audio_time, uint64_t prev_time)
{
	input_and_output(audio, audio_time, prev_time);
}

/* No-worker fixture: an audio_output with no audio thread, so a test owns
 * every tick via oracle_audio_tick — no worker races the driver. The
 * recursive input_mutex is still real: connect/disconnect lock it. */
int oracle_audio_output_open_no_thread(audio_t **audio, struct audio_output_info *info)
{
	struct audio_output *out;
	bool planar;

	if (!valid_audio_params(info))
		return AUDIO_OUTPUT_INVALIDPARAM;

	planar = is_audio_planar(info->format);
	out = bzalloc(sizeof(struct audio_output));
	if (!out)
		return AUDIO_OUTPUT_FAIL;

	memcpy(&out->info, info, sizeof(struct audio_output_info));
	out->channels = get_audio_channels(info->speakers);
	out->planes = planar ? out->channels : 1;
	out->input_cb = info->input_callback;
	out->input_param = info->input_param;
	out->block_size = (planar ? 1 : out->channels) * get_audio_bytes_per_channel(info->format);

	if (pthread_mutex_init_recursive(&out->input_mutex) != 0) {
		bfree(out);
		return AUDIO_OUTPUT_FAIL;
	}

	/* initialized stays false: no stop_event or thread exists; the close
	 * below destroys the mutex itself then frees the inputs like the C. */
	*audio = out;
	return AUDIO_OUTPUT_SUCCESS;
}

void oracle_audio_output_close_no_thread(audio_t *audio)
{
	if (!audio)
		return;
	pthread_mutex_destroy(&audio->input_mutex);
	audio_output_close(audio); /* initialized==false: frees inputs + bfree */
}

size_t oracle_audio_mix_input_count(audio_t *audio, size_t mix_idx)
{
	return audio->mixes[mix_idx].inputs.num;
}

bool oracle_audio_mix_input_resampled(audio_t *audio, size_t mix_idx, size_t input_idx)
{
	return audio->mixes[mix_idx].inputs.array[input_idx].resampler != NULL;
}

const struct audio_convert_info *oracle_audio_mix_input_conversion(audio_t *audio, size_t mix_idx, size_t input_idx)
{
	return &audio->mixes[mix_idx].inputs.array[input_idx].conversion;
}
