/* C-only characterization before extracting obs_get_main_texture (Refs #5429).
 * An initialized core is a precondition. The main canvas mix is swapped for a
 * zeroed stand-in so no graphics or video reset is needed; the getter only
 * reads the texture_rendered flag and the render_texture pointer and never
 * dereferences the texture. The core global is hidden from tests, so a NULL
 * main canvas (unreachable in a live core) is pinned by the Rust swap's
 * field-adapter parity instead (#5430). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static char texture_a_storage;
static char texture_b_storage;
#define TEXTURE_A ((gs_texture_t *)&texture_a_storage)
#define TEXTURE_B ((gs_texture_t *)&texture_b_storage)

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

struct canvas_refs {
	long refs;
	long weak_refs;
};

static struct canvas_refs read_refs(const obs_canvas_t *canvas)
{
	struct canvas_refs refs = {
		.refs = os_atomic_load_long(&canvas->context.control->ref.refs),
		.weak_refs = os_atomic_load_long(&canvas->context.control->ref.weak_refs),
	};
	return refs;
}

static void assert_refs_unchanged(const obs_canvas_t *canvas, struct canvas_refs before)
{
	struct canvas_refs now = read_refs(canvas);
	assert_int_equal(now.refs, before.refs);
	assert_int_equal(now.weak_refs, before.weak_refs);
}

/* The core keeps its own reference, so the borrowed pointer stays valid. */
static obs_canvas_t *main_canvas(void)
{
	obs_canvas_t *canvas = obs_get_main_canvas();
	assert_non_null(canvas);
	obs_canvas_release(canvas);
	return canvas;
}

static void main_canvas_without_mix_returns_null(void **state)
{
	(void)state;
	obs_canvas_t *canvas = main_canvas();
	assert_null(canvas->mix);
	struct canvas_refs before = read_refs(canvas);
	for (unsigned i = 0; i < 16; ++i)
		assert_null(obs_get_main_texture());
	assert_null(canvas->mix);
	assert_refs_unchanged(canvas, before);
}

static void unrendered_mix_returns_null(void **state)
{
	(void)state;
	obs_canvas_t *canvas = main_canvas();
	struct obs_core_video_mix *saved = canvas->mix;
	assert_null(saved);
	struct obs_core_video_mix mix, before;
	memset(&mix, 0, sizeof(mix));
	mix.render_texture = TEXTURE_A;
	mix.texture_rendered = false;
	canvas->mix = &mix;
	struct canvas_refs refs = read_refs(canvas);

	/* A texture that exists but has not been rendered this frame is not
	 * handed out. */
	memcpy(&before, &mix, sizeof(mix));
	for (unsigned i = 0; i < 16; ++i)
		assert_null(obs_get_main_texture());
	assert_memory_equal(&mix, &before, sizeof(mix));
	assert_ptr_equal(canvas->mix, &mix);
	assert_refs_unchanged(canvas, refs);

	canvas->mix = saved;
}

static void rendered_mix_texture_is_borrowed_by_identity(void **state)
{
	(void)state;
	obs_canvas_t *canvas = main_canvas();
	struct obs_core_video_mix *saved = canvas->mix;
	assert_null(saved);
	struct obs_core_video_mix mix, before;
	memset(&mix, 0, sizeof(mix));
	mix.render_texture = TEXTURE_A;
	os_atomic_set_bool(&mix.texture_rendered, true);
	canvas->mix = &mix;
	struct canvas_refs refs = read_refs(canvas);

	memcpy(&before, &mix, sizeof(mix));
	for (unsigned i = 0; i < 16; ++i)
		assert_ptr_equal(obs_get_main_texture(), TEXTURE_A);
	assert_memory_equal(&mix, &before, sizeof(mix));
	assert_ptr_equal(canvas->mix, &mix);
	assert_refs_unchanged(canvas, refs);

	/* The getter follows the current mix's texture, not a cached value. */
	mix.render_texture = TEXTURE_B;
	memcpy(&before, &mix, sizeof(mix));
	assert_ptr_equal(obs_get_main_texture(), TEXTURE_B);
	assert_memory_equal(&mix, &before, sizeof(mix));

	/* A rendered mix with no texture answers NULL, not the stale one. */
	mix.render_texture = NULL;
	assert_null(obs_get_main_texture());

	/* Clearing the flag withdraws the texture again. */
	mix.render_texture = TEXTURE_A;
	os_atomic_set_bool(&mix.texture_rendered, false);
	assert_null(obs_get_main_texture());
	os_atomic_set_bool(&mix.texture_rendered, true);
	assert_ptr_equal(obs_get_main_texture(), TEXTURE_A);
	assert_ptr_equal(canvas->mix, &mix);
	assert_refs_unchanged(canvas, refs);

	canvas->mix = saved;
	assert_null(obs_get_main_texture());
}

static void auxiliary_canvas_mix_is_not_the_main_texture(void **state)
{
	(void)state;
	obs_canvas_t *main = main_canvas();
	obs_canvas_t *other = obs_canvas_create_private("get main texture other", NULL, 0);
	assert_non_null(other);
	struct obs_core_video_mix *saved = other->mix;
	struct obs_core_video_mix mix;
	memset(&mix, 0, sizeof(mix));
	mix.render_texture = TEXTURE_B;
	os_atomic_set_bool(&mix.texture_rendered, true);
	other->mix = &mix;
	assert_null(main->mix);
	assert_null(obs_get_main_texture());
	other->mix = saved;
	obs_canvas_release(other);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(main_canvas_without_mix_returns_null, setup, teardown),
		cmocka_unit_test_setup_teardown(unrendered_mix_returns_null, setup, teardown),
		cmocka_unit_test_setup_teardown(rendered_mix_texture_is_borrowed_by_identity, setup, teardown),
		cmocka_unit_test_setup_teardown(auxiliary_canvas_mix_is_not_the_main_texture, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
