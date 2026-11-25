#pragma once
#ifdef __cplusplus
extern "C" {
#endif
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct AudioState AudioState;
typedef struct CaptureState CaptureState;
typedef void (*FilterBlock)(void *, float *const *, const float *const *, size_t, size_t);

AudioState *vst3_filter_audio_create(size_t, size_t, uint64_t);
void vst3_filter_audio_destroy(AudioState *);
CaptureState *vst3_filter_capture_create(const AudioState *);
void vst3_filter_capture_destroy(CaptureState *);
void vst3_filter_capture_reset_timestamp(const CaptureState *);
void vst3_filter_capture_push(const CaptureState *, const float *const *, uint32_t, uint64_t);
bool vst3_filter_audio_process(AudioState *, const float *const *, uint32_t, uint64_t, bool, void *, FilterBlock,
			       float **, uint32_t *, uint64_t *);

typedef struct FilterState FilterState;
FilterState *vst3_filter_state_transcode(const uint8_t *, size_t, bool);
const uint8_t *vst3_filter_state_bytes(const FilterState *, size_t *);
void vst3_filter_state_destroy(FilterState *);
#ifdef __cplusplus
}
#endif

struct RouteState;
struct RouteLookup;
extern "C" {
RouteState *vst3_filter_route_create();
void vst3_filter_route_destroy(RouteState *state);
void *vst3_filter_route_select(RouteState *state, const char *name);
void *vst3_filter_route_detach(RouteState *state);
RouteLookup *vst3_filter_route_lookup(RouteState *state, uint64_t now);
const char *vst3_filter_route_name(const RouteLookup *lookup);
bool vst3_filter_route_attach(RouteState *state, const RouteLookup *lookup, void *source);
void vst3_filter_route_lookup_destroy(RouteLookup *lookup);
}

struct CatalogBuilder;
struct CatalogState;
enum ClassChange : uint8_t { ClearClass, RetainClass, InitialClass, SwapClass };
using TextPair = void (*)(void *, const char *, const char *);
extern "C" {
CatalogBuilder *vst3_filter_catalog_begin();
void vst3_filter_catalog_add(CatalogBuilder *builder, const char *id, const char *path, const char *name,
			     const char *pluginName, bool effect);
CatalogState *vst3_filter_catalog_finish(CatalogBuilder *builder);
void vst3_filter_catalog_destroy(CatalogState *state);
void vst3_filter_catalog_choices(const CatalogState *state, void *context, TextPair add);
uint8_t vst3_filter_class_change(const CatalogState *state, const char *current, bool hasProcessor,
				 const char *requested, const char *cachedPath, const char *cachedName, void *context,
				 TextPair setLocation);
}
