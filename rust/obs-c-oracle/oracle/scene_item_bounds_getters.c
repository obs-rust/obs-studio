/* Verbatim extracted C, real complete items, and the existing mutex boundary. */
#define obs_sceneitem_get_bounds_type oracle_obs_sceneitem_get_bounds_type
#define obs_sceneitem_get_bounds_alignment oracle_obs_sceneitem_get_bounds_alignment
#define obs_sceneitem_get_bounds_crop oracle_obs_sceneitem_get_bounds_crop
#include "obs-scene-item-bounds-getters.c.h"
#undef obs_sceneitem_get_bounds_type
#undef obs_sceneitem_get_bounds_alignment
#undef obs_sceneitem_get_bounds_crop

typedef enum obs_bounds_type (*type_getter)(const obs_sceneitem_t *);
typedef uint32_t (*align_getter)(const obs_sceneitem_t *);
typedef bool (*crop_getter)(const obs_sceneitem_t *);
obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item);
void obs_sceneitem_fields_unlock(obs_scene_t *scene);
size_t oracle_scene_item_bounds_layout(unsigned field);
unsigned oracle_scene_item_bounds_probe(unsigned type, uint32_t align, unsigned flags, uint32_t neighbor,
					type_getter get_type, align_getter get_align, crop_getter get_crop);
unsigned oracle_scene_item_bounds_concurrent(type_getter get_type, align_getter get_align, crop_getter get_crop);

/* Test-build providers for the private production adapters: same helpers,
 * captured parent, ordering, and detached path as obs-scene.c.h. */
obs_scene_t *obs_sceneitem_fields_lock(const obs_sceneitem_t *item)
{
	return item_fields_lock(item);
}
void obs_sceneitem_fields_unlock(obs_scene_t *scene)
{
	item_fields_unlock(scene);
}

size_t oracle_scene_item_bounds_layout(unsigned field)
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

unsigned oracle_scene_item_bounds_probe(unsigned type, uint32_t align, unsigned flags, uint32_t neighbor,
					type_getter get_type, align_getter get_align, crop_getter get_crop)
{
	struct obs_scene scene = {0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		return 128;
	struct obs_scene_item item;
	/* Guard every byte, including the suffix and prefix padding. Unread pointer
     * and bool representations are opaque: only the initialized fields below
     * may be read by these getters. Float neighbors include NaN payloads. */
	memset(&item, 0xa5, sizeof(item));
	item.parent = (flags & 1) ? &scene : NULL;
	item.bounds_type = (enum obs_bounds_type)type;
	item.bounds_align = align;
	item.crop_to_bounds = (flags & 2) != 0;
	item.absolute_coordinates = (flags & 4) != 0;
	memcpy(&item.bounds.x, &neighbor, sizeof(neighbor));
	memcpy(&item.rot, &neighbor, sizeof(neighbor));
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	unsigned result = 0;
	if (get_type(NULL) != oracle_obs_sceneitem_get_bounds_type(NULL))
		result |= 1;
	if (get_align(NULL) != oracle_obs_sceneitem_get_bounds_alignment(NULL))
		result |= 2;
	if (get_crop(NULL) != oracle_obs_sceneitem_get_bounds_crop(NULL))
		result |= 4;
	/* Recursive calls while the owning thread holds the exact video mutex. */
	if (pthread_mutex_lock(&scene.video_mutex))
		result |= 128;
	if (get_type(&item) != oracle_obs_sceneitem_get_bounds_type(&item))
		result |= 8;
	if (get_align(&item) != oracle_obs_sceneitem_get_bounds_alignment(&item))
		result |= 16;
	if (get_crop(&item) != oracle_obs_sceneitem_get_bounds_crop(&item))
		result |= 32;
	if (pthread_mutex_unlock(&scene.video_mutex))
		result |= 128;
	if (memcmp(before, &item, sizeof(item)))
		result |= 64;
	if (pthread_mutex_destroy(&scene.video_mutex))
		result |= 128;
	return result;
}

struct bounds_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	type_getter get_type;
	align_getter get_align;
	crop_getter get_crop;
	unsigned getter;
	uint32_t result;
};
static void *bounds_read_thread(void *data)
{
	struct bounds_read *read = data;
	os_event_signal(read->entered);
	if (read->getter == 0)
		read->result = (uint32_t)read->get_type(read->item);
	else if (read->getter == 1)
		read->result = read->get_align(read->item);
	else
		read->result = read->get_crop(read->item);
	os_event_signal(read->finished);
	return NULL;
}

static unsigned bounds_concurrent(type_getter get_type, align_getter get_align, crop_getter get_crop)
{
	struct obs_scene scene = {0};
	struct obs_scene_item item = {0};
	item.parent = &scene;
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		return 128;
	unsigned result = 0;
	for (unsigned getter = 0; getter < 3; getter++) {
		struct bounds_read read = {.item = &item,
					   .get_type = get_type,
					   .get_align = get_align,
					   .get_crop = get_crop,
					   .getter = getter};
		if (os_event_init(&read.entered, OS_EVENT_TYPE_AUTO))
			abort();
		if (os_event_init(&read.finished, OS_EVENT_TYPE_AUTO))
			abort();
		item.bounds_type = OBS_BOUNDS_NONE;
		item.bounds_align = 0;
		item.crop_to_bounds = false;
		if (pthread_mutex_lock(&scene.video_mutex))
			abort();
		pthread_t thread;
		if (pthread_create(&thread, NULL, bounds_read_thread, &read))
			abort();
		if (os_event_timedwait(read.entered, 10000))
			abort();
		int blocked = os_event_timedwait(read.finished, 20);
		item.bounds_type = OBS_BOUNDS_STRETCH;
		item.bounds_align = UINT32_MAX;
		item.crop_to_bounds = true;
		if (pthread_mutex_unlock(&scene.video_mutex))
			abort();
		if (pthread_join(thread, NULL))
			abort();
		if (blocked != ETIMEDOUT)
			result |= 1U << getter;
		if (read.result != (getter == 0 ? (uint32_t)OBS_BOUNDS_STRETCH : getter == 1 ? UINT32_MAX : 1))
			result |= 8U << getter;
		os_event_destroy(read.finished);
		os_event_destroy(read.entered);
	}
	if (pthread_mutex_destroy(&scene.video_mutex))
		result |= 128;
	return result;
}
unsigned oracle_scene_item_bounds_concurrent(type_getter get_type, align_getter get_align, crop_getter get_crop)
{
	unsigned original = bounds_concurrent(oracle_obs_sceneitem_get_bounds_type,
					      oracle_obs_sceneitem_get_bounds_alignment,
					      oracle_obs_sceneitem_get_bounds_crop);
	unsigned port = bounds_concurrent(get_type, get_align, get_crop);
	return original | port;
}
