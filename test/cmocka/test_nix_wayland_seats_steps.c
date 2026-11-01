#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-nix-wayland-seats.h"

/*
 * Characterization of the seat decisions of the Wayland hotkey platform
 * (libobs/obs-nix-wayland.c), moved to libobs/obs-nix-wayland-seats.c
 * before their Rust port:
 * - obs_nix_wayland_seats_bind_version(): platform_registry_handler()'s
 *   wl_seat version;
 * - obs_nix_wayland_seats_pick(): take_keyboard()'s seat;
 * - obs_nix_wayland_seats_lacks_keyboard(): the keyboard bit test of
 *   platform_seat_capabilities()'s release;
 * - obs_nix_wayland_seats_find_name(): platform_registry_remover()'s seat;
 * - obs_nix_wayland_seats_uses_release(): release_seat()'s choice.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No compositor is needed: the seats are
 * made up here, and their wl_seat pointers are never read.
 * test_wayland_multiseat covers the callers through the real platform.
 */

#define KEYBOARD OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD
/* WL_SEAT_CAPABILITY_POINTER and WL_SEAT_CAPABILITY_TOUCH. */
#define POINTER 1u
#define TOUCH 4u

static void bind_version(void **state)
{
	(void)state;

	/* Below version 4 the compositor is too old: nothing is bound. */
	assert_int_equal(obs_nix_wayland_seats_bind_version(0), 0);
	assert_int_equal(obs_nix_wayland_seats_bind_version(1), 0);
	assert_int_equal(obs_nix_wayland_seats_bind_version(2), 0);
	assert_int_equal(obs_nix_wayland_seats_bind_version(3), 0);

	/* From 4 to 7 the offered version is bound. */
	assert_int_equal(obs_nix_wayland_seats_bind_version(4), 4);
	assert_int_equal(obs_nix_wayland_seats_bind_version(5), 5);
	assert_int_equal(obs_nix_wayland_seats_bind_version(6), 6);
	assert_int_equal(obs_nix_wayland_seats_bind_version(7), 7);

	/* Newer versions are capped at 7. */
	assert_int_equal(obs_nix_wayland_seats_bind_version(8), 7);
	assert_int_equal(obs_nix_wayland_seats_bind_version(10), 7);
	assert_int_equal(obs_nix_wayland_seats_bind_version(0x7fffffffu), 7);
	assert_int_equal(obs_nix_wayland_seats_bind_version(0x80000000u), 7);
	assert_int_equal(obs_nix_wayland_seats_bind_version(UINT32_MAX), 7);
}

static void pick_no_seat(void **state)
{
	(void)state;

	/* An empty seat array is NULL (a DARRAY that never grew). */
	assert_true(obs_nix_wayland_seats_pick(NULL, 0) == SIZE_MAX);

	struct obs_nix_wayland_seat seats[] = {{NULL, 1, KEYBOARD}};
	assert_true(obs_nix_wayland_seats_pick(seats, 0) == SIZE_MAX);
}

static void pick_no_keyboard_seat(void **state)
{
	(void)state;

	struct obs_nix_wayland_seat seats[] = {
		{NULL, 1, 0},
		{NULL, 2, POINTER},
		{NULL, 3, TOUCH},
		{NULL, 4, POINTER | TOUCH},
		{NULL, 5, ~(uint32_t)KEYBOARD},
	};
	assert_true(obs_nix_wayland_seats_pick(seats, 5) == SIZE_MAX);
}

static void pick_first_keyboard_seat(void **state)
{
	(void)state;

	/* Two keyboard seats: the first wins, whatever its name. */
	struct obs_nix_wayland_seat seats[] = {
		{NULL, 9, POINTER},
		{NULL, 7, KEYBOARD},
		{NULL, 3, POINTER | KEYBOARD},
	};
	assert_true(obs_nix_wayland_seats_pick(seats, 3) == 1);

	/* Only the first num seats count. */
	assert_true(obs_nix_wayland_seats_pick(seats, 1) == SIZE_MAX);
	assert_true(obs_nix_wayland_seats_pick(seats, 2) == 1);
	assert_true(obs_nix_wayland_seats_pick(&seats[2], 1) == 0);

	/* Any capabilities with the keyboard bit, the first seat included. */
	struct obs_nix_wayland_seat all[] = {
		{NULL, 1, UINT32_MAX},
		{NULL, 2, KEYBOARD},
	};
	assert_true(obs_nix_wayland_seats_pick(all, 2) == 0);

	struct obs_nix_wayland_seat touch_keyboard[] = {
		{NULL, 1, TOUCH},
		{NULL, 2, TOUCH},
		{NULL, 3, TOUCH | KEYBOARD},
	};
	assert_true(obs_nix_wayland_seats_pick(touch_keyboard, 3) == 2);
}

static void lacks_keyboard(void **state)
{
	(void)state;

	/* No keyboard bit: the platform then compares the seat with the
	 * keyboard's, and releases the keyboard if it is that seat. */
	assert_true(obs_nix_wayland_seats_lacks_keyboard(0));
	assert_true(obs_nix_wayland_seats_lacks_keyboard(POINTER));
	assert_true(obs_nix_wayland_seats_lacks_keyboard(POINTER | TOUCH));
	assert_true(obs_nix_wayland_seats_lacks_keyboard(~(uint32_t)KEYBOARD));

	/* A keyboard: the platform keeps it, without comparing the seat. */
	assert_false(obs_nix_wayland_seats_lacks_keyboard(KEYBOARD));
	assert_false(obs_nix_wayland_seats_lacks_keyboard(POINTER | KEYBOARD));
	assert_false(obs_nix_wayland_seats_lacks_keyboard(UINT32_MAX));
}

static void capabilities_of_another_seat(void **state)
{
	(void)state;

	/* The platform's release is lacks_keyboard() && seat == keyboard_seat:
	 * a seat other than the keyboard's (or any seat while there is no
	 * keyboard) never releases it, keyboard or not. */
	const uint32_t caps[] = {0, POINTER, KEYBOARD, ~(uint32_t)KEYBOARD, UINT32_MAX};
	const bool lacks[] = {true, true, false, true, false};
	for (size_t i = 0; i < sizeof(caps) / sizeof(caps[0]); i++) {
		bool is_keyboard_seat = false;
		assert_true(obs_nix_wayland_seats_lacks_keyboard(caps[i]) == lacks[i]);
		assert_false(obs_nix_wayland_seats_lacks_keyboard(caps[i]) && is_keyboard_seat);
	}
}

static void find_name(void **state)
{
	(void)state;

	struct obs_nix_wayland_seat seats[] = {
		{NULL, 12, KEYBOARD},
		{NULL, 0, 0},
		{NULL, 30, POINTER},
		{NULL, UINT32_MAX, KEYBOARD},
	};
	assert_true(obs_nix_wayland_seats_find_name(seats, 4, 12) == 0);
	assert_true(obs_nix_wayland_seats_find_name(seats, 4, 0) == 1);
	assert_true(obs_nix_wayland_seats_find_name(seats, 4, 30) == 2);
	assert_true(obs_nix_wayland_seats_find_name(seats, 4, UINT32_MAX) == 3);

	/* Only the first num seats count. */
	assert_true(obs_nix_wayland_seats_find_name(seats, 3, UINT32_MAX) == SIZE_MAX);

	/* The name alone decides, not the capabilities. */
	struct obs_nix_wayland_seat same[] = {
		{NULL, 5, 0},
		{NULL, 5, KEYBOARD},
	};
	assert_true(obs_nix_wayland_seats_find_name(same, 2, 5) == 0);
	assert_true(obs_nix_wayland_seats_find_name(&same[1], 1, 5) == 0);
}

static void find_unknown_name(void **state)
{
	(void)state;

	/* A global the platform never bound (another interface, or a seat that
	 * was too old) is removed: nothing is dropped. */
	assert_true(obs_nix_wayland_seats_find_name(NULL, 0, 0) == SIZE_MAX);
	assert_true(obs_nix_wayland_seats_find_name(NULL, 0, 7) == SIZE_MAX);

	struct obs_nix_wayland_seat seats[] = {
		{NULL, 1, KEYBOARD},
		{NULL, 2, KEYBOARD},
	};
	assert_true(obs_nix_wayland_seats_find_name(seats, 2, 3) == SIZE_MAX);
	assert_true(obs_nix_wayland_seats_find_name(seats, 2, 0) == SIZE_MAX);
	assert_true(obs_nix_wayland_seats_find_name(seats, 2, UINT32_MAX) == SIZE_MAX);
}

static void uses_release(void **state)
{
	(void)state;

	/* Before wl_seat version 5 there is no release request. */
	assert_false(obs_nix_wayland_seats_uses_release(0));
	assert_false(obs_nix_wayland_seats_uses_release(1));
	assert_false(obs_nix_wayland_seats_uses_release(4));

	assert_true(obs_nix_wayland_seats_uses_release(5));
	assert_true(obs_nix_wayland_seats_uses_release(6));
	assert_true(obs_nix_wayland_seats_uses_release(7));
	assert_true(obs_nix_wayland_seats_uses_release(8));
	assert_true(obs_nix_wayland_seats_uses_release(UINT32_MAX));

	/* The versions bound are 4 to 7: only 4 is destroyed. */
	for (uint32_t offered = 4; offered < 12; offered++) {
		uint32_t bound = obs_nix_wayland_seats_bind_version(offered);
		assert_true(obs_nix_wayland_seats_uses_release(bound) == (bound >= 5));
	}
}

static void handover(void **state)
{
	(void)state;

	/* The platform's steps for two keyboard seats, the keyboard's seat
	 * losing its keyboard and then being removed, as obs-nix-wayland.c
	 * runs them: store the capabilities, release, take; or release,
	 * release the seat, erase, take. */
	struct obs_nix_wayland_seat seats[] = {
		{NULL, 21, KEYBOARD},
		{NULL, 22, POINTER | KEYBOARD},
	};
	size_t num = 2;
	size_t keyboard = obs_nix_wayland_seats_pick(seats, num);
	assert_true(keyboard == 0);

	/* Seat 21 keeps its keyboard: nothing changes. */
	assert_false(obs_nix_wayland_seats_lacks_keyboard(KEYBOARD) && keyboard == 0);

	/* Seat 22 loses its keyboard: not the keyboard's seat. */
	seats[1].capabilities = POINTER;
	assert_false(obs_nix_wayland_seats_lacks_keyboard(seats[1].capabilities) && keyboard == 1);
	seats[1].capabilities = POINTER | KEYBOARD;

	/* Seat 21 loses it: released, and seat 22 takes over. */
	seats[0].capabilities = 0;
	assert_true(obs_nix_wayland_seats_lacks_keyboard(seats[0].capabilities) && keyboard == 0);
	keyboard = obs_nix_wayland_seats_pick(seats, num);
	assert_true(keyboard == 1);

	/* Seat 22 is removed: it had the keyboard, and no seat is left with
	 * one. */
	size_t gone = obs_nix_wayland_seats_find_name(seats, num, 22);
	assert_true(gone == 1);
	assert_true(gone == keyboard);
	num--;
	assert_true(obs_nix_wayland_seats_pick(seats, num) == SIZE_MAX);

	/* Seat 21 gets a keyboard again and takes it. */
	seats[0].capabilities = KEYBOARD | TOUCH;
	assert_false(obs_nix_wayland_seats_lacks_keyboard(seats[0].capabilities));
	assert_true(obs_nix_wayland_seats_pick(seats, num) == 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(bind_version),
		cmocka_unit_test(pick_no_seat),
		cmocka_unit_test(pick_no_keyboard_seat),
		cmocka_unit_test(pick_first_keyboard_seat),
		cmocka_unit_test(lacks_keyboard),
		cmocka_unit_test(capabilities_of_another_seat),
		cmocka_unit_test(find_name),
		cmocka_unit_test(find_unknown_name),
		cmocka_unit_test(uses_release),
		cmocka_unit_test(handover),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
