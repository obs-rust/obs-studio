#include <string.h>
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <graphics/graphics.h>

/*
 * libobs/graphics' Linux entry points (obs-rust/obs-studio#872) live in
 * graphics-functions-linux.h and libobs still exports every one of them.
 * The macOS and Windows lists are compiled by the cross check.
 */

#if !__has_include("libobs/graphics/graphics-functions-linux.h")
#error "graphics-functions-linux.h is missing"
#endif

static void test_linux_entry_points(void **state)
{
	(void)state;
	assert_non_null((void *)gs_texture_create_from_dmabuf);
	assert_non_null((void *)gs_query_dmabuf_capabilities);
	assert_non_null((void *)gs_query_dmabuf_modifiers_for_format);
	assert_non_null((void *)gs_texture_create_from_pixmap);
	assert_non_null((void *)gs_query_sync_capabilities);
	assert_non_null((void *)gs_sync_create);
	assert_non_null((void *)gs_sync_destroy);
	assert_non_null((void *)gs_sync_wait);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_linux_entry_points),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
