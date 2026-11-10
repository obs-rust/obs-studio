/* Original extracted setter and real production signal adapter over C layouts. */
#include "libobs/obs-scene-item-locked-reference-signal.c.h"
void oracle_obs_sceneitem_select(obs_sceneitem_t *item, bool lock);
#define obs_sceneitem_select oracle_obs_sceneitem_select
#include "obs-scene-item-select.c.h"
#undef obs_sceneitem_select


typedef void (*select_fn)(obs_sceneitem_t *, bool);
unsigned oracle_scene_item_select_probe(unsigned flags, unsigned requests, int64_t id, select_fn setter);

struct select_oracle_capture {
	obs_scene_t *scene;
	obs_sceneitem_t *item;
	unsigned count;
	unsigned invalid;
	bool requested;
};

static void select_oracle_signal(void *data, calldata_t *cd)
{
	struct select_oracle_capture *capture = data;
	capture->count++;
	if (calldata_ptr(cd, "scene") != capture->scene || calldata_ptr(cd, "item") != capture->item ||
	    capture->item->selected != capture->requested)
		capture->invalid++;
}

unsigned oracle_scene_item_select_probe(unsigned flags, unsigned requests, int64_t id, select_fn setter)
{
	struct obs_source source = {0};
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	scene.source = &source;
	source.context.signals = signal_handler_create();
	if (!source.context.signals)
		return 128;
	if (!signal_handler_add(source.context.signals, "void item_select(ptr scene, ptr item)") ||
	    !signal_handler_add(source.context.signals, "void item_deselect(ptr scene, ptr item)")) {
		signal_handler_destroy(source.context.signals);
		return 128;
	}
	struct select_oracle_capture capture = {.scene = &scene, .item = &item};
	signal_handler_connect(source.context.signals, "item_select", select_oracle_signal, &capture);
	signal_handler_connect(source.context.signals, "item_deselect", select_oracle_signal, &capture);
	item.parent = (flags & 1) ? &scene : NULL;
	item.selected = (flags & 2) != 0;
	item.locked = (flags & 4) != 0;
	item.removed = (flags & 8) != 0;
	item.is_group = (flags & 16) != 0;
	item.id = id;
	unsigned result = 0;
	for (unsigned step = 0; step < 16; step++) {
		bool requested = ((requests >> step) & 1) != 0;
		capture.requested = requested;
		capture.count = capture.invalid = 0;
		bool initial = item.selected;
		unsigned char before[sizeof(item)];
		memcpy(before, &item, sizeof(item));
		oracle_obs_sceneitem_select(&item, requested);
		bool c_selected = item.selected;
		unsigned c_count = capture.count;
		unsigned c_invalid = capture.invalid;
		item.selected = initial;
		if (memcmp(before, &item, sizeof(item)))
			result |= 1;
		capture.count = capture.invalid = 0;
		setter(&item, requested);
		if (item.selected != c_selected)
			result |= 2;
		if (capture.count != c_count || c_invalid || capture.invalid)
			result |= 4;
		item.selected = initial;
		if (memcmp(before, &item, sizeof(item)))
			result |= 8;
		item.selected = c_selected;
		setter(NULL, requested);
		oracle_obs_sceneitem_select(NULL, requested);
	}
	signal_handler_destroy(source.context.signals);
	return result;
}
