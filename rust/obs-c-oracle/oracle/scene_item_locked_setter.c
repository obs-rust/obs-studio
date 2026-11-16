/* Original extracted setter and real production signal adapter over C layouts. */
#include "libobs/obs-scene-item-locked-reference-signal.c.h"
bool oracle_obs_sceneitem_set_locked(obs_sceneitem_t *item, bool lock);
#define obs_sceneitem_set_locked oracle_obs_sceneitem_set_locked
#include "obs-scene-item-locked-setter.c.h"
#undef obs_sceneitem_set_locked
#include "scene_item_locked_fields.inc"

typedef bool (*locked_setter_fn)(obs_sceneitem_t *, bool);
unsigned oracle_scene_item_locked_setter_probe(unsigned flags, unsigned requests, int64_t id, locked_setter_fn setter);

struct locked_oracle_capture {
	obs_scene_t *scene;
	obs_sceneitem_t *item;
	unsigned count;
	unsigned invalid;
	bool requested;
};

static void locked_oracle_signal(void *data, calldata_t *cd)
{
	struct locked_oracle_capture *capture = data;
	capture->count++;
	if (calldata_ptr(cd, "scene") != capture->scene || calldata_ptr(cd, "item") != capture->item ||
	    calldata_bool(cd, "locked") != capture->requested || capture->item->locked != capture->requested)
		capture->invalid++;
}

unsigned oracle_scene_item_locked_setter_probe(unsigned flags, unsigned requests, int64_t id, locked_setter_fn setter)
{
	struct obs_source source = {0};
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	scene.source = &source;
	source.context.signals = signal_handler_create();
	if (!source.context.signals)
		return 128;
	if (!signal_handler_add(source.context.signals, "void item_locked(ptr scene, ptr item, bool locked)")) {
		signal_handler_destroy(source.context.signals);
		return 128;
	}
	struct locked_oracle_capture capture = {.scene = &scene, .item = &item};
	signal_handler_connect(source.context.signals, "item_locked", locked_oracle_signal, &capture);
	item.parent = (flags & 1) ? &scene : NULL;
	item.locked = (flags & 2) != 0;
	item.selected = (flags & 4) != 0;
	item.removed = (flags & 8) != 0;
	item.is_group = (flags & 16) != 0;
	item.id = id;
	unsigned result = 0;
	for (unsigned step = 0; step < 16; step++) {
		bool requested = ((requests >> step) & 1) != 0;
		capture.requested = requested;
		capture.count = capture.invalid = 0;
		bool initial = item.locked;
		unsigned char before[sizeof(item)];
		memcpy(before, &item, sizeof(item));
		bool c_result = oracle_obs_sceneitem_set_locked(&item, requested);
		bool c_locked = item.locked;
		unsigned c_count = capture.count;
		unsigned c_invalid = capture.invalid;
		item.locked = initial;
		if (memcmp(before, &item, sizeof(item)))
			result |= 1;
		capture.count = capture.invalid = 0;
		bool rust_result = setter(&item, requested);
		if (rust_result != c_result || item.locked != c_locked)
			result |= 2;
		if (capture.count != c_count || c_invalid || capture.invalid)
			result |= 4;
		item.locked = initial;
		if (memcmp(before, &item, sizeof(item)))
			result |= 8;
		item.locked = c_locked;
		if (setter(NULL, requested) != oracle_obs_sceneitem_set_locked(NULL, requested))
			result |= 16;
	}
	signal_handler_destroy(source.context.signals);
	return result;
}
