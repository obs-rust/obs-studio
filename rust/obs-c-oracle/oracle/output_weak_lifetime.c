/* Exact original functions, real layouts and real atomic primitives. Stack
 * probes retain a weak owner; final frees use allocator-backed controls. */
#include <stddef.h>
#include <string.h>
#include "obs-internal.h"

void oracle_obs_weak_output_addref(obs_weak_output_t *weak);
void oracle_obs_weak_output_release(obs_weak_output_t *weak);
obs_weak_output_t *oracle_obs_output_get_weak_output(obs_output_t *output);
obs_output_t *oracle_obs_weak_output_get_output(obs_weak_output_t *weak);
bool oracle_obs_weak_output_references_output(obs_weak_output_t *weak, obs_output_t *output);
#define obs_weak_output_addref oracle_obs_weak_output_addref
#define obs_weak_output_release oracle_obs_weak_output_release
#define obs_output_get_weak_output oracle_obs_output_get_weak_output
#define obs_weak_output_get_output oracle_obs_weak_output_get_output
#define obs_weak_output_references_output oracle_obs_weak_output_references_output
#include "obs-output-weak-lifetime.c.h"
#undef obs_weak_output_addref
#undef obs_weak_output_release
#undef obs_output_get_weak_output
#undef obs_weak_output_get_output
#undef obs_weak_output_references_output

#include "output-weak-lifetime-fields.inc"

void oracle_output_weak_lifetime_layout(size_t *layout);
void oracle_output_weak_lifetime_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_weak_ref);
	layout[1] = _Alignof(struct obs_weak_ref);
	layout[2] = offsetof(struct obs_weak_ref, refs);
	layout[3] = offsetof(struct obs_weak_ref, weak_refs);
	layout[4] = sizeof(struct obs_weak_output);
	layout[5] = _Alignof(struct obs_weak_output);
	layout[6] = offsetof(struct obs_weak_output, ref);
	layout[7] = offsetof(struct obs_weak_output, output);
}

/* Flags independently select null weak, null output argument, null stored
 * target, alias argument and null output control. Avoid C signed overflow
 * and releasing a stack control's final weak owner. */
bool oracle_output_weak_lifetime_probe(unsigned operation, long refs, long weak_refs, unsigned flags,
				       void (*addref)(obs_weak_output_t *), void (*release)(obs_weak_output_t *),
				       obs_weak_output_t *(*get_weak)(obs_output_t *),
				       obs_output_t *(*promote)(obs_weak_output_t *),
				       bool (*references)(obs_weak_output_t *, obs_output_t *));
bool oracle_output_weak_lifetime_probe(unsigned operation, long refs, long weak_refs, unsigned flags,
				       void (*addref)(obs_weak_output_t *), void (*release)(obs_weak_output_t *),
				       obs_weak_output_t *(*get_weak)(obs_output_t *),
				       obs_output_t *(*promote)(obs_weak_output_t *),
				       bool (*references)(obs_weak_output_t *, obs_output_t *))
{
	struct obs_output output = {0};
	struct obs_output alias = {0};
	struct obs_output before;
	struct obs_output alias_before;
	struct obs_weak_output control = {0};
	output.context.control = (flags & 16) ? NULL : (struct obs_weak_object *)&control;
	control.output = (flags & 4) ? NULL : &output;
	memcpy(&before, &output, sizeof(output));
	memcpy(&alias_before, &alias, sizeof(alias));
	struct obs_weak_output expected;
	void *results[2] = {NULL, NULL};
	bool identities[2] = {false, false};
	for (unsigned ported = 0; ported < 2; ++ported) {
		control.ref.refs = refs;
		control.ref.weak_refs = weak_refs;
		obs_weak_output_t *weak = (flags & 1) ? NULL : &control;
		obs_output_t *target = (flags & 2) ? NULL : ((flags & 8) ? &alias : &output);
		switch (operation) {
		case 0:
			(ported ? addref : oracle_obs_weak_output_addref)(weak);
			break;
		case 1:
			(ported ? release : oracle_obs_weak_output_release)(weak);
			break;
		case 2:
			results[ported] = (ported ? get_weak : oracle_obs_output_get_weak_output)(target);
			break;
		case 3:
			results[ported] = (ported ? promote : oracle_obs_weak_output_get_output)(weak);
			break;
		case 4:
			identities[ported] =
				(ported ? references : oracle_obs_weak_output_references_output)(weak, target);
			break;
		}
		if (!ported)
			memcpy(&expected, &control, sizeof(control));
	}
	return results[0] == results[1] && identities[0] == identities[1] &&
	       memcmp(&expected, &control, sizeof(control)) == 0 && memcmp(&before, &output, sizeof(output)) == 0 &&
	       memcmp(&alias_before, &alias, sizeof(alias)) == 0;
}

void oracle_output_weak_lifetime_final_release(void (*release)(obs_weak_output_t *));
void oracle_output_weak_lifetime_final_release(void (*release)(obs_weak_output_t *))
{
	struct obs_weak_output *original = bzalloc(sizeof(*original));
	struct obs_weak_output *ported = bzalloc(sizeof(*ported));
	original->ref.refs = -1;
	ported->ref.refs = -1;
	oracle_obs_weak_output_release(original);
	release(ported);
}
