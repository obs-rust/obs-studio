/* Characterize obs_sceneitem_get_draw_transform, obs_sceneitem_get_box_transform
 * and obs_sceneitem_get_box_scale before the Rust swap (Refs #5130): the live
 * library and the exact extracted bodies separately. Stack fixtures need no
 * OBS startup, graphics context, or worker threads. A live item with a NULL
 * output is outside the C contract and is not exercised. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <errno.h>
#include <string.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

/* Dependency probes make capture/copy/unlock order observable without
 * replacing any library symbol. The public entry points below still use libobs. */
#define obs_sceneitem_get_draw_transform draw_body
#define obs_sceneitem_get_box_transform box_body
#define obs_sceneitem_get_box_scale scale_body
#define obs_sceneitem_fields_lock matrix_lock
#define obs_sceneitem_fields_unlock matrix_unlock
#include "libobs/obs-scene-item-matrix-getters.c.h"
#undef obs_sceneitem_fields_unlock
#undef obs_sceneitem_fields_lock
#undef obs_sceneitem_get_box_scale
#undef obs_sceneitem_get_box_transform
#undef obs_sceneitem_get_draw_transform

enum getter { GET_DRAW, GET_BOX, GET_SCALE, GETTER_COUNT };

static struct obs_scene_item *expected_item;
static obs_scene_t *captured_parent;
static obs_scene_t *replacement_parent;
static void *output;
static const void *expected_output;
static size_t output_size;
static unsigned phase;
static unsigned diagnostics;

obs_scene_t *matrix_lock(const obs_sceneitem_t *item)
{
	assert_ptr_equal(item, expected_item);
	assert_int_equal(phase++, 0);
	obs_scene_t *parent = item->parent;
	assert_ptr_equal(parent, captured_parent);
	/* Nothing is copied before the lock is taken. */
	assert_memory_not_equal(output, expected_output, output_size);
	if (parent)
		assert_int_equal(pthread_mutex_lock(&parent->video_mutex), 0);
	/* Model a changed parent after capture: unlock must use the returned scene. */
	expected_item->parent = replacement_parent;
	return parent;
}

void matrix_unlock(obs_scene_t *scene)
{
	assert_int_equal(phase++, 1);
	assert_ptr_equal(scene, captured_parent);
	/* The copy is complete while the captured parent is still locked. */
	assert_memory_equal(output, expected_output, output_size);
	if (scene) {
		assert_int_equal(pthread_mutex_trylock(&scene->video_mutex), EBUSY);
		assert_int_equal(pthread_mutex_unlock(&scene->video_mutex), 0);
	}
}

static void count_diagnostic(int level, const char *format, va_list args, void *data)
{
	(void)level;
	(void)format;
	(void)args;
	(void)data;
	diagnostics++;
}

static float bits_float(uint32_t bits)
{
	float value;
	memcpy(&value, &bits, sizeof(value));
	return value;
}

/* Asymmetric, with signed zeros, infinities, a subnormal and quiet NaNs with
 * payloads: any recomputation, transpose or field swap changes the bytes. */
static void fill_matrix(struct matrix4 *m, uint32_t seed)
{
	const uint32_t special[] = {UINT32_C(0x80000000), UINT32_C(0x00000000), UINT32_C(0x7fc00001),
				    UINT32_C(0xffc12345), UINT32_C(0x7f800000), UINT32_C(0xff800000),
				    UINT32_C(0x00000001), UINT32_C(0x3f800000)};
	float *cells[16] = {&m->x.x, &m->x.y, &m->x.z, &m->x.w, &m->y.x, &m->y.y, &m->y.z, &m->y.w,
			    &m->z.x, &m->z.y, &m->z.z, &m->z.w, &m->t.x, &m->t.y, &m->t.z, &m->t.w};
	for (uint32_t i = 0; i < 16; i++) {
		uint32_t bits = (i % 2 == 0) ? special[(i / 2 + seed) % 8] : (UINT32_C(0x40000000) + seed * 97 + i);
		*cells[i] = bits_float(bits);
	}
}

static void fill_sentinel(void *dst, size_t size, unsigned char byte)
{
	memset(dst, byte, size);
}

static void fill_item(struct obs_scene_item *item, uint32_t seed)
{
	fill_matrix(&item->draw_transform, seed);
	fill_matrix(&item->box_transform, seed + 3);
	item->box_scale.x = bits_float(seed % 2 ? UINT32_C(0x80000000) : UINT32_C(0x7fc0beef));
	item->box_scale.y = bits_float(seed % 2 ? UINT32_C(0xffc00002) : UINT32_C(0xc0490fdb));
	/* Neighbouring transform state must not leak into the outputs. */
	vec2_set(&item->output_scale, 0.25f, -0.5f);
	vec2_set(&item->scale, 7.0f, -9.0f);
	vec2_set(&item->pos, -1.5f, 2.5f);
	item->rot = 33.0f;
}

static void call_body(enum getter which, const obs_sceneitem_t *item, void *out)
{
	switch (which) {
	case GET_DRAW:
		draw_body(item, out);
		break;
	case GET_BOX:
		box_body(item, out);
		break;
	default:
		scale_body(item, out);
		break;
	}
}

static void call_public(enum getter which, const obs_sceneitem_t *item, void *out)
{
	switch (which) {
	case GET_DRAW:
		obs_sceneitem_get_draw_transform(item, out);
		break;
	case GET_BOX:
		obs_sceneitem_get_box_transform(item, out);
		break;
	default:
		obs_sceneitem_get_box_scale(item, out);
		break;
	}
}

static const void *stored_field(const struct obs_scene_item *item, enum getter which, size_t *size)
{
	switch (which) {
	case GET_DRAW:
		*size = sizeof(item->draw_transform);
		return &item->draw_transform;
	case GET_BOX:
		*size = sizeof(item->box_transform);
		return &item->box_transform;
	default:
		*size = sizeof(item->box_scale);
		return &item->box_scale;
	}
}

static void null_item_is_noop(void **state)
{
	(void)state;
	phase = diagnostics = 0;
	base_set_log_handler(count_diagnostic, NULL);
	for (unsigned which = 0; which < GETTER_COUNT; which++) {
		struct matrix4 m, sentinel_m;
		struct vec2 v, sentinel_v;
		fill_sentinel(&m, sizeof(m), 0xA5);
		fill_sentinel(&v, sizeof(v), 0x5A);
		memcpy(&sentinel_m, &m, sizeof(m));
		memcpy(&sentinel_v, &v, sizeof(v));
		void *out = which == GET_SCALE ? (void *)&v : (void *)&m;

		call_public(which, NULL, out);
		call_body(which, NULL, out);
		/* A NULL output is never written through for a NULL item. */
		call_public(which, NULL, NULL);
		call_body(which, NULL, NULL);

		assert_memory_equal(&m, &sentinel_m, sizeof(m));
		assert_memory_equal(&v, &sentinel_v, sizeof(v));
	}
	base_set_log_handler(NULL, NULL);
	assert_int_equal(diagnostics, 0);
	assert_int_equal(phase, 0);
}

static void captured_parent_and_order(void **state)
{
	(void)state;
	struct obs_scene scene = {0}, other = {0};
	/* Nonrecursive: a second lock before unlock cannot pass the trylock probe. */
	assert_int_equal(pthread_mutex_init(&scene.video_mutex, NULL), 0);
	for (unsigned which = 0; which < GETTER_COUNT; which++) {
		for (unsigned parented = 0; parented < 2; parented++) {
			for (uint32_t seed = 0; seed < 4; seed++) {
				struct obs_scene_item item = {0};
				fill_item(&item, seed);
				captured_parent = parented ? &scene : NULL;
				replacement_parent = parented ? &other : &scene;
				item.parent = captured_parent;
				struct obs_scene_item before;
				memcpy(&before, &item, sizeof(item));

				struct matrix4 m;
				struct vec2 v;
				fill_sentinel(&m, sizeof(m), 0xC3);
				fill_sentinel(&v, sizeof(v), 0x3C);
				output = which == GET_SCALE ? (void *)&v : (void *)&m;
				expected_output = stored_field(&item, which, &output_size);
				expected_item = &item;
				phase = 0;

				call_body(which, &item, output);

				assert_int_equal(phase, 2);
				assert_memory_equal(output, expected_output, output_size);
				/* Nothing else in the item is written: no recomputation. */
				assert_ptr_equal(item.parent, replacement_parent);
				item.parent = before.parent;
				assert_memory_equal(&item, &before, sizeof(item));
			}
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

static void live_copies_exact_fields(void **state)
{
	(void)state;
	struct obs_scene scene = {0};
	/* Nonrecursive: a parent left locked fails the trylock after each call. */
	assert_int_equal(pthread_mutex_init(&scene.video_mutex, NULL), 0);
	for (unsigned which = 0; which < GETTER_COUNT; which++) {
		for (unsigned parented = 0; parented < 2; parented++) {
			for (uint32_t seed = 0; seed < 8; seed++) {
				struct obs_scene_item item = {0};
				fill_item(&item, seed);
				item.parent = parented ? &scene : NULL;
				struct obs_scene_item before;
				memcpy(&before, &item, sizeof(item));
				struct matrix4 m;
				struct vec2 v;
				fill_sentinel(&m, sizeof(m), 0x96);
				fill_sentinel(&v, sizeof(v), 0x69);
				void *out = which == GET_SCALE ? (void *)&v : (void *)&m;
				size_t size;
				const void *field = stored_field(&item, which, &size);

				call_public(which, &item, out);

				assert_memory_equal(out, field, size);
				assert_memory_equal(&item, &before, sizeof(item));
				assert_int_equal(pthread_mutex_trylock(&scene.video_mutex), 0);
				assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
			}
		}
	}
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

static void live_reads_under_parent_lock(void **state)
{
	(void)state;
	/* A recursive parent lock held by the caller: each getter relocks and
	 * releases it once, leaving the caller's hold intact. */
	struct obs_scene scene = {0};
	assert_int_equal(pthread_mutex_init_recursive(&scene.video_mutex), 0);
	struct obs_scene_item item = {0};
	fill_item(&item, 1);
	item.parent = &scene;
	assert_int_equal(pthread_mutex_lock(&scene.video_mutex), 0);
	struct matrix4 draw, box;
	struct vec2 scale;
	obs_sceneitem_get_draw_transform(&item, &draw);
	obs_sceneitem_get_box_transform(&item, &box);
	obs_sceneitem_get_box_scale(&item, &scale);
	assert_memory_equal(&draw, &item.draw_transform, sizeof(draw));
	assert_memory_equal(&box, &item.box_transform, sizeof(box));
	assert_memory_equal(&scale, &item.box_scale, sizeof(scale));
	assert_int_equal(pthread_mutex_unlock(&scene.video_mutex), 0);
	assert_int_equal(pthread_mutex_destroy(&scene.video_mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_item_is_noop),
					   cmocka_unit_test(captured_parent_and_order),
					   cmocka_unit_test(live_copies_exact_fields),
					   cmocka_unit_test(live_reads_under_parent_lock)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
