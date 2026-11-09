/* Original extracted setters, with scene dependencies supplied by a deterministic
 * fixture. The real backend and symbol parity are covered by unchanged Tier 2. */
#include <string.h>
#include "util/threading.h"
#include "obs-scene.h"

void oracle_sceneitem_set_bounds_type(obs_sceneitem_t *item, enum obs_bounds_type type);
void oracle_sceneitem_set_bounds_alignment(obs_sceneitem_t *item, uint32_t alignment);
void oracle_sceneitem_set_bounds_crop(obs_sceneitem_t *item, bool crop);
void oracle_sceneitem_set_bounds(obs_sceneitem_t *item, const struct vec2 *bounds);

#define obs_scene_bounds_valid_type oracle_scene_bounds_valid_type
#define obs_sceneitem_set_bounds_type oracle_sceneitem_set_bounds_type
#define obs_sceneitem_set_bounds_alignment oracle_sceneitem_set_bounds_alignment
#define obs_sceneitem_set_bounds_crop oracle_sceneitem_set_bounds_crop
#define obs_sceneitem_set_bounds oracle_sceneitem_set_bounds
#include "obs-scene-item-bounds-setters.c.h"
#undef obs_scene_bounds_valid_type
#undef obs_sceneitem_set_bounds_type
#undef obs_sceneitem_set_bounds_alignment
#undef obs_sceneitem_set_bounds_crop
#undef obs_sceneitem_set_bounds

#include "scene_item_bounds_fields.inc"

float obs_sceneitem_bounds_height_dependency(const obs_sceneitem_t *item)
{
	return item->parent ? (float)item->parent->cy : item->scale_ref.y;
}
void obs_sceneitem_bounds_update_dependency(obs_sceneitem_t *item)
{
	if (!item->parent || item->parent->is_group)
		os_atomic_set_bool(&item->update_transform, true);
	else if (os_atomic_load_long(&item->defer_update) <= 0)
		item->last_width++;
}
void obs_sceneitem_bounds_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	const float height = obs_sceneitem_bounds_height_dependency(item);
	dst->x = (2.0f * v->x) / height;
	dst->y = 2.0f * v->y / height;
}

typedef void (*bounds_type_setter)(obs_sceneitem_t *, enum obs_bounds_type);
typedef void (*bounds_align_setter)(obs_sceneitem_t *, uint32_t);
typedef void (*bounds_crop_setter)(obs_sceneitem_t *, bool);
typedef void (*bounds_size_setter)(obs_sceneitem_t *, const struct vec2 *);
typedef uint32_t (*bounds_validator)(int64_t);
unsigned oracle_scene_item_bounds_setters_probe(uint32_t kind, uint32_t alignment, unsigned flags, float x, float y,
						uint32_t height, bounds_type_setter set_type,
						bounds_align_setter set_align, bounds_crop_setter set_crop,
						bounds_size_setter set_size, bounds_validator validate);

unsigned oracle_scene_item_bounds_setters_probe(uint32_t kind, uint32_t alignment, unsigned flags, float x, float y,
						uint32_t height, bounds_type_setter set_type,
						bounds_align_setter set_align, bounds_crop_setter set_crop,
						bounds_size_setter set_size, bounds_validator validate)
{
	struct obs_scene scene = {0};
	scene.is_group = (flags & 1) != 0;
	scene.cy = height;
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		return 1;
	struct obs_scene_item expected = {0};
	expected.parent = (flags & 2) ? &scene : NULL;
	expected.absolute_coordinates = (flags & 4) != 0;
	expected.scale_ref.y = (float)height;
	expected.defer_update = (flags & 8) != 0;
	expected.update_transform = (flags & 16) != 0;
	expected.bounds_type = OBS_BOUNDS_STRETCH;
	expected.bounds_align = 0xfeedbeef;
	expected.id = -0x12345678;
	expected.pos.x = -123.5f;
	expected.bounds.x = x;
	expected.bounds.y = y;
	struct obs_scene_item actual;
	memcpy(&actual, &expected, sizeof(actual));
	unsigned result = 0;
	set_type(NULL, (enum obs_bounds_type)kind);
	set_align(NULL, alignment);
	set_crop(NULL, true);
	set_size(NULL, NULL);
	oracle_sceneitem_set_bounds_type(&expected, (enum obs_bounds_type)kind);
	set_type(&actual, (enum obs_bounds_type)kind);
	if (memcmp(&expected, &actual, sizeof(actual)))
		result |= 2;
	oracle_sceneitem_set_bounds_alignment(&expected, alignment);
	set_align(&actual, alignment);
	if (memcmp(&expected, &actual, sizeof(actual)))
		result |= 4;
	oracle_sceneitem_set_bounds_crop(&expected, (flags & 32) != 0);
	set_crop(&actual, (flags & 32) != 0);
	if (memcmp(&expected, &actual, sizeof(actual)))
		result |= 8;
	const struct vec2 input = {.x = x, .y = y};
	oracle_sceneitem_set_bounds(&expected, (flags & 64) ? &expected.bounds : &input);
	set_size(&actual, (flags & 64) ? &actual.bounds : &input);
	if (memcmp(&expected, &actual, sizeof(actual)) || input.x != x || input.y != y)
		result |= 16;
	const int64_t values[] = {INT64_MIN, -1, 0, 1, 6, 7, INT32_MAX, UINT32_MAX, INT64_MAX, INT64_C(1) << 32};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++)
		if (validate(values[i]) != oracle_scene_bounds_valid_type(values[i]))
			result |= 32;
	pthread_mutex_destroy(&scene.video_mutex);
	return result;
}
