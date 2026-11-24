#pragma once

#include <stddef.h>
#include <stdint.h>

struct Vst3RustHost;
struct Vst3RustState;

extern "C" {
Vst3RustHost *vst3_rust_host_create(const char *path, const char *class_id, double sample_rate, int32_t max_frames,
				    uint64_t arrangement, void *context);
void vst3_rust_host_destroy(Vst3RustHost *host);
const char *vst3_rust_host_name(const Vst3RustHost *host);
size_t vst3_rust_host_sidechain_channels(const Vst3RustHost *host);
void *vst3_rust_host_controller(const Vst3RustHost *host);
void vst3_rust_host_deactivate(const Vst3RustHost *host);
void vst3_rust_host_set_processing(const Vst3RustHost *host, bool processing);
bool vst3_rust_host_process(const Vst3RustHost *host, float *const *main, size_t channels,
			    const float *const *sidechain, size_t sidechain_frames, size_t frames, int64_t timestamp,
			    bool *gui_pending);
void vst3_rust_host_drain_gui(const Vst3RustHost *host);

Vst3RustState *vst3_rust_host_save(const Vst3RustHost *host);
const uint8_t *vst3_rust_state_data(const Vst3RustState *state, bool controller, size_t *size);
void vst3_rust_state_destroy(Vst3RustState *state);
bool vst3_rust_host_restore(const Vst3RustHost *host, const uint8_t *component, size_t component_size,
			    const uint8_t *controller, size_t controller_size);

int32_t vst3_rust_host_app_name(uint16_t *name);
int32_t vst3_rust_host_app_create(char *cid, char *iid, void **object);
int32_t vst3_rust_host_app_supports(const char *iid);
}
