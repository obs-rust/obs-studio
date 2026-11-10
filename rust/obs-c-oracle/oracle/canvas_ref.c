/* Exact original bodies over the real private header; production operations
 * enter only through callbacks. LONG_MAX strong upgrades are C signed UB. */
#include <stddef.h>
#include <string.h>
#include "obs-internal.h"

void oracle_obs_weak_canvas_addref(obs_weak_canvas_t *weak);
void oracle_obs_weak_canvas_release(obs_weak_canvas_t *weak);
obs_canvas_t *oracle_obs_canvas_get_ref(obs_canvas_t *canvas);
obs_weak_canvas_t *oracle_obs_canvas_get_weak_canvas(obs_canvas_t *canvas);
obs_canvas_t *oracle_obs_weak_canvas_get_canvas(obs_weak_canvas_t *weak);

#define obs_weak_canvas_addref oracle_obs_weak_canvas_addref
#define obs_weak_canvas_release oracle_obs_weak_canvas_release
#define obs_canvas_get_ref oracle_obs_canvas_get_ref
#define obs_canvas_get_weak_canvas oracle_obs_canvas_get_weak_canvas
#define obs_weak_canvas_get_canvas oracle_obs_weak_canvas_get_canvas
#include "obs-canvas-ref.c.h"
#undef obs_weak_canvas_addref
#undef obs_weak_canvas_release
#undef obs_canvas_get_ref
#undef obs_canvas_get_weak_canvas
#undef obs_weak_canvas_get_canvas

#include "canvas_ref_fields.inc"

void oracle_canvas_ref_layout(size_t *layout);
void oracle_canvas_ref_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_weak_ref);
	layout[1] = _Alignof(struct obs_weak_ref);
	layout[2] = offsetof(struct obs_weak_ref, refs);
	layout[3] = offsetof(struct obs_weak_ref, weak_refs);
	layout[4] = sizeof(struct obs_weak_canvas);
	layout[5] = _Alignof(struct obs_weak_canvas);
	layout[6] = offsetof(struct obs_weak_canvas, ref);
	layout[7] = offsetof(struct obs_weak_canvas, canvas);
}

bool oracle_canvas_ref_probe(unsigned operation, long strong, long weak_count, bool null_handle,
			     void (*addref)(obs_weak_canvas_t *), void (*release)(obs_weak_canvas_t *),
			     obs_canvas_t *(*get_ref)(obs_canvas_t *), obs_weak_canvas_t *(*get_weak)(obs_canvas_t *),
			     obs_canvas_t *(*upgrade)(obs_weak_canvas_t *));
bool oracle_canvas_ref_probe(unsigned operation, long strong, long weak_count, bool null_handle,
			     void (*addref)(obs_weak_canvas_t *), void (*release)(obs_weak_canvas_t *),
			     obs_canvas_t *(*get_ref)(obs_canvas_t *), obs_weak_canvas_t *(*get_weak)(obs_canvas_t *),
			     obs_canvas_t *(*upgrade)(obs_weak_canvas_t *))
{
	struct obs_canvas canvas = {0};
	struct obs_canvas before;
	struct obs_weak_canvas control = {0};
	canvas.context.control = (struct obs_weak_object *)&control;
	control.canvas = &canvas;
	memcpy(&before, &canvas, sizeof(canvas));
	void *results[2] = {NULL, NULL};
	long counts[2][2];
	for (unsigned ported = 0; ported < 2; ported++) {
		control.ref.refs = strong;
		control.ref.weak_refs = weak_count;
		obs_weak_canvas_t *handle = null_handle ? NULL : &control;
		obs_canvas_t *target = null_handle ? NULL : &canvas;
		switch (operation) {
		case 0:
			if (ported)
				addref(handle);
			else
				oracle_obs_weak_canvas_addref(handle);
			break;
		case 1:
			if (ported)
				release(handle);
			else
				oracle_obs_weak_canvas_release(handle);
			break;
		case 2:
			results[ported] = ported ? get_ref(target) : oracle_obs_canvas_get_ref(target);
			break;
		case 3:
			results[ported] = ported ? get_weak(target) : oracle_obs_canvas_get_weak_canvas(target);
			break;
		default:
			results[ported] = ported ? upgrade(handle) : oracle_obs_weak_canvas_get_canvas(handle);
			break;
		}
		counts[ported][0] = control.ref.refs;
		counts[ported][1] = control.ref.weak_refs;
	}
	void *expected = !null_handle && operation >= 2 && (operation == 3 || strong >= 0)
				 ? (operation == 3 ? (void *)&control : (void *)&canvas)
				 : NULL;
	return results[0] == expected && results[1] == expected && counts[0][0] == counts[1][0] &&
	       counts[0][1] == counts[1][1] && control.canvas == &canvas &&
	       memcmp(&canvas, &before, sizeof(canvas)) == 0;
}

/* Exercise the final weak release using the allocator that owns the control.
 * Sanitizers also see both allocation/release pairs; never inspect freed bytes. */
void oracle_canvas_ref_final_release(void (*release)(obs_weak_canvas_t *));
void oracle_canvas_ref_final_release(void (*release)(obs_weak_canvas_t *))
{
	struct obs_weak_canvas *original = bzalloc(sizeof(*original));
	struct obs_weak_canvas *ported = bzalloc(sizeof(*ported));
	original->ref.refs = -1;
	ported->ref.refs = -1;
	oracle_obs_weak_canvas_release(original);
	release(ported);
}
