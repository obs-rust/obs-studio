/* Verbatim C extraction, complete C-owned items, production via callbacks only. */
#define obs_sceneitem_get_rot oracle_obs_sceneitem_get_rot
#define obs_sceneitem_get_alignment oracle_obs_sceneitem_get_alignment
#include "obs-scene-item-transform-fields.c.h"
#undef obs_sceneitem_get_rot
#undef obs_sceneitem_get_alignment

typedef float (*rot_getter)(const obs_sceneitem_t *);
typedef uint32_t (*align_getter)(const obs_sceneitem_t *);
size_t oracle_scene_item_transform_getters_layout(unsigned field);
unsigned oracle_scene_item_transform_probe(uint32_t bits, uint32_t align, unsigned flags, rot_getter get_rot,
					   align_getter get_align);
unsigned oracle_scene_item_transform_concurrent(rot_getter get_rot, align_getter get_align);

static uint32_t transform_bits(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

size_t oracle_scene_item_transform_getters_layout(unsigned field)
{
	const size_t alignment = _Alignof(struct matrix4);
	const size_t span = offsetof(struct obs_scene_item, crop_to_bounds) + sizeof(bool);
	switch (field) {
	case 0:
		return (span + alignment - 1) / alignment * alignment;
	case 1:
		return alignment;
	case 2:
		return offsetof(struct obs_scene_item, ref);
	case 3:
		return offsetof(struct obs_scene_item, item_render);
	case 4:
		return offsetof(struct obs_scene_item, crop);
	case 5:
		return offsetof(struct obs_scene_item, absolute_coordinates);
	case 6:
		return offsetof(struct obs_scene_item, pos);
	case 7:
		return offsetof(struct obs_scene_item, scale);
	case 8:
		return offsetof(struct obs_scene_item, scale_ref);
	case 9:
		return offsetof(struct obs_scene_item, rot);
	case 10:
		return offsetof(struct obs_scene_item, align);
	case 11:
		return offsetof(struct obs_scene_item, last_width);
	case 12:
		return offsetof(struct obs_scene_item, last_height);
	case 13:
		return offsetof(struct obs_scene_item, output_scale);
	case 14:
		return offsetof(struct obs_scene_item, scale_filter);
	case 15:
		return offsetof(struct obs_scene_item, blend_method);
	case 16:
		return offsetof(struct obs_scene_item, blend_type);
	case 17:
		return offsetof(struct obs_scene_item, box_transform);
	case 18:
		return offsetof(struct obs_scene_item, box_scale);
	case 19:
		return offsetof(struct obs_scene_item, draw_transform);
	case 20:
		return offsetof(struct obs_scene_item, bounds_type);
	case 21:
		return offsetof(struct obs_scene_item, bounds_align);
	case 22:
		return offsetof(struct obs_scene_item, bounds);
	case 23:
		return offsetof(struct obs_scene_item, crop_to_bounds);
	case 24:
		return sizeof(enum obs_bounds_type);
	case 25:
		return sizeof(enum obs_scale_type);
	case 26:
		return sizeof(enum obs_blending_method);
	case 27:
		return sizeof(enum obs_blending_type);
	default:
		return (size_t)-1;
	}
}

unsigned oracle_scene_item_transform_probe(uint32_t bits, uint32_t align, unsigned flags, rot_getter get_rot,
					   align_getter get_align)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		return 128;
	struct obs_scene_item item;
	memset(&item, 0xa5, sizeof(item));
	item.parent = (flags & 1) ? &scene : NULL;
	memcpy(&item.rot, &bits, sizeof(bits));
	item.align = align;
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	unsigned result = 0;
	if (transform_bits(get_rot(NULL)) != transform_bits(oracle_obs_sceneitem_get_rot(NULL)))
		result |= 1;
	if (get_align(NULL) != oracle_obs_sceneitem_get_alignment(NULL))
		result |= 2;
	if (pthread_mutex_lock(&scene.video_mutex))
		result |= 128;
	if (transform_bits(get_rot(&item)) != transform_bits(oracle_obs_sceneitem_get_rot(&item)))
		result |= 8;
	if (get_align(&item) != oracle_obs_sceneitem_get_alignment(&item))
		result |= 16;
	if (pthread_mutex_unlock(&scene.video_mutex))
		result |= 128;
	if (memcmp(before, &item, sizeof(item)))
		result |= 64;
	if (pthread_mutex_destroy(&scene.video_mutex))
		result |= 128;
	return result;
}

struct transform_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	rot_getter get_rot;
	align_getter get_align;
	unsigned getter;
	uint32_t result;
};
static void *transform_read_thread(void *data)
{
	struct transform_read *read = data;
	os_event_signal(read->entered);
	if (read->getter == 0)
		read->result = transform_bits(read->get_rot(read->item));
	else
		read->result = read->get_align(read->item);
	os_event_signal(read->finished);
	return NULL;
}

static unsigned transform_concurrent(rot_getter get_rot, align_getter get_align)
{
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	item.parent = &scene;
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		return 128;
	unsigned result = 0;
	for (unsigned getter = 0; getter < 2; getter++) {
		struct transform_read read = {.item = &item,
					      .get_rot = get_rot,
					      .get_align = get_align,
					      .getter = getter};
		if (os_event_init(&read.entered, OS_EVENT_TYPE_AUTO))
			abort();
		if (os_event_init(&read.finished, OS_EVENT_TYPE_AUTO))
			abort();
		item.rot = 0.0f;
		item.align = 0;
		if (pthread_mutex_lock(&scene.video_mutex))
			abort();
		pthread_t thread;
		if (pthread_create(&thread, NULL, transform_read_thread, &read))
			abort();
		if (os_event_timedwait(read.entered, 10000))
			abort();
		int blocked = os_event_timedwait(read.finished, 20);
		item.rot = -0.0f;
		item.align = UINT32_MAX;
		if (pthread_mutex_unlock(&scene.video_mutex))
			abort();
		if (pthread_join(thread, NULL))
			abort();
		if (blocked != ETIMEDOUT)
			result |= 1U << getter;
		if (read.result != (getter == 0 ? UINT32_C(0x80000000) : UINT32_MAX))
			result |= 8U << getter;
		os_event_destroy(read.finished);
		os_event_destroy(read.entered);
	}
	if (pthread_mutex_destroy(&scene.video_mutex))
		result |= 128;
	return result;
}
unsigned oracle_scene_item_transform_concurrent(rot_getter get_rot, align_getter get_align)
{
	unsigned original = transform_concurrent(oracle_obs_sceneitem_get_rot, oracle_obs_sceneitem_get_alignment);
	unsigned port = transform_concurrent(get_rot, get_align);
	return original | port;
}
