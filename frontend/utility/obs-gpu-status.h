#pragma once

/* C ABI of rust/obs-gpu-status (src/ffi/mod.rs), linked into the frontend
 * only when ENABLE_RUST_LIBOBS is on (obs-rust/obs-studio#846). */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Record libobs' one-second average frame time; call once a second. 0 (no
 * frame rendered yet) is skipped. The last 15 samples are kept. */
void obs_gpu_status_sample(uint64_t frame_ns);

/* Start probing the system's GPUs (sysfs, nvidia-smi) on a thread, so the
 * first obs_gpu_status_report finds them probed. */
void obs_gpu_status_prefetch(void);

/* Classify `adapter` (NULL when graphics init failed) against the GPUs
 * detected on the system (probed once per process) and the median of the
 * sampled frame times; with fewer than five samples, or interval_ns == 0,
 * frame timings are ignored. Writes 0 (green), 1 (yellow) or 2 (red) to
 * `state` when it is not NULL. Returns the gpuAcceleration JSON object; free
 * it with obs_gpu_status_free. */
char *obs_gpu_status_report(const char *adapter, uint64_t interval_ns, int *state);

void obs_gpu_status_free(char *json);

/* Start sampling the render adapter's utilization once a second on a thread
 * (#1025), less often after slow samples (#1022); the report's "usage" and
 * "readout" use the last sample, or "n/a" when it is too old. Call after
 * each video reset; later calls change the adapter. `render_node` is the
 * Linux EGL render node (adapter 0 in gs_enum_adapters) or NULL;
 * `adapter_index` is the Windows obs_video_info.adapter, or -1. */
void obs_gpu_status_usage_start(const char *render_node, int adapter_index);

/* Offer one OBS log line; the ones about graphics (adapter, DRM render node,
 * EGL/GLX, video init) are quoted in the dialog. NULL is ignored. */
void obs_gpu_status_log_line(const char *line);

/* Lines for the OBS log from the sampling thread since the last call (for
 * example a source that is too slow), one per line, or NULL. Free a non-NULL
 * result with obs_gpu_status_free. */
char *obs_gpu_status_take_log(void);

#ifdef __cplusplus
}
#endif
