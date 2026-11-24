#pragma once

#include <stddef.h>
#include <stdint.h>

struct Vst3RustHost;
struct Vst3RustState;
struct Vst3RustEditor;
namespace Steinberg {
class FUnknown;
struct ViewRect;
namespace Linux {
class IRunLoop;
}
} // namespace Steinberg

extern "C" {
Vst3RustHost *vst3_rust_host_create(const char *path, const char *class_id, double sample_rate, int32_t max_frames,
				    uint64_t arrangement, void *context);
void vst3_rust_host_destroy(Vst3RustHost *host);
const char *vst3_rust_host_name(const Vst3RustHost *host);
size_t vst3_rust_host_sidechain_channels(const Vst3RustHost *host);
Vst3RustEditor *vst3_rust_editor_create(const Vst3RustHost *host, const char *platform);
void vst3_rust_editor_destroy(Vst3RustEditor *editor);
bool vst3_rust_editor_can_resize(const Vst3RustEditor *editor);
bool vst3_rust_editor_size(const Vst3RustEditor *editor, Steinberg::ViewRect *rect);
int32_t vst3_rust_editor_constrain(const Vst3RustEditor *editor, Steinberg::ViewRect *rect);
int32_t vst3_rust_editor_on_size(const Vst3RustEditor *editor, Steinberg::ViewRect *rect);
void vst3_rust_editor_scale(const Vst3RustEditor *editor, float scale);
bool vst3_rust_editor_attach(const Vst3RustEditor *editor, void *parent, const char *platform, void *context,
			     int32_t (*resize)(void *, const Steinberg::ViewRect *), Steinberg::FUnknown *hostContext);
void vst3_rust_editor_detach(const Vst3RustEditor *editor);

void *vst3_rust_run_loop_create(uintptr_t context, void (*watchFd)(uintptr_t, uint64_t, int),
				void (*watchTimer)(uintptr_t, uint64_t, int), void (*unwatch)(uintptr_t, uint64_t));
Steinberg::Linux::IRunLoop *vst3_rust_run_loop_interface(const void *);
void vst3_rust_run_loop_dispatch(const void *, uint64_t, int);
void vst3_rust_run_loop_destroy(void *);
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
