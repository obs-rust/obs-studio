/* Original extracted body over an instrumented lifetime/lock boundary.
 * The body does not inspect scene storage. The same boundary drives the Rust shim. */
#include "libobs/build/libobs.pch.h"

struct atomic_fixture {
	uint64_t refs;
	uint64_t *out;
	size_t count;
	bool release_caller;
	bool removed;
	uint64_t data;
};
static void atomic_record(struct atomic_fixture *f, uint64_t event)
{
	f->out[++f->count] = event;
	f->out[++f->count] = f->refs;
}
static obs_scene_t *atomic_acquire(obs_scene_t *scene)
{
	struct atomic_fixture *f = (struct atomic_fixture *)scene;
	atomic_record(f, 1);
	if (!f->refs)
		return NULL;
	f->refs++;
	return scene;
}
static void atomic_release(obs_scene_t *scene)
{
	struct atomic_fixture *f = (struct atomic_fixture *)scene;
	f->refs--;
	atomic_record(f, 5);
}
void oracle_atomic_full_lock(obs_scene_t *scene);
void oracle_atomic_full_unlock(obs_scene_t *scene);
void oracle_atomic_full_lock(obs_scene_t *scene)
{
	atomic_record((struct atomic_fixture *)scene, 2);
}
void oracle_atomic_full_unlock(obs_scene_t *scene)
{
	atomic_record((struct atomic_fixture *)scene, 4);
}
#define obs_scene_atomic_full_lock oracle_atomic_full_lock
#define obs_scene_atomic_full_unlock oracle_atomic_full_unlock
#define obs_scene_get_ref atomic_acquire
#define obs_scene_release atomic_release
#define obs_scene_atomic_update oracle_obs_scene_atomic_update
void oracle_obs_scene_atomic_update(obs_scene_t *, obs_scene_atomic_update_func, void *);
#include "libobs/obs-scene-atomic-update.c.h"
#undef obs_scene_atomic_update
#undef obs_scene_release
#undef obs_scene_get_ref
#undef obs_scene_atomic_full_unlock
#undef obs_scene_atomic_full_lock

/* Functional lifetime/lock boundary for the Rust ABI under test. These use
 * the same instrumented scene as the original C body, rather than link stubs. */
obs_scene_t *obs_scene_get_ref(obs_scene_t *scene)
{
	return atomic_acquire(scene);
}
void obs_scene_release(obs_scene_t *scene)
{
	atomic_release(scene);
}
void obs_scene_atomic_full_lock(obs_scene_t *scene);
void obs_scene_atomic_full_unlock(obs_scene_t *scene);
void obs_scene_atomic_full_lock(obs_scene_t *scene)
{
	oracle_atomic_full_lock(scene);
}
void obs_scene_atomic_full_unlock(obs_scene_t *scene)
{
	oracle_atomic_full_unlock(scene);
}

static void atomic_callback(void *data, obs_scene_t *scene)
{
	struct atomic_fixture *f = (struct atomic_fixture *)scene;
	atomic_record(f, data == &f->data ? 3 : 99);
	f->out[++f->count] = *(uint64_t *)data;
	if (f->release_caller)
		f->refs--;
	atomic_record(f, 6);
}
/* out has room for 16 uint64_t values, length first. */
void oracle_scene_atomic_trace(unsigned bits, unsigned refs, uint64_t data, uint64_t *out);
void oracle_scene_atomic_trace_with(unsigned bits, unsigned refs, uint64_t data, uint64_t *out,
				    void (*update)(obs_scene_t *, obs_scene_atomic_update_func, void *));
void oracle_scene_atomic_trace_with(unsigned bits, unsigned refs, uint64_t data, uint64_t *out,
				    void (*update)(obs_scene_t *, obs_scene_atomic_update_func, void *))
{
	struct atomic_fixture f = {
		.refs = refs,
		.out = out,
		.release_caller = (bits & 2) != 0,
		.removed = (bits & 4) != 0,
		.data = data,
	};
	update((bits & 1) ? NULL : (obs_scene_t *)&f, atomic_callback, &f.data);
	out[0] = f.count;
}

void oracle_scene_atomic_trace(unsigned bits, unsigned refs, uint64_t data, uint64_t *out)
{
	oracle_scene_atomic_trace_with(bits, refs, data, out, oracle_obs_scene_atomic_update);
}
