/* Original scene and source lifetime code over real private C layouts.
 * Source operations are scoped to this TLS fixture, never global stand-ins. */
#include "libobs/build/libobs.pch.h"
#include <stdlib.h>
#include <string.h>

struct scene_ref_fixture {
	struct obs_core core;
	struct obs_scene scene;
	struct obs_source source;
	struct obs_source alias;
	struct obs_weak_source control;
	struct obs_core *active_core;
	unsigned acquisitions;
	unsigned releases;
	unsigned destroys;
	bool identity;
	long refs_at_destroy;
	long weak_at_destroy;
};
#if defined(_MSC_VER)
static __declspec(thread) struct scene_ref_fixture *scene_ref_current;
#else
static _Thread_local struct scene_ref_fixture *scene_ref_current;
#endif

static void scene_ref_destroy(obs_source_t *source)
{
	struct scene_ref_fixture *f = scene_ref_current;
	f->destroys++;
	f->identity = source == &f->source;
	f->refs_at_destroy = f->control.ref.refs;
	f->weak_at_destroy = f->control.ref.weak_refs;
}
static obs_source_t *scene_ref_original_get(obs_source_t *source);
static void scene_ref_original_release(obs_source_t *source);
static void scene_ref_original_weak_release(obs_weak_source_t *weak);
static obs_source_t *scene_ref_original_weak_get(obs_weak_source_t *weak);
#define obs scene_ref_current->active_core
#define get_weak(source) ((obs_weak_source_t *)(source)->context.control)
#define obs_source_destroy scene_ref_destroy
#define obs_source_get_ref scene_ref_original_get
#define obs_source_release scene_ref_original_release
#define obs_weak_source_release scene_ref_original_weak_release
#define obs_weak_source_get_source scene_ref_original_weak_get
#include "scene-ref-source.inc"
#undef obs_weak_source_get_source
#undef obs_weak_source_release
#undef obs_source_release
#undef obs_source_get_ref
#undef obs_source_destroy
#undef get_weak
#undef obs

bool oracle_atomic_source_acquire(obs_source_t *source);
void oracle_atomic_source_release(obs_source_t *source);
static obs_source_t *scene_ref_acquire(obs_source_t *source)
{
	if (!scene_ref_current)
		return oracle_atomic_source_acquire(source) ? source : NULL;
	scene_ref_current->acquisitions++;
	return scene_ref_original_get(source);
}
static void scene_ref_release(obs_source_t *source)
{
	if (!scene_ref_current) {
		oracle_atomic_source_release(source);
		return;
	}
	scene_ref_current->releases++;
	scene_ref_original_release(source);
}
obs_scene_t *oracle_obs_scene_get_ref(obs_scene_t *scene);
void oracle_obs_scene_release(obs_scene_t *scene);
#define obs_source_get_ref scene_ref_acquire
#define obs_source_release scene_ref_release
#define obs_scene_get_ref oracle_obs_scene_get_ref
#define obs_scene_release oracle_obs_scene_release
#include "libobs/obs-scene-ref.c.h"
#undef obs_scene_release
#undef obs_scene_get_ref
#include "scene-ref-fields.inc"
#undef obs_source_release
#undef obs_source_get_ref

typedef obs_scene_t *(*scene_ref_get_fn)(obs_scene_t *);
typedef void (*scene_ref_release_fn)(obs_scene_t *);
/* operation: acquire, release, balanced acquisitions plus final release.
 * flags: NULL scene, NULL source, NULL promoted source, alias promotion, no core.
 * Exclude signed overflow and release without ownership, as in original C.
 * A held external weak owner retains the control across final destruction. */
bool oracle_scene_ref_probe(unsigned operation, unsigned flags, unsigned acquisitions, long refs, scene_ref_get_fn get,
			    scene_ref_release_fn release);
bool oracle_scene_ref_probe(unsigned operation, unsigned flags, unsigned acquisitions, long refs, scene_ref_get_fn get,
			    scene_ref_release_fn release)
{
	struct scene_ref_fixture *f = calloc(1, sizeof(*f));
	struct scene_ref_fixture *initial = malloc(sizeof(*f));
	struct scene_ref_fixture *expected = malloc(sizeof(*f));
	if (!f || !initial || !expected) {
		free(f);
		free(initial);
		free(expected);
		return false;
	}
	f->active_core = (flags & 16) ? NULL : &f->core;
	f->scene.source = (flags & 2) ? NULL : &f->source;
	f->scene.id_counter = 73;
	f->source.context.control = (struct obs_weak_object *)&f->control;
	f->source.context.data = &f->scene;
	f->source.context.name = "scene ref source sentinel";
	f->control.source = (flags & 4) ? NULL : ((flags & 8) ? &f->alias : &f->source);
	f->control.ref.refs = refs;
	f->control.ref.weak_refs = 1;
	memcpy(initial, f, sizeof(*f));
	scene_ref_current = f;
	bool valid = true;
	obs_scene_t *results[2] = {NULL, NULL};
	for (unsigned ported = 0; ported < 2; ++ported) {
		memcpy(f, initial, sizeof(*f));
		obs_scene_t *scene = (flags & 1) ? NULL : &f->scene;
		scene_ref_get_fn acquire = ported ? get : oracle_obs_scene_get_ref;
		scene_ref_release_fn drop = ported ? release : oracle_obs_scene_release;
		if (operation == 0) {
			results[ported] = acquire(scene);
		} else if (operation == 1) {
			drop(scene);
		} else {
			for (unsigned i = 0; i < acquisitions; ++i)
				valid = (acquire(scene) == scene) && valid;
			for (unsigned i = 0; i < acquisitions; ++i)
				drop(scene);
			drop(scene);
			/* Fixture keeps expired source/scene allocated just as the Tier 2
			 * synchronous destruction callback does until its task returns. */
			results[ported] = acquire(scene);
		}
		if (!ported)
			memcpy(expected, f, sizeof(*f));
	}
	valid = valid && results[0] == results[1] && memcmp(expected, f, sizeof(*f)) == 0;
	if (operation == 0) {
		bool success = !(flags & 7) && refs >= 0;
		valid = valid && (results[1] == (success ? &f->scene : NULL));
	}
	if (operation == 2 && !(flags & 3))
		valid = valid && f->destroys == 1 && f->identity && f->refs_at_destroy == -1 &&
			f->weak_at_destroy == 1 && f->control.ref.weak_refs == 0 && f->control.ref.refs == -1 &&
			results[1] == NULL;
	scene_ref_current = NULL;
	free(expected);
	free(initial);
	free(f);
	return valid;
}
