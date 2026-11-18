/* Original getters with complete C-owned objects; production only via callbacks. */
#define obs_sceneitem_get_pos oracle_obs_sceneitem_get_pos
#define obs_sceneitem_get_bounds oracle_obs_sceneitem_get_bounds
#define obs_sceneitem_get_info2 oracle_obs_sceneitem_get_info2
#include "obs-scene-item-info-getters.c.h"
#undef obs_sceneitem_get_pos
#undef obs_sceneitem_get_bounds
#undef obs_sceneitem_get_info2

typedef void (*vec2_getter)(const obs_sceneitem_t *, struct vec2 *);
typedef void (*info_getter)(const obs_sceneitem_t *, struct obs_transform_info *);
void oracle_obs_scene_pos_to_absolute(struct vec2 *, const struct vec2 *, float, float, bool);
void oracle_obs_scene_size_to_absolute(struct vec2 *, const struct vec2 *, float, bool);
bool oracle_obs_scene_coords_nudge(bool, bool);
size_t oracle_scene_item_info_layout(unsigned field);
unsigned oracle_scene_item_info_probe(const uint32_t *bits, unsigned flags, vec2_getter get_pos, vec2_getter get_bounds,
				      info_getter get_info);
unsigned oracle_scene_item_info_concurrent(vec2_getter get_pos, vec2_getter get_bounds, info_getter get_info);

/* Test-build providers for the private conversion boundaries. The fixtures
 * use a custom-sized parent; a detached item reads 0x0, as the main canvas
 * before the first obs_reset_video(). The canvas lookup itself is
 * characterized against the actual libraries, not mocked as parity. The
 * scale boundary comes from scene_item_scale_getter.c. */
static void oracle_dimensions(const obs_sceneitem_t *item, float *x, float *y)
{
	*x = item->parent ? (float)item->parent->cx : 0.0f;
	*y = item->parent ? (float)item->parent->cy : 0.0f;
}

void obs_sceneitem_pos_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	oracle_dimensions(item, &x, &y);
	oracle_obs_scene_pos_to_absolute(
		dst, v, x, y, oracle_obs_scene_coords_nudge(item->is_group, item->parent && item->parent->is_group));
}

void obs_sceneitem_size_canvas_read(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	float x, y;
	oracle_dimensions(item, &x, &y);
	oracle_obs_scene_size_to_absolute(
		dst, v, y, oracle_obs_scene_coords_nudge(item->is_group, item->parent && item->parent->is_group));
}

size_t oracle_scene_item_info_layout(unsigned field)
{
	const size_t values[] = {sizeof(struct obs_transform_info),
				 _Alignof(struct obs_transform_info),
				 offsetof(struct obs_transform_info, pos),
				 offsetof(struct obs_transform_info, rot),
				 offsetof(struct obs_transform_info, scale),
				 offsetof(struct obs_transform_info, alignment),
				 offsetof(struct obs_transform_info, bounds_type),
				 offsetof(struct obs_transform_info, bounds_alignment),
				 offsetof(struct obs_transform_info, bounds),
				 offsetof(struct obs_transform_info, crop_to_bounds),
				 sizeof(enum obs_bounds_type),
				 offsetof(struct obs_scene_item, absolute_coordinates),
				 offsetof(struct obs_scene_item, pos),
				 offsetof(struct obs_scene_item, rot),
				 offsetof(struct obs_scene_item, align),
				 offsetof(struct obs_scene_item, bounds_type),
				 offsetof(struct obs_scene_item, bounds_align),
				 offsetof(struct obs_scene_item, bounds),
				 offsetof(struct obs_scene_item, crop_to_bounds)};
	return field < sizeof(values) / sizeof(*values) ? values[field] : (size_t)-1;
}

struct guarded_vec2 {
	uint32_t pre;
	struct vec2 out;
	uint32_t post;
};

struct guarded_info {
	uint32_t pre;
	struct obs_transform_info info;
	uint32_t post;
};

/* bits: pos x/y, scale x/y, bounds x/y, rot, align, bounds_type, bounds_align,
 * scale_ref.y. flags: 1 parent, 2 absolute, 4 group, 8 nested scene,
 * 16 crop_to_bounds, 32 parent is a group. */
unsigned oracle_scene_item_info_probe(const uint32_t *bits, unsigned flags, vec2_getter get_pos, vec2_getter get_bounds,
				      info_getter get_info)
{
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080, .is_group = (flags & 32) != 0};
	if (pthread_mutex_init_recursive(&scene.video_mutex))
		abort();
	struct obs_scene_item item;
	memset(&item, 0xa5, sizeof(item));
	item.parent = (flags & 1) ? &scene : NULL;
	item.absolute_coordinates = (flags & 2) != 0;
	item.is_group = (flags & 4) != 0;
	item.is_scene = (flags & 8) != 0;
	item.crop_to_bounds = (flags & 16) != 0;
	/* The scale boundary's stand-in needs a parent for a plain relative item. */
	if (!item.parent && !item.absolute_coordinates && !item.is_group && !item.is_scene)
		item.parent = &scene;
	memcpy(&item.pos.x, &bits[0], sizeof(uint32_t));
	memcpy(&item.pos.y, &bits[1], sizeof(uint32_t));
	memcpy(&item.scale.x, &bits[2], sizeof(uint32_t));
	memcpy(&item.scale.y, &bits[3], sizeof(uint32_t));
	memcpy(&item.bounds.x, &bits[4], sizeof(uint32_t));
	memcpy(&item.bounds.y, &bits[5], sizeof(uint32_t));
	memcpy(&item.rot, &bits[6], sizeof(uint32_t));
	item.align = bits[7];
	memcpy(&item.bounds_type, &bits[8], sizeof(uint32_t));
	item.bounds_align = bits[9];
	memcpy(&item.scale_ref.y, &bits[10], sizeof(uint32_t));
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));

	unsigned result = 0;
	struct guarded_vec2 expected, actual;
	memset(&expected, 0x59, sizeof(expected));
	memcpy(&actual, &expected, sizeof(actual));
	get_pos(NULL, &actual.out);
	get_pos(NULL, NULL);
	get_bounds(NULL, &actual.out);
	get_bounds(NULL, NULL);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 1;
	struct guarded_info expected_info, actual_info;
	memset(&expected_info, 0x5a, sizeof(expected_info));
	memcpy(&actual_info, &expected_info, sizeof(actual_info));
	get_info(NULL, &actual_info.info);
	get_info(NULL, NULL);
	get_info(&item, NULL);
	if (memcmp(&actual_info, &expected_info, sizeof(actual_info)) || memcmp(before, &item, sizeof(item)))
		result |= 2;

	if (pthread_mutex_lock(&scene.video_mutex))
		abort();
	oracle_obs_sceneitem_get_pos(&item, &expected.out);
	get_pos(&item, &actual.out);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 4;
	const struct vec2 expected_pos = expected.out;
	oracle_obs_sceneitem_get_bounds(&item, &expected.out);
	get_bounds(&item, &actual.out);
	if (memcmp(&actual, &expected, sizeof(actual)))
		result |= 8;
	const struct vec2 expected_bounds = expected.out;
	/* Padding keeps the caller's bytes: compare the whole guarded struct. */
	oracle_obs_sceneitem_get_info2(&item, &expected_info.info);
	get_info(&item, &actual_info.info);
	if (memcmp(&actual_info, &expected_info, sizeof(actual_info)))
		result |= 16;
	if (memcmp(before, &item, sizeof(item)))
		result |= 32;
	/* The original permits output aliasing the stored field. */
	get_pos(&item, &item.pos);
	if (memcmp(&item.pos, &expected_pos, sizeof(item.pos)))
		result |= 64;
	memcpy(&item, before, sizeof(item));
	get_bounds(&item, &item.bounds);
	if (memcmp(&item.bounds, &expected_bounds, sizeof(item.bounds)))
		result |= 128;
	memcpy(&item, before, sizeof(item));
	if (pthread_mutex_unlock(&scene.video_mutex))
		abort();
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

struct info_read {
	struct obs_scene_item *item;
	os_event_t *entered;
	os_event_t *finished;
	vec2_getter get_pos;
	vec2_getter get_bounds;
	info_getter get_info;
	struct guarded_vec2 out;
	struct guarded_info info;
};

static void *info_read_thread(void *data)
{
	struct info_read *read = data;
	os_event_signal(read->entered);
	if (read->get_pos)
		read->get_pos(read->item, &read->out.out);
	else if (read->get_bounds)
		read->get_bounds(read->item, &read->out.out);
	else
		read->get_info(read->item, &read->info.info);
	os_event_signal(read->finished);
	return NULL;
}

/* One reader blocks on the parent's video mutex while the fields change,
 * then publishes the values the original computes from the changed fields. */
static unsigned info_concurrent(vec2_getter get_pos, vec2_getter get_bounds, info_getter get_info, bool relative)
{
	struct obs_scene scene = {.custom_size = true, .cx = 1920, .cy = 1080};
	struct obs_scene_item item = {.parent = &scene, .absolute_coordinates = !relative};
	item.scale_ref.y = 540.0f;
	struct info_read read = {.item = &item, .get_pos = get_pos, .get_bounds = get_bounds, .get_info = get_info};
	memset(&read.out, 0x59, sizeof(read.out));
	memset(&read.info, 0x5a, sizeof(read.info));
	struct guarded_vec2 expected = read.out;
	struct guarded_info expected_info = read.info;
	if (pthread_mutex_init_recursive(&scene.video_mutex) || os_event_init(&read.entered, OS_EVENT_TYPE_AUTO) ||
	    os_event_init(&read.finished, OS_EVENT_TYPE_AUTO) || pthread_mutex_lock(&scene.video_mutex))
		abort();
	pthread_t thread;
	if (pthread_create(&thread, NULL, info_read_thread, &read) || os_event_timedwait(read.entered, 10000))
		abort();
	int blocked = os_event_timedwait(read.finished, 20);
	item.pos.x = 0.0009765625f;
	item.pos.y = 0.00048828125f;
	item.scale.x = -3.0f;
	item.scale.y = 5.0f;
	item.bounds.x = 0.5f;
	item.bounds.y = -9.0f;
	item.rot = 11.0f;
	item.align = 5;
	item.bounds_type = OBS_BOUNDS_STRETCH;
	item.bounds_align = 10;
	item.crop_to_bounds = true;
	if (pthread_mutex_unlock(&scene.video_mutex) || pthread_join(thread, NULL))
		abort();
	if (get_pos)
		oracle_obs_sceneitem_get_pos(&item, &expected.out);
	else if (get_bounds)
		oracle_obs_sceneitem_get_bounds(&item, &expected.out);
	else
		oracle_obs_sceneitem_get_info2(&item, &expected_info.info);
	unsigned result = (blocked != ETIMEDOUT ? 1U : 0U) |
			  (memcmp(&read.out, &expected, sizeof(expected)) ? 2U : 0U) |
			  (memcmp(&read.info, &expected_info, sizeof(expected_info)) ? 4U : 0U);
	os_event_destroy(read.finished);
	os_event_destroy(read.entered);
	if (pthread_mutex_destroy(&scene.video_mutex))
		abort();
	return result;
}

unsigned oracle_scene_item_info_concurrent(vec2_getter get_pos, vec2_getter get_bounds, info_getter get_info)
{
	unsigned result = 0;
	for (int relative = 0; relative < 2; relative++) {
		result |= info_concurrent(oracle_obs_sceneitem_get_pos, NULL, NULL, relative) |
			  info_concurrent(get_pos, NULL, NULL, relative) |
			  info_concurrent(NULL, oracle_obs_sceneitem_get_bounds, NULL, relative) |
			  info_concurrent(NULL, get_bounds, NULL, relative) |
			  info_concurrent(NULL, NULL, oracle_obs_sceneitem_get_info2, relative) |
			  info_concurrent(NULL, NULL, get_info, relative);
	}
	return result;
}
