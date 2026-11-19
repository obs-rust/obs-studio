/* Original source query and C weak upgrade on real private layouts. */
#include <string.h>
#include "libobs/build/libobs.pch.h"
obs_canvas_t *oracle_obs_weak_canvas_get_canvas(obs_weak_canvas_t *weak);
obs_canvas_t *oracle_obs_source_get_canvas(const obs_source_t *source);
#define obs_source_get_canvas oracle_obs_source_get_canvas
#define obs_weak_canvas_get_canvas oracle_obs_weak_canvas_get_canvas
#include "source-canvas-getter.inc"
#undef obs_weak_canvas_get_canvas
#undef obs_source_get_canvas
#include "source_canvas_getter_fields.inc"

typedef obs_canvas_t *(*source_canvas_getter)(const obs_source_t *);

bool oracle_source_canvas_check(source_canvas_getter getter, long refs, long weak_refs, unsigned flags, uint32_t noise);
bool oracle_source_canvas_check(source_canvas_getter getter, long refs, long weak_refs, unsigned flags, uint32_t noise)
{
	struct obs_canvas canvas[2] = {0};
	struct obs_canvas canvas_before[2];
	memcpy(canvas_before, canvas, sizeof(canvas));
	struct obs_weak_canvas controls[2] = {0};
	struct obs_source source = {0}, before;
	bool equal = true;
	source_canvas_getter getters[] = {oracle_obs_source_get_canvas, getter};
	for (size_t g = 0; g < 2; g++) {
		for (size_t i = 0; i < 2; i++) {
			controls[i].ref.refs = refs;
			controls[i].ref.weak_refs = weak_refs;
			controls[i].canvas = &canvas[i];
		}
		source.info.output_flags = noise;
		source.info.type_data = &source;
		/* Moving a source changes only its borrowed weak token. Retain every
		 * earlier return until both the old and new owner have been queried. */
		obs_canvas_t *results[2][2];
		for (size_t owner = 0; owner < 2; owner++) {
			size_t index = (flags & 2) ? owner : 0;
			source.canvas = (flags & 1) ? NULL : &controls[index];
			memcpy(&before, &source, sizeof(source));
			for (size_t call = 0; call < 2; call++) {
				results[owner][call] = getters[g](&source);
				obs_canvas_t *expected = !(flags & 1) && refs >= 0 ? &canvas[index] : NULL;
				equal = equal && results[owner][call] == expected &&
					memcmp(&before, &source, sizeof(source)) == 0;
				for (size_t i = 0; i < 2; i++) {
					long acquired = 0;
					if (!(flags & 1) && refs >= 0) {
						if (flags & 2)
							acquired = i < owner ? 2 : (i == owner ? (long)call + 1 : 0);
						else if (i == 0)
							acquired = (long)(owner * 2 + call + 1);
					}
					equal = equal && controls[i].ref.refs == refs + acquired &&
						controls[i].ref.weak_refs == weak_refs &&
						controls[i].canvas == &canvas[i];
				}
			}
		}
		/* Balance the synthetic owned returns without entering destruction:
		 * initial live counts are retained throughout this fixture. */
		for (size_t owner = 0; owner < 2; owner++) {
			for (size_t call = 0; call < 2; call++) {
				if (results[owner][call]) {
					size_t index = (flags & 2) ? owner : 0;
					os_atomic_dec_long(&controls[index].ref.refs);
				}
			}
		}
		for (size_t i = 0; i < 2; i++)
			equal = equal && controls[i].ref.refs == refs && controls[i].ref.weak_refs == weak_refs;
	}
	return equal && memcmp(canvas_before, canvas, sizeof(canvas)) == 0;
}
