#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <cmocka.h>

/* Private production seam. Deliberately link only its real C unit: neither
 * libobs nor a fake graphics runtime may supply this callback. The unchanged
 * test_graphics_viewport additionally observes publication by gs_create and
 * reset by gs_destroy through the public image loader in both OFF/ON modes. */
uint32_t gs_image_max_texture_size(void);
void gs_image_set_max_texture_size(uint32_t size);

static void cached_limit(void **state)
{
	(void)state;
	assert_int_equal(gs_image_max_texture_size(), 0);
	const uint32_t limits[] = {1, 7, 8192, 16384, UINT32_MAX, 0};
	for (size_t i = 0; i < sizeof(limits) / sizeof(limits[0]); i++) {
		gs_image_set_max_texture_size(limits[i]);
		assert_int_equal(gs_image_max_texture_size(), limits[i]);
		/* Loading the callback repeatedly must not consume or change it. */
		assert_int_equal(gs_image_max_texture_size(), limits[i]);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(cached_limit)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
