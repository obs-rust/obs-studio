/* Original getter with complete C-owned objects; production only via callbacks. */
#define obs_sceneitem_get_scale oracle_obs_sceneitem_get_scale
#include "obs-scene-item-scale-fields.c.h"
#undef obs_sceneitem_get_scale

typedef void (*scale_getter)(const obs_sceneitem_t *, struct vec2 *);
void oracle_obs_scene_item_canvas_scale(struct vec2 *, const struct vec2 *, float, float);
bool oracle_obs_scene_coords_keep_scale(bool, bool);
size_t oracle_scene_item_scale_layout(unsigned field);
unsigned oracle_scene_item_scale_probe(uint32_t x, uint32_t y, uint32_t reference, unsigned flags, scale_getter getter);
unsigned oracle_scene_item_scale_concurrent(scale_getter getter);

/* Test-build provider for the private conversion boundary. The fixtures below
 * use a custom-sized parent, or the group/scene keep-scale path. Canvas lookup
 * itself is characterized against the actual libraries, not mocked as parity. */
void obs_sceneitem_scale_canvas_read(struct vec2 *dst, const obs_sceneitem_t *item)
{
	if (oracle_obs_scene_coords_keep_scale(item->is_group, item->is_scene)) {
		vec2_copy(dst, &item->scale);
		return;
	}
	oracle_obs_scene_item_canvas_scale(dst, &item->scale, (float)item->parent->cy, item->scale_ref.y);
}

size_t oracle_scene_item_scale_layout(unsigned field)
{
	const size_t values[] = {sizeof(struct vec2),
				 _Alignof(struct vec2),
				 offsetof(struct vec2, x),
				 offsetof(struct vec2, y),
				 offsetof(struct obs_scene_item, absolute_coordinates),
				 offsetof(struct obs_scene_item, scale),
				 offsetof(struct obs_scene_item, scale_ref)};
	return field < sizeof(values) / sizeof(*values) ? values[field] : (size_t)-1;
}

unsigned oracle_scene_item_scale_probe(uint32_t x, uint32_t y, uint32_t reference, unsigned flags, scale_getter getter)
{
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct obs_scene_item item;
	memset(&item, 0xa5, sizeof(item));
	item.parent = (flags & 1) ? &scene : NULL;
	item.absolute_coordinates = (flags & 2) != 0;
	item.is_group = (flags & 4) != 0;
	item.is_scene = (flags & 8) != 0;
	if (!item.parent && !item.absolute_coordinates && !item.is_group && !item.is_scene)
		item.parent = &scene;
	memcpy(&item.scale.x, &x, sizeof(x));
	memcpy(&item.scale.y, &y, sizeof(y));
	memcpy(&item.scale_ref.y, &reference, sizeof(reference));
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	struct guarded_scale {
		uint32_t pre;
		struct vec2 out;
		uint32_t post;
	} expected, actual;
	memset(&expected, 0x59, sizeof(expected));
	memcpy(&actual, &expected, sizeof(actual));
	unsigned result = 0;
	getter(NULL, &actual.out);
	getter(NULL, NULL);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 1;
	if (pthread_mutex_lock(&scene.video_mutex))
		abort();
	oracle_obs_sceneitem_get_scale(&item, &expected.out);
	getter(&item, &actual.out);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 2;
	if (memcmp(before, &item, sizeof(item)))
		result |= 4;
	/* The original permits output aliasing the stored scale. */
	getter(&item, &item.scale);
	if (memcmp(&item.scale, &expected.out, sizeof(item.scale)))
		result |= 8;
	if (pthread_mutex_unlock(&scene.video_mutex))
		abort();
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

struct scale_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	scale_getter getter;
	struct vec2 out;
};
static void *scale_read_thread(void *data)
{
	struct scale_read *read = data;
	os_event_signal(read->entered);
	read->getter(read->item, &read->out);
	os_event_signal(read->finished);
	return NULL;
}
static unsigned scale_concurrent(scale_getter getter, bool relative)
{
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080};
	struct obs_scene_item item = {.parent = &scene, .absolute_coordinates = !relative};
	item.scale_ref.y = 540.0f;
	struct scale_read read = {.item = &item, .getter = getter};
	if (pthread_mutex_init_recursive(&scene.video_mutex) || os_event_init(&read.entered, OS_EVENT_TYPE_AUTO) ||
	    os_event_init(&read.finished, OS_EVENT_TYPE_AUTO) || pthread_mutex_lock(&scene.video_mutex))
		abort();
	pthread_t thread;
	if (pthread_create(&thread, NULL, scale_read_thread, &read) || os_event_timedwait(read.entered, 10000))
		abort();
	int blocked = os_event_timedwait(read.finished, 20);
	item.scale.x = -3.0f;
	item.scale.y = 5.0f;
	if (pthread_mutex_unlock(&scene.video_mutex) || pthread_join(thread, NULL))
		abort();
	struct vec2 expected = {.x = relative ? -6.0f : -3.0f, .y = relative ? 10.0f : 5.0f};
	unsigned result = (blocked != ETIMEDOUT ? 1U : 0U) | (memcmp(&read.out, &expected, sizeof(expected)) ? 2U : 0U);
	os_event_destroy(read.finished);
	os_event_destroy(read.entered);
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}
unsigned oracle_scene_item_scale_concurrent(scale_getter getter)
{
	return scale_concurrent(oracle_obs_sceneitem_get_scale, false) | scale_concurrent(getter, false) |
	       scale_concurrent(oracle_obs_sceneitem_get_scale, true) | scale_concurrent(getter, true);
}
