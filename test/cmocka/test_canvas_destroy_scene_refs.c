/* Destroying a canvas that holds the last references to its scenes must not
 * read a scene it just freed (#2049).
 *
 * A SCENE_REF canvas holds a reference to each scene in it, and
 * obs_canvas_destroy() releases them while it walks its source list. When
 * that release was the last reference, the scene is destroyed, and the walk
 * read the next list entry out of it. libobs frees a source on its
 * destruction task thread, so the read raced that free, and lost it now and
 * then in test_context_names (#2049).
 *
 * The free stays on another thread, so no public API makes this race
 * deterministic. The case runs it many times and M_PERTURB makes glibc fill
 * freed memory, so a lost race follows a garbage pointer instead of reading
 * the old bytes. Public API only, so it runs against libobs, the hybrid
 * libobs and libobs-c-reference. obs_startup() needs a display on Linux;
 * without one the case skips. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <malloc.h>
#include <stdio.h>

#include <obs.h>

static void test_destroy_releases_the_last_scene_refs(void **state)
{
	(void)state;
	mallopt(M_PERTURB, 0xa5);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}

	for (int round = 0; round < 100; round++) {
		obs_canvas_t *canvas = obs_canvas_create_private("scene refs", NULL, SCENE_REF | EPHEMERAL);
		assert_non_null(canvas);

		/* The canvas's references are the only ones left after these. */
		for (int i = 0; i < 16; i++) {
			char name[16];
			snprintf(name, sizeof(name), "scene %d", i);
			obs_scene_t *scene = obs_canvas_scene_create(canvas, name);
			assert_non_null(scene);
			obs_scene_release(scene);
		}

		obs_canvas_remove(canvas);
		obs_canvas_release(canvas);
	}

	obs_shutdown();
	mallopt(M_PERTURB, 0);
}

/* A source's deferred destroy waits on the mutex of the list it was last in.
 * While a source sits in a canvas's name list that mutex belongs to the
 * canvas, and obs_canvas_destroy() frees it synchronously on the calling
 * thread. A source destroyed just before the canvas therefore had its
 * deferred destroy read freed memory: the Windows Cargo-core characterization
 * crashed in pthread_mutex_unlock through obs_source_destroy_defer (#2547).
 *
 * Public API cannot pause the destroy task thread, so the case frees the
 * canvases right after queueing the source destroy and keeps a backlog of
 * throwaway scene destroys ahead of it. M_PERTURB makes glibc fill the freed
 * canvas memory, so a deferred destroy that still waits on the canvas mutex
 * follows garbage instead of the old bytes. The case runs it many times and
 * runs against libobs, the hybrid libobs and libobs-c-reference. */
static void test_deferred_destroy_after_canvas_free(void **state)
{
	(void)state;
	mallopt(M_PERTURB, 0xa5);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}

	for (int round = 0; round < 100; round++) {
		obs_canvas_t *src = obs_canvas_create_private("mutex src", NULL, EPHEMERAL);
		obs_canvas_t *dst = obs_canvas_create_private("mutex dst", NULL, EPHEMERAL);
		assert_non_null(src);
		assert_non_null(dst);

		obs_scene_t *moved = obs_canvas_scene_create(src, "mutex moved");
		assert_non_null(moved);

		/* A backlog ahead of the moved scene's destroy: each release
		 * queues one, so its destroy lands after the canvases are
		 * freed and the freed mutex bytes are read. */
		for (int i = 0; i < 32; i++) {
			char name[32];
			snprintf(name, sizeof(name), "mutex filler %d", i);
			obs_scene_t *filler = obs_canvas_scene_create(dst, name);
			assert_non_null(filler);
			obs_scene_release(filler);
		}

		/* Moving puts the scene's wait mutex on the destination. */
		obs_canvas_move_scene(moved, dst);
		obs_scene_release(moved);

		obs_canvas_remove(src);
		obs_canvas_release(src);
		obs_canvas_remove(dst);
		obs_canvas_release(dst);
		obs_wait_for_destroy_queue();
	}

	obs_shutdown();
	mallopt(M_PERTURB, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_destroy_releases_the_last_scene_refs),
		cmocka_unit_test(test_deferred_destroy_after_canvas_free),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
