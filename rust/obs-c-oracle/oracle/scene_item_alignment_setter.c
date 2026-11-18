/* Original extracted setter; share the bounds oracle's real fields locks and
 * update recorder, just as production shares the C transform adapter. */
#define obs_sceneitem_set_alignment oracle_obs_sceneitem_set_alignment
#include "libobs/build/libobs.pch.h"
#include "scene-item-alignment-setter.inc"
#undef obs_sceneitem_set_alignment

typedef void (*alignment_setter)(obs_sceneitem_t *, uint32_t);
unsigned oracle_scene_bounds_alignment_probe(alignment_setter original, alignment_setter rust, uint32_t old,
					     uint32_t value, int32_t defer, unsigned flags);
unsigned oracle_scene_alignment_probe(alignment_setter rust, uint32_t old, uint32_t value, int32_t defer,
				      unsigned flags);
unsigned oracle_scene_alignment_null(alignment_setter rust);

unsigned oracle_scene_alignment_probe(alignment_setter rust, uint32_t old, uint32_t value, int32_t defer,
				      unsigned flags)
{
	return oracle_scene_bounds_alignment_probe(oracle_obs_sceneitem_set_alignment, rust, old, value, defer, flags);
}

#if defined(__GNUC__) || defined(__clang__)
static void alignment_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void alignment_capture(int level, const char *format, va_list args, void *param)
{
	UNUSED_PARAMETER(level);
	UNUSED_PARAMETER(format);
	UNUSED_PARAMETER(args);
	(*(unsigned *)param)++;
}

unsigned oracle_scene_alignment_null(alignment_setter rust)
{
	log_handler_t previous;
	void *param;
	base_get_log_handler(&previous, &param);
	unsigned count = 0;
	base_set_log_handler(alignment_capture, &count);
	oracle_obs_sceneitem_set_alignment(NULL, UINT32_MAX);
	rust(NULL, UINT32_MAX);
	base_set_log_handler(previous, param);
	return count;
}
