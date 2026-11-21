/* Characterization of obs-nix-platform.c.h: borrowed, unsynchronized host state. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-nix-platform.h"

static void platform_state(void **unused)
{
	(void)unused;
	unsigned char first = 7, second = 9;
	assert_int_equal(obs_get_nix_platform(), OBS_NIX_PLATFORM_X11_EGL);
	assert_null(obs_get_nix_platform_display());
	obs_set_nix_platform_display(&first);
	assert_ptr_equal(obs_get_nix_platform_display(), &first);
	obs_set_nix_platform(OBS_NIX_PLATFORM_WAYLAND);
	assert_int_equal(obs_get_nix_platform(), OBS_NIX_PLATFORM_WAYLAND);
	assert_ptr_equal(obs_get_nix_platform_display(), &first);
	obs_set_nix_platform_display(&second);
	assert_ptr_equal(obs_get_nix_platform_display(), &second);
	assert_int_equal(obs_get_nix_platform(), OBS_NIX_PLATFORM_WAYLAND);
	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
	assert_int_equal(obs_get_nix_platform(), OBS_NIX_PLATFORM_X11_EGL);
	assert_ptr_equal(obs_get_nix_platform_display(), &second);
	obs_set_nix_platform_display(NULL);
	assert_null(obs_get_nix_platform_display());
	assert_int_equal(first, 7);
	assert_int_equal(second, 9);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(platform_state)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
