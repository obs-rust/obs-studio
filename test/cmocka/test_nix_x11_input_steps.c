#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include "obs-nix-x11-input.h"

/*
 * Characterization of the X11 keysym table and mouse button answers of
 * libobs/obs-nix-x11.c, moved to libobs/obs-nix-x11-input.c before their
 * Rust port:
 * - obs_nix_x11_input_keysym(): get_keysym();
 * - obs_nix_x11_input_mouse_pressed(): mouse_button_pressed()'s answer with
 *   XInput, after the raw event drain;
 * - obs_nix_x11_input_commit(): its commit of the drain;
 * - obs_nix_x11_input_mask_pressed(): its answer without XInput, on the
 *   QueryPointer reply's mask.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. No X server is needed: the arrays a drain
 * fills are set by hand here (press: pressed and update; release: update).
 * test_x11_volume_keys and test_x11_keymap cover the keysym table through
 * the real libobs on an X server.
 */

/* Some X11 keysyms (X11/keysymdef.h, X11/XF86keysym.h, X11/Sunkeysym.h). */
#define XK_Return 0xff0d
#define XK_space 0x20
#define XK_1 0x31
#define XK_A 0x41
#define XK_grave 0x60
#define XK_cedilla 0xb8
#define XK_Shift_L 0xffe1
#define XK_Control_L 0xffe3
#define XK_Alt_L 0xffe9
#define XK_F1 0xffbe
#define XK_KP_0 0xffb0
#define XK_KP_Multiply 0xffaa
#define XK_Kanji 0xff21
#define XK_dead_grave 0xfe50
#define XF86XK_AudioPlay 0x1008ff14
#define XF86XK_AudioRaiseVolume 0x1008ff13
#define XF86XK_Open 0x1008ff6b
#define SunXK_Front 0x1005ff71

static bool pressed[XINPUT_MOUSE_LEN];
static bool update[XINPUT_MOUSE_LEN];
static bool held[XINPUT_MOUSE_LEN];

static int setup(void **state)
{
	(void)state;
	memset(pressed, 0, sizeof(pressed));
	memset(update, 0, sizeof(update));
	memset(held, 0, sizeof(held));
	return 0;
}

/* The start of a poll: obs-nix-x11.c clears pressed and update. */
static void new_poll(void)
{
	memset(pressed, 0, sizeof(pressed));
	memset(update, 0, sizeof(update));
}

/* What the drain stores for a raw button event of button n (1-based). */
static void press(int n)
{
	pressed[n - 1] = true;
	update[n - 1] = true;
}

static void release(int n)
{
	update[n - 1] = true;
}

static bool answer(obs_key_t key)
{
	return obs_nix_x11_input_mouse_pressed(key, pressed, held);
}

/* The keysym of a sample of keys, one or two per part of the table. */
static void keysym_table(void **state)
{
	(void)state;
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_RETURN), XK_Return);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_SHIFT), XK_Shift_L);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_CONTROL), XK_Control_L);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_ALT), XK_Alt_L);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_F1), XK_F1);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_SPACE), XK_space);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_1), XK_1);
	/* The upper-case keysym, though the key types lower case. */
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_A), XK_A);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_NUM0), XK_KP_0);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_NUMASTERISK), XK_KP_Multiply);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_KANJI), XK_Kanji);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_DEAD_GRAVE), XK_dead_grave);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_OPEN), XF86XK_Open);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_FRONT), SunXK_Front);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_VK_MEDIA_PLAY_PAUSE), XF86XK_AudioPlay);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_VK_VOLUME_UP), XF86XK_AudioRaiseVolume);
}

/* Two keys that share a keysym (characterized, not endorsed: a keysym lookup
 * of the base table finds only the first). */
static void keysym_shared(void **state)
{
	(void)state;
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_QUOTELEFT), XK_grave);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_ASCIITILDE), XK_grave);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_CEDILLA), XK_cedilla);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_CCEDILLA), XK_cedilla);
}

/* Mouse buttons 1 to 5 are their number shifted left by 16; 6 to 29 have
 * none. */
static void keysym_mouse(void **state)
{
	(void)state;
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_MOUSE1), 1 << 16);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_MOUSE2), 2 << 16);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_MOUSE3), 3 << 16);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_MOUSE4), 4 << 16);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_MOUSE5), 5 << 16);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_MOUSE6), 0);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_MOUSE29), 0);
}

/* A key the table doesn't name has 0, and so does any value that isn't a
 * key. */
static void keysym_none(void **state)
{
	(void)state;
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_NONE), 0);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_META), 0);
	assert_int_equal(obs_nix_x11_input_keysym(OBS_KEY_LAST_VALUE), 0);
	assert_int_equal(obs_nix_x11_input_keysym((obs_key_t)-1), 0);
}

/* The slot of each obs mouse key: Mouse 1 reads button 1, Mouse 2 the right
 * button (3), Mouse 3 the middle button (2), Mouse 4 to 29 buttons 8 to 33;
 * buttons 4 to 7, the wheel, have no key. */
static void slots(void **state)
{
	(void)state;
	const struct {
		obs_key_t key;
		int slot;
	} map[] = {
		{OBS_KEY_MOUSE1, 0},   {OBS_KEY_MOUSE2, 2},   {OBS_KEY_MOUSE3, 1},   {OBS_KEY_MOUSE4, 7},
		{OBS_KEY_MOUSE5, 8},   {OBS_KEY_MOUSE6, 9},   {OBS_KEY_MOUSE7, 10},  {OBS_KEY_MOUSE8, 11},
		{OBS_KEY_MOUSE9, 12},  {OBS_KEY_MOUSE10, 13}, {OBS_KEY_MOUSE11, 14}, {OBS_KEY_MOUSE12, 15},
		{OBS_KEY_MOUSE13, 16}, {OBS_KEY_MOUSE14, 17}, {OBS_KEY_MOUSE15, 18}, {OBS_KEY_MOUSE16, 19},
		{OBS_KEY_MOUSE17, 20}, {OBS_KEY_MOUSE18, 21}, {OBS_KEY_MOUSE19, 22}, {OBS_KEY_MOUSE20, 23},
		{OBS_KEY_MOUSE21, 24}, {OBS_KEY_MOUSE22, 25}, {OBS_KEY_MOUSE23, 26}, {OBS_KEY_MOUSE24, 27},
		{OBS_KEY_MOUSE25, 28}, {OBS_KEY_MOUSE26, 29}, {OBS_KEY_MOUSE27, 30}, {OBS_KEY_MOUSE28, 31},
		{OBS_KEY_MOUSE29, 32},
	};

	for (size_t i = 0; i < sizeof(map) / sizeof(map[0]); i++) {
		for (int slot = 0; slot < XINPUT_MOUSE_LEN; slot++) {
			memset(pressed, 0, sizeof(pressed));
			memset(held, 0, sizeof(held));
			pressed[slot] = true;
			assert_int_equal(answer(map[i].key), slot == map[i].slot);
			pressed[slot] = false;
			held[slot] = true;
			assert_int_equal(answer(map[i].key), slot == map[i].slot);
		}
	}
}

/* Mouse 29 reads slot 32, which the drain never fills: it takes buttons 1 to
 * 32 into slots 0 to 31 (characterized, not endorsed: Mouse 29 never reads as
 * pressed with XInput). */
static void mouse29_unfilled(void **state)
{
	(void)state;
	for (int n = 1; n < XINPUT_MOUSE_LEN; n++)
		press(n);
	assert_false(answer(OBS_KEY_MOUSE29));
	obs_nix_x11_input_commit(update, pressed, held);
	new_poll();
	assert_false(answer(OBS_KEY_MOUSE29));
	assert_true(answer(OBS_KEY_MOUSE28));
}

/* Any other key is never pressed, whatever the arrays hold. */
static void other_keys(void **state)
{
	(void)state;
	memset(pressed, 1, sizeof(pressed));
	memset(held, 1, sizeof(held));
	assert_false(answer(OBS_KEY_NONE));
	assert_false(answer(OBS_KEY_A));
	assert_false(answer(OBS_KEY_LAST_VALUE));
	assert_false(answer((obs_key_t)-1));
}

/* A press in one poll and its release in a later one: pressed in the poll of
 * the press, held in the polls between, still pressed in the poll of the
 * release (the answer comes before the commit), released after it. */
static void press_then_release(void **state)
{
	(void)state;
	press(1);
	assert_true(answer(OBS_KEY_MOUSE1));
	obs_nix_x11_input_commit(update, pressed, held);
	assert_true(held[0]);

	new_poll();
	assert_true(answer(OBS_KEY_MOUSE1));
	obs_nix_x11_input_commit(update, pressed, held);
	assert_true(held[0]);

	new_poll();
	release(1);
	assert_true(answer(OBS_KEY_MOUSE1));
	obs_nix_x11_input_commit(update, pressed, held);
	assert_false(held[0]);

	new_poll();
	assert_false(answer(OBS_KEY_MOUSE1));
}

/* A release with no press before it: never pressed, nothing held. */
static void release_without_press(void **state)
{
	(void)state;
	release(3);
	assert_false(answer(OBS_KEY_MOUSE2));
	obs_nix_x11_input_commit(update, pressed, held);
	assert_false(held[2]);
	new_poll();
	assert_false(answer(OBS_KEY_MOUSE2));
}

/* The commit: an updated slot takes this poll's pressed state, any other slot
 * keeps its held state; pressed and update are left as they are. */
static void commit(void **state)
{
	(void)state;
	held[0] = true;  /* updated, released: false */
	held[1] = true;  /* not updated: stays */
	held[2] = false; /* updated, pressed: true */
	held[3] = false; /* not updated, pressed without update: stays false */
	update[0] = true;
	update[2] = true;
	pressed[2] = true;
	pressed[3] = true;
	held[XINPUT_MOUSE_LEN - 1] = true;
	update[XINPUT_MOUSE_LEN - 1] = true;

	obs_nix_x11_input_commit(update, pressed, held);

	assert_false(held[0]);
	assert_true(held[1]);
	assert_true(held[2]);
	assert_false(held[3]);
	assert_false(held[XINPUT_MOUSE_LEN - 1]);
	assert_true(update[0]);
	assert_true(pressed[3]);
	assert_false(update[3]);
}

/* Without XInput: Mouse 1 is button 1's mask bit, Mouse 2 button 3's and
 * Mouse 3 button 2's; every other key, Mouse 4 included, is never pressed. */
static void mask(void **state)
{
	(void)state;
	assert_true(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE1, OBS_NIX_X11_INPUT_BUTTON_MASK_1));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE1, OBS_NIX_X11_INPUT_BUTTON_MASK_2));
	assert_true(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE2, OBS_NIX_X11_INPUT_BUTTON_MASK_3));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE2, OBS_NIX_X11_INPUT_BUTTON_MASK_2));
	assert_true(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE3, OBS_NIX_X11_INPUT_BUTTON_MASK_2));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE3, OBS_NIX_X11_INPUT_BUTTON_MASK_3));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE1, 0));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE1, 0xfeff));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_MOUSE4, 0xffff));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_A, 0xffff));
	assert_false(obs_nix_x11_input_mask_pressed(OBS_KEY_NONE, 0xffff));
	/* The pointer masks are X's (xcb/xproto.h); obs-nix-x11.c checks them. */
	assert_int_equal(OBS_NIX_X11_INPUT_BUTTON_MASK_1, 0x100);
	assert_int_equal(OBS_NIX_X11_INPUT_BUTTON_MASK_2, 0x200);
	assert_int_equal(OBS_NIX_X11_INPUT_BUTTON_MASK_3, 0x400);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(keysym_table),
		cmocka_unit_test(keysym_shared),
		cmocka_unit_test(keysym_mouse),
		cmocka_unit_test(keysym_none),
		cmocka_unit_test_setup(slots, setup),
		cmocka_unit_test_setup(mouse29_unfilled, setup),
		cmocka_unit_test_setup(other_keys, setup),
		cmocka_unit_test_setup(press_then_release, setup),
		cmocka_unit_test_setup(release_without_press, setup),
		cmocka_unit_test_setup(commit, setup),
		cmocka_unit_test(mask),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
