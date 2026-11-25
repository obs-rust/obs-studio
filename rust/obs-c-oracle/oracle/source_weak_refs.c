/* Tier 3: the original source weak reference counting, renamed, over
 * real-header controls, beside the Rust exports and the real adapter. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
/* Renamed before the PCH, so obs.h declares the renamed originals too. */
#define obs_weak_source_addref oracle_obs_weak_source_addref
#define obs_weak_source_release oracle_obs_weak_source_release
#include "libobs/build/libobs.pch.h"
#include "source-weak-refs.inc"
#undef obs_weak_source_addref
#undef obs_weak_source_release
#include "source_weak_refs_fields.inc"

/* A bzalloc'd control (what the final release bfrees) over a source; noise
 * fills the source bytes so a stray write shows. */
struct source_weak_refs_fixture {
	struct obs_source *source;
	struct obs_weak_source *weak;
	unsigned char source_before[sizeof(struct obs_source)];
};

void *oracle_source_weak_refs_new(long refs, long weak_refs, uint8_t noise);
void *oracle_source_weak_refs_new(long refs, long weak_refs, uint8_t noise)
{
	struct source_weak_refs_fixture *f = calloc(1, sizeof(*f));
	if (!f)
		return NULL;
	f->source = malloc(sizeof(*f->source));
	if (!f->source) {
		free(f);
		return NULL;
	}
	memset(f->source, noise, sizeof(*f->source));
	f->weak = bzalloc(sizeof(*f->weak));
	f->weak->ref.refs = refs;
	f->weak->ref.weak_refs = weak_refs;
	f->weak->source = f->source;
	f->source->context.control = (struct obs_weak_object *)f->weak;
	memcpy(f->source_before, f->source, sizeof(*f->source));
	return f;
}

void *oracle_source_weak_refs_weak(void *handle);
void *oracle_source_weak_refs_weak(void *handle)
{
	return ((struct source_weak_refs_fixture *)handle)->weak;
}

/* Strong and weak counts of a control that has not been freed yet. */
void oracle_source_weak_refs_counts(void *handle, long *counts);
void oracle_source_weak_refs_counts(void *handle, long *counts)
{
	const struct source_weak_refs_fixture *f = handle;
	counts[0] = f->weak->ref.refs;
	counts[1] = f->weak->ref.weak_refs;
}

/* The source bytes and the control's source pointer are untouched, and the
 * adapter returns the control's own weak count. */
bool oracle_source_weak_refs_intact(void *handle);
bool oracle_source_weak_refs_intact(void *handle)
{
	const struct source_weak_refs_fixture *f = handle;
	return memcmp(f->source_before, f->source, sizeof(*f->source)) == 0 && f->weak->source == f->source &&
	       obs_weak_source_weak_refs_field(f->weak) == &f->weak->ref.weak_refs;
}

void oracle_source_weak_refs_free(void *handle, bool released);
void oracle_source_weak_refs_free(void *handle, bool released)
{
	struct source_weak_refs_fixture *f = handle;
	if (!released)
		bfree(f->weak);
	free(f->source);
	free(f);
}

void oracle_source_weak_refs_layout(size_t *layout);
void oracle_source_weak_refs_layout(size_t *layout)
{
	layout[0] = sizeof(long);
	layout[1] = _Alignof(long);
}
