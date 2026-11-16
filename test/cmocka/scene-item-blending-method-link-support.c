/* Real field-address adapter for standalone Rust archive consumers. */
#include "util/threading.h"
#include "obs-scene.h"

unsigned *obs_sceneitem_blending_method_field(obs_sceneitem_t *item);
unsigned *obs_sceneitem_blending_method_field(obs_sceneitem_t *item)
{
	_Static_assert(sizeof(enum obs_blending_method) == sizeof(unsigned), "blending method ABI");
	_Static_assert(_Alignof(enum obs_blending_method) == _Alignof(unsigned), "blending method alignment");
	return (unsigned *)&item->blend_method;
}
