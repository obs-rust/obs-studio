/* Tier 3: the original source weak-handle acquisition over the real private
 * layout, beside the Rust export, with the real control adapter. The C addref
 * lives in obs-source.c.h, outside the oracle, so this file defines it under
 * its real name: the original body, plus a per-thread record of its calls. */
#include <stdlib.h>
#include <string.h>
#define obs_source_get_weak_source oracle_obs_source_get_weak_source
#include "libobs/build/libobs.pch.h"
#include "source-get-weak.inc"
#undef obs_source_get_weak_source
#include "source_get_weak_fields.inc"

static _Thread_local unsigned source_get_weak_addrefs;
static _Thread_local obs_weak_source_t *source_get_weak_last_addref;

void obs_weak_source_addref(obs_weak_source_t *weak)
{
	source_get_weak_addrefs++;
	source_get_weak_last_addref = weak;
	if (!weak)
		return;

	obs_weak_ref_addref(&weak->ref);
}

typedef obs_weak_source_t *(*source_get_weak_entry)(obs_source_t *);

struct source_get_weak_outcome {
	bool returned_control;
	unsigned addrefs;
	long weak_refs;
};

/* flags: bit 0 source present, bit 1 control present. noise fills every other
 * byte of the source and control. Each side runs count acquisitions on its own
 * fixture; returns, addref calls and arguments, counts and bytes must match. */
static bool source_get_weak_run(source_get_weak_entry entry, unsigned flags, uint32_t noise, long refs, long weak_refs,
				unsigned count, struct source_get_weak_outcome *out)
{
	struct obs_source *source = malloc(sizeof(*source));
	struct obs_weak_source *control = malloc(sizeof(*control));
	if (!source || !control) {
		free(source);
		free(control);
		return false;
	}
	memset(source, (int)(noise & 0xff), sizeof(*source));
	memset(control, (int)((noise >> 8) & 0xff), sizeof(*control));
	control->ref.refs = refs;
	control->ref.weak_refs = weak_refs;
	control->source = source;
	source->context.control = (flags & 2) ? (struct obs_weak_object *)control : NULL;
	unsigned char source_before[sizeof(*source)];
	memcpy(source_before, source, sizeof(*source));
	obs_weak_source_t *expected = (flags & 3) == 3 ? control : NULL;
	bool ok = true;
	memset(out, 0, sizeof(*out));
	out->returned_control = true;
	source_get_weak_addrefs = 0;
	for (unsigned i = 0; i < count; i++) {
		source_get_weak_last_addref = (obs_weak_source_t *)source;
		unsigned before = source_get_weak_addrefs;
		obs_weak_source_t *weak = entry((flags & 1) ? source : NULL);
		out->returned_control = out->returned_control && weak == expected;
		ok = ok && (weak == NULL || weak == control);
		if (source_get_weak_addrefs != before)
			ok = ok && source_get_weak_last_addref == expected;
		ok = ok && memcmp(source_before, source, sizeof(*source)) == 0 && control->ref.refs == refs &&
		     control->source == source;
	}
	out->addrefs = source_get_weak_addrefs;
	out->weak_refs = control->ref.weak_refs;
	free(control);
	free(source);
	return ok;
}

bool oracle_source_get_weak_check(source_get_weak_entry entry, unsigned flags, uint32_t noise, long refs,
				  long weak_refs, unsigned count, bool *returned_control, unsigned *addrefs,
				  long *final_weak_refs);
bool oracle_source_get_weak_check(source_get_weak_entry entry, unsigned flags, uint32_t noise, long refs,
				  long weak_refs, unsigned count, bool *returned_control, unsigned *addrefs,
				  long *final_weak_refs)
{
	struct source_get_weak_outcome rust, original;
	bool ok = source_get_weak_run(entry, flags, noise, refs, weak_refs, count, &rust);
	ok = source_get_weak_run(oracle_obs_source_get_weak_source, flags, noise, refs, weak_refs, count, &original) &&
	     ok;
	*returned_control = original.returned_control;
	*addrefs = original.addrefs;
	*final_weak_refs = original.weak_refs;
	return ok && rust.returned_control == original.returned_control && rust.addrefs == original.addrefs &&
	       rust.weak_refs == original.weak_refs;
}
