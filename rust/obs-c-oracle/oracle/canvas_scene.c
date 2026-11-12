/* Original wrappers and real-header adapters, with functional traced host calls.
 * These doubles exist only in the oracle test archive, never in libobs. */
#include "libobs/build/libobs.pch.h"
#include <string.h>

obs_scene_t *oracle_obs_canvas_scene_create(obs_canvas_t *canvas, const char *name);
void oracle_obs_canvas_scene_remove(obs_scene_t *scene);
#define obs_canvas_scene_create oracle_obs_canvas_scene_create
#define obs_canvas_scene_remove oracle_obs_canvas_scene_remove
#include "libobs/obs-canvas-scene.c.h"
#undef obs_canvas_scene_create
#undef obs_canvas_scene_remove
#include "canvas_scene_fields.inc"

struct canvas_scene_fixture {
	struct obs_canvas canvas;
	struct obs_weak_canvas weak;
	struct obs_source source;
	struct obs_source alternate;
	struct obs_scene scene;
	unsigned creates;
	unsigned removes;
	bool arguments_match;
	const char *name;
	bool null_canvas;
};
#if defined(_MSC_VER)
static __declspec(thread) struct canvas_scene_fixture *canvas_scene_current;
#else
static _Thread_local struct canvas_scene_fixture *canvas_scene_current;
#endif

obs_source_t *obs_source_create_canvas(obs_canvas_t *canvas, const char *id, const char *name, obs_data_t *settings,
				       obs_data_t *hotkey_data)
{
	struct canvas_scene_fixture *f = canvas_scene_current;
	f->creates++;
	f->arguments_match = canvas == (f->null_canvas ? NULL : &f->canvas) && strcmp(id, "scene") == 0 &&
			     name == f->name && settings == NULL && hotkey_data == NULL;
	f->source.canvas = f->null_canvas ? NULL : &f->weak;
	return &f->source;
}

bool oracle_canvas_move_remove(obs_source_t *source);
void obs_canvas_remove_source(obs_source_t *source)
{
	if (oracle_canvas_move_remove(source))
		return;
	struct canvas_scene_fixture *f = canvas_scene_current;
	f->removes++;
	f->arguments_match = f->arguments_match && source == f->scene.source;
	source->canvas = NULL;
}

typedef obs_scene_t *(*canvas_scene_create_fn)(obs_canvas_t *, const char *);
typedef void (*canvas_scene_remove_fn)(obs_scene_t *);
bool oracle_canvas_scene_check(const char *name, bool null_canvas, bool null_data, bool alternate, unsigned repeats,
			       canvas_scene_create_fn create, canvas_scene_remove_fn remove);
bool oracle_canvas_scene_check(const char *name, bool null_canvas, bool null_data, bool alternate, unsigned repeats,
			       canvas_scene_create_fn create, canvas_scene_remove_fn remove)
{
	struct canvas_scene_fixture f;
	struct canvas_scene_fixture initial;
	struct canvas_scene_fixture expected;
	memset(&f, 0, sizeof(f));
	f.name = name;
	f.null_canvas = null_canvas;
	f.arguments_match = true;
	f.weak.canvas = &f.canvas;
	f.source.context.data = null_data ? NULL : &f.scene;
	f.scene.source = alternate ? &f.alternate : &f.source;
	f.alternate.canvas = &f.weak;
	memcpy(&initial, &f, sizeof(f));
	canvas_scene_current = &f;
	obs_scene_t *original = oracle_obs_canvas_scene_create(null_canvas ? NULL : &f.canvas, name);
	for (unsigned i = 0; i < repeats; ++i)
		oracle_obs_canvas_scene_remove(&f.scene);
	memcpy(&expected, &f, sizeof(f));
	memcpy(&f, &initial, sizeof(f));
	obs_scene_t *ported = create(null_canvas ? NULL : &f.canvas, name);
	for (unsigned i = 0; i < repeats; ++i)
		remove(&f.scene);
	bool valid = original == ported && ported == (null_data ? NULL : &f.scene) && f.creates == 1 &&
		     f.removes == repeats && f.arguments_match && memcmp(&expected, &f, sizeof(f)) == 0 &&
		     obs_canvas_scene_context_data(&f.source) == f.source.context.data &&
		     obs_canvas_scene_source(&f.scene) == f.scene.source;
	canvas_scene_current = NULL;
	return valid;
}
