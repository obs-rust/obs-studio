/* Original migration body, real private layouts, and traced dependency seams. */
#include "libobs/build/libobs.pch.h"
#include <string.h>

void oracle_obs_canvas_move_scene(obs_scene_t *scene, obs_canvas_t *dst);
#define obs_canvas_move_scene oracle_obs_canvas_move_scene
#include "libobs/obs-canvas-move.c.h"
#undef obs_canvas_move_scene
#include "canvas_move_fields.inc"

enum { CANVAS_MOVE_ITEMS = 8, CANVAS_MOVE_SOURCES = CANVAS_MOVE_ITEMS + 2 };
struct canvas_move_fixture {
	struct obs_canvas canvases[2];
	struct obs_weak_canvas weak[2];
	struct obs_source sources[CANVAS_MOVE_SOURCES];
	struct obs_scene scene;
	struct obs_scene_item items[CANVAS_MOVE_ITEMS];
	bool present[CANVAS_MOVE_ITEMS];
	bool observer[CANVAS_MOVE_ITEMS];
	unsigned events[128];
	unsigned count;
	unsigned length;
	bool valid;
};
#if defined(_MSC_VER)
static __declspec(thread) struct canvas_move_fixture *canvas_move_current;
#else
static _Thread_local struct canvas_move_fixture *canvas_move_current;
#endif

static void canvas_move_event(unsigned operation, obs_source_t *source, unsigned canvas)
{
	struct canvas_move_fixture *f = canvas_move_current;
	if (f->count >= 128) {
		f->valid = false;
		return;
	}
	unsigned index = source ? (unsigned)(source - f->sources) : 0;
	f->events[f->count++] = operation * 256 + index * 2 + canvas;
}

/* Shared remove_source test seam: canvas_scene.c dispatches here only while
 * this thread owns a migration fixture. Other lifecycle fixtures are untouched. */
bool oracle_canvas_move_remove(obs_source_t *source);
bool oracle_canvas_move_remove(obs_source_t *source)
{
	struct canvas_move_fixture *f = canvas_move_current;
	if (!f)
		return false;
	if (!source || !source->canvas) {
		f->valid = false;
		return true;
	}
	struct obs_weak_canvas *old = source->canvas;
	unsigned canvas = (unsigned)(old - f->weak);
	canvas_move_event(1, source, canvas);
	source->canvas = NULL;
	if (strcmp(source->info.id, "group") == 0) {
		for (unsigned i = 0; i < f->length; ++i) {
			if (f->items[i].source != source)
				continue;
			f->observer[i] = false;
			if (f->scene.source->canvas == old)
				f->present[i] = false;
		}
	}
	return true;
}

void obs_canvas_insert_source(obs_canvas_t *canvas, obs_source_t *source)
{
	struct canvas_move_fixture *f = canvas_move_current;
	unsigned index = (unsigned)(canvas - f->canvases);
	if (source->canvas || index > 1)
		f->valid = false;
	source->canvas = &f->weak[index];
	canvas_move_event(2, source, index);
}

void obs_scene_enum_items(obs_scene_t *scene, bool (*callback)(obs_scene_t *, obs_sceneitem_t *, void *), void *param)
{
	struct canvas_move_fixture *f = canvas_move_current;
	f->valid = f->valid && scene == &f->scene;
	canvas_move_event(3, NULL, 0);
	for (unsigned i = 0; i < f->length; ++i) {
		if (f->present[i] && !callback(scene, &f->items[i], param)) {
			f->valid = false;
			break;
		}
	}
}

typedef void (*canvas_move_fn)(obs_scene_t *, obs_canvas_t *);
bool oracle_canvas_move_check(unsigned groups, unsigned length, bool reverse, bool same, unsigned flags,
			      canvas_move_fn ported);
bool oracle_canvas_move_check(unsigned groups, unsigned length, bool reverse, bool same, unsigned flags,
			      canvas_move_fn ported)
{
	if (length > CANVAS_MOVE_ITEMS)
		return false;
	struct canvas_move_fixture f;
	memset(&f, 0, sizeof(f));
	f.valid = true;
	f.length = length;
	f.canvases[0].flags = flags & 1 ? SCENE_REF : 0;
	f.canvases[1].flags = flags & 2 ? SCENE_REF : 0;
	for (unsigned i = 0; i < 2; ++i)
		f.weak[i].canvas = &f.canvases[i];
	for (unsigned i = 0; i < CANVAS_MOVE_SOURCES; ++i) {
		f.sources[i].canvas = &f.weak[0];
		f.sources[i].info.id = i >= 2 && (groups & (1u << (i - 2))) ? "group" : "scene";
	}
	f.scene.source = &f.sources[0];
	for (unsigned i = 0; i < length; ++i) {
		unsigned index = reverse ? length - i - 1 : i;
		f.items[i].source = &f.sources[index + 2];
		f.present[i] = true;
		f.observer[i] = strcmp(f.items[i].source->info.id, "group") == 0;
	}
	struct canvas_move_fixture initial;
	struct canvas_move_fixture expected;
	memcpy(&initial, &f, sizeof(f));
	canvas_move_current = &f;
	oracle_obs_canvas_move_scene(&f.scene, &f.canvases[same ? 0 : 1]);
	memcpy(&expected, &f, sizeof(f));
	memcpy(&f, &initial, sizeof(f));
	ported(&f.scene, &f.canvases[same ? 0 : 1]);
	bool valid = f.valid && memcmp(&f, &expected, sizeof(f)) == 0 &&
		     obs_canvas_move_scene_source(&f.scene) == f.scene.source;
	for (unsigned i = 0; i < length; ++i)
		valid = valid && obs_canvas_move_item_source(&f.items[i]) == f.items[i].source;
	canvas_move_current = NULL;
	return valid;
}
