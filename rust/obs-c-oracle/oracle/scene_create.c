/* Callback-only original-C constructor oracle, with real source/core layouts.
 * No global libobs stubs: all factory effects are local to this fixture. */
#include "libobs/build/libobs.pch.h"
#include <stdlib.h>
#include <string.h>

struct scene_create_fixture {
	struct obs_core core;
	struct obs_canvas canvas;
	struct obs_source source;
	struct obs_scene scene;
	unsigned calls;
	bool private;
	bool valid;
	const char *name;
};
#if defined(_MSC_VER)
static __declspec(thread) struct scene_create_fixture *scene_create_current;
#else
static _Thread_local struct scene_create_fixture *scene_create_current;
#endif

static obs_scene_t *scene_create_public(obs_canvas_t *canvas, const char *id, const char *name);
static obs_scene_t *scene_create_private(obs_canvas_t *canvas, const char *id, const char *name);
obs_scene_t *oracle_obs_scene_create(const char *name);
obs_scene_t *oracle_obs_scene_create_private(const char *name);
#define obs (&scene_create_current->core)
#define create_id scene_create_public
#define create_private_id scene_create_private
#define obs_scene_create oracle_obs_scene_create
#define obs_scene_create_private oracle_obs_scene_create_private
#include "libobs/obs-scene-create.c.h"
#undef obs_scene_create_private
#undef obs_scene_create
#undef create_private_id
#undef create_id

static obs_source_t *scene_create_factory(obs_canvas_t *canvas, const char *id, const char *name, obs_data_t *settings,
					  obs_data_t *hotkeys, bool private)
{
	struct scene_create_fixture *f = scene_create_current;
	f->calls++;
	f->valid = f->valid && canvas == f->core.data.main_canvas && strcmp(id, "scene") == 0 && name == f->name &&
		   settings == NULL && hotkeys == NULL;
	f->private = private;
	/* Sentinel writes make any extra/missing factory call or reference observable. */
	f->source.context.private = private;
	f->scene.source = &f->source;
	return &f->source;
}
static obs_source_t *scene_create_public_factory(obs_canvas_t *c, const char *id, const char *n, obs_data_t *s,
						 obs_data_t *h)
{
	return scene_create_factory(c, id, n, s, h, false);
}
static obs_source_t *scene_create_private_factory(obs_canvas_t *c, const char *id, const char *n, obs_data_t *s)
{
	return scene_create_factory(c, id, n, s, NULL, true);
}
/* Original inline helper bodies, without substituting the constructors under test. */
static inline obs_scene_t *scene_create_public(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = scene_create_public_factory(canvas, id, name, NULL, NULL);
	return source->context.data;
}
static inline obs_scene_t *scene_create_private(obs_canvas_t *canvas, const char *id, const char *name)
{
	struct obs_source *source = scene_create_private_factory(canvas, id, name, NULL);
	return source->context.data;
}
#define obs_source_create_canvas scene_create_public_factory
#define obs_source_create_private_with_canvas scene_create_private_factory
#include "scene-create-fields.inc"
#undef obs_source_create_private_with_canvas
#undef obs_source_create_canvas
#undef obs

typedef obs_scene_t *(*scene_create_fn)(const char *);
bool oracle_scene_create_check(const char *name, bool private, bool null_canvas, bool null_data,
			       scene_create_fn create);
bool oracle_scene_create_check(const char *name, bool private, bool null_canvas, bool null_data, scene_create_fn create)
{
	struct scene_create_fixture *f = calloc(1, sizeof(*f));
	struct scene_create_fixture *initial = malloc(sizeof(*f));
	struct scene_create_fixture *expected = malloc(sizeof(*f));
	if (!f || !initial || !expected) {
		free(f);
		free(initial);
		free(expected);
		return false;
	}
	f->core.data.main_canvas = null_canvas ? NULL : &f->canvas;
	f->source.context.data = null_data ? NULL : &f->scene;
	f->name = name;
	f->valid = true;
	memcpy(initial, f, sizeof(*f));
	scene_create_current = f;
	obs_scene_t *original = private ? oracle_obs_scene_create_private(name) : oracle_obs_scene_create(name);
	memcpy(expected, f, sizeof(*f));
	memcpy(f, initial, sizeof(*f));
	obs_scene_t *ported =
		create ? create(name)
		       : (private ? oracle_obs_scene_create_private(name) : oracle_obs_scene_create(name));
	bool valid = original == ported && ported == (null_data ? NULL : &f->scene) && f->calls == 1 &&
		     f->private == private && f->valid && memcmp(expected, f, sizeof(*f)) == 0;
	scene_create_current = NULL;
	free(expected);
	free(initial);
	free(f);
	return valid;
}
