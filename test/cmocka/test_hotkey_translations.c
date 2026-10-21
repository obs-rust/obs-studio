/* Characterization of the hotkey key-name translations in
 * libobs/obs-hotkey-translations.c before their port to the obs-hotkey crate.
 *
 * obs_hotkeys_set_translations_s() copies min(size, sizeof) bytes of the
 * frontend's struct obs_hotkeys_translations, stores each named key's
 * translated name, and expands the numpad and mouse templates ("Numpad %1",
 * "Mouse %1") into the names of NUM0-NUM9 and MOUSE1-MOUSE29: the template is
 * depadded, " %1" is appended when it has no "%1", and every "%1" is
 * replaced by the digits.
 *
 * This test links the real libobs (obs_startup) and goes only through the
 * public API: it sets translations with obs_hotkeys_set_translations_s() and
 * reads them back through obs_key_to_str() and obs_key_combination_to_str().
 * On Linux and FreeBSD (X11 and Wayland) those show the stored name of every
 * key the struct names; Windows and macOS show only some of them, so the
 * test is built for Linux and FreeBSD only. Run under xvfb-run: obs_startup
 * needs a display there.
 *
 * The translations live in one store for the whole process and can't be
 * cleared through the API, so the cases run in order and each sets what it
 * checks first; the defaults cases come first.
 *
 * An empty or padding-only numpad or mouse template ("" or " ") is skipped
 * like a NULL one, so the keys keep their names. It used to leave the
 * template string NULL, and the expansion then called strstr(NULL, "%1"),
 * which crashed; the cases that set one do it in a forked child first, so a
 * crash fails that case instead of the whole binary.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs.h>
#include <util/dstr.h>

static int obs_up;

/* ------------------------------------------------------------------------- */

static void set(struct obs_hotkeys_translations *t)
{
	obs_hotkeys_set_translations_s(t, sizeof(*t));
}

/* Set in a forked child first: if the set crashes there, fail this case
 * instead of the whole binary, and leave the parent's store as it was. The
 * child restores the default crash signal actions, so cmocka's handlers
 * don't go on running the next cases in it. */
static void set_checked(struct obs_hotkeys_translations *t)
{
	fflush(NULL);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		const int crash_signals[] = {SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGABRT};
		for (size_t i = 0; i < sizeof(crash_signals) / sizeof(crash_signals[0]); i++)
			signal(crash_signals[i], SIG_DFL);
		struct rlimit no_core = {0, 0};
		setrlimit(RLIMIT_CORE, &no_core);
		set(t);
		_exit(0);
	}

	int status = 0;
	assert_int_equal(waitpid(pid, &status, 0), pid);
	if (WIFSIGNALED(status))
		fprintf(stderr, "obs_hotkeys_set_translations_s crashed: signal %d\n", WTERMSIG(status));
	assert_true(WIFEXITED(status));
	assert_int_equal(WEXITSTATUS(status), 0);

	set(t);
}

static void assert_key(obs_key_t key, const char *expected)
{
	struct dstr str = {0};
	obs_key_to_str(key, &str);
	const char *got = str.array ? str.array : "";
	if (strcmp(got, expected) != 0)
		fprintf(stderr, "%s: got \"%s\", expected \"%s\"\n", obs_key_to_name(key), got, expected);
	assert_string_equal(got, expected);
	dstr_free(&str);
}

static void assert_numpad_template(const char *tmpl, const char *num0, const char *num5, const char *num9)
{
	struct obs_hotkeys_translations t = {0};
	t.numpad_num = tmpl;
	set(&t);
	assert_key(OBS_KEY_NUM0, num0);
	assert_key(OBS_KEY_NUM5, num5);
	assert_key(OBS_KEY_NUM9, num9);
}

static void assert_mouse_template(const char *tmpl, const char *mouse1, const char *mouse10, const char *mouse29)
{
	struct obs_hotkeys_translations t = {0};
	t.mouse_num = tmpl;
	set(&t);
	assert_key(OBS_KEY_MOUSE1, mouse1);
	assert_key(OBS_KEY_MOUSE10, mouse10);
	assert_key(OBS_KEY_MOUSE29, mouse29);
}

/* Every member set, to "<prefix> <member>" (templates to "<prefix> num %1"
 * and "<prefix> mouse %1"). The strings live in `buf`. */
#define MEMBERS(X)                                                                                                    \
	X(insert)                                                                                                     \
	X(del)                                                                                                        \
	X(home)                                                                                                       \
	X(end)                                                                                                        \
	X(page_up)                                                                                                    \
	X(page_down)                                                                                                  \
	X(num_lock)                                                                                                   \
	X(scroll_lock)                                                                                                \
	X(caps_lock)                                                                                                  \
	X(backspace)                                                                                                  \
	X(tab)                                                                                                        \
	X(print)                                                                                                      \
	X(pause)                                                                                                      \
	X(left)                                                                                                       \
	X(right)                                                                                                      \
	X(up)                                                                                                         \
	X(down)                                                                                                       \
	X(shift)                                                                                                      \
	X(alt)                                                                                                        \
	X(control)                                                                                                    \
	X(meta)                                                                                                       \
	X(menu)                                                                                                       \
	X(space)                                                                                                      \
	X(numpad_num)                                                                                                 \
	X(numpad_divide)                                                                                              \
	X(numpad_multiply)                                                                                            \
	X(numpad_minus)                                                                                               \
	X(numpad_plus)                                                                                                \
	X(numpad_decimal)                                                                                             \
	X(apple_keypad_num)                                                                                           \
	X(apple_keypad_divide)                                                                                        \
	X(apple_keypad_multiply)                                                                                      \
	X(apple_keypad_minus)                                                                                         \
	X(apple_keypad_plus)                                                                                          \
	X(apple_keypad_decimal)                                                                                       \
	X(apple_keypad_equal)                                                                                         \
	X(mouse_num)                                                                                                  \
	X(escape)

#define COUNT_MEMBER(m) +1
enum { MEMBER_COUNT = 0 MEMBERS(COUNT_MEMBER) };

struct filled {
	struct obs_hotkeys_translations t;
	char buf[MEMBER_COUNT][64];
};

static void fill(struct filled *f, const char *prefix)
{
	size_t i = 0;
	memset(f, 0, sizeof(*f));
#define FILL_MEMBER(m)                                                              \
	if (strcmp(#m, "numpad_num") == 0 || strcmp(#m, "apple_keypad_num") == 0)   \
		snprintf(f->buf[i], sizeof(f->buf[i]), "%s %s %%1", prefix, #m);    \
	else if (strcmp(#m, "mouse_num") == 0)                                      \
		snprintf(f->buf[i], sizeof(f->buf[i]), "%s mouse %%1", prefix);     \
	else                                                                        \
		snprintf(f->buf[i], sizeof(f->buf[i]), "%s %s", prefix, #m);        \
	f->t.m = f->buf[i];                                                         \
	i++;
	MEMBERS(FILL_MEMBER)
#undef FILL_MEMBER
	assert_int_equal(i, MEMBER_COUNT);
}

/* The named keys the frontend translates on Linux, with the member that sets
 * each. Left, right, up and down have their own case: arrow_keys_translated. */
static const struct {
	obs_key_t key;
	const char *member;
} named_keys[] = {
	{OBS_KEY_INSERT, "insert"},
	{OBS_KEY_DELETE, "del"},
	{OBS_KEY_HOME, "home"},
	{OBS_KEY_END, "end"},
	{OBS_KEY_PAGEUP, "page_up"},
	{OBS_KEY_PAGEDOWN, "page_down"},
	{OBS_KEY_NUMLOCK, "num_lock"},
	{OBS_KEY_SCROLLLOCK, "scroll_lock"},
	{OBS_KEY_CAPSLOCK, "caps_lock"},
	{OBS_KEY_BACKSPACE, "backspace"},
	{OBS_KEY_TAB, "tab"},
	{OBS_KEY_PRINT, "print"},
	{OBS_KEY_PAUSE, "pause"},
	{OBS_KEY_SHIFT, "shift"},
	{OBS_KEY_ALT, "alt"},
	{OBS_KEY_CONTROL, "control"},
	{OBS_KEY_META, "meta"},
	{OBS_KEY_MENU, "menu"},
	{OBS_KEY_SPACE, "space"},
	{OBS_KEY_ESCAPE, "escape"},
	{OBS_KEY_NUMSLASH, "numpad_divide"},
	{OBS_KEY_NUMASTERISK, "numpad_multiply"},
	{OBS_KEY_NUMMINUS, "numpad_minus"},
	{OBS_KEY_NUMPLUS, "numpad_plus"},
	{OBS_KEY_NUMPERIOD, "numpad_decimal"},
};

#define NAMED_KEY_COUNT (sizeof(named_keys) / sizeof(named_keys[0]))

static void assert_named_keys(const char *prefix)
{
	for (size_t i = 0; i < NAMED_KEY_COUNT; i++) {
		char want[64];
		snprintf(want, sizeof(want), "%s %s", prefix, named_keys[i].member);
		assert_key(named_keys[i].key, want);
	}
}

static void assert_numbered_keys(const char *prefix)
{
	char want[64];
	for (int i = 0; i <= 9; i++) {
		snprintf(want, sizeof(want), "%s numpad_num %d", prefix, i);
		assert_key(OBS_KEY_NUM0 + i, want);
	}
	for (int i = 1; i <= 29; i++) {
		snprintf(want, sizeof(want), "%s mouse %d", prefix, i);
		assert_key(OBS_KEY_MOUSE1 + i - 1, want);
	}
}

/* ------------------------------------------------------------------------- */

/* Before any translation is set, the platform's English names. */
static void defaults_before_any_set(void **state)
{
	(void)state;
	assert_key(OBS_KEY_INSERT, "Insert");
	assert_key(OBS_KEY_PAGEUP, "Page Up");
	assert_key(OBS_KEY_LEFT, "Left");
	assert_key(OBS_KEY_META, "Super");
	assert_key(OBS_KEY_ESCAPE, "Escape");
	assert_key(OBS_KEY_NUMSLASH, "Numpad /");
	assert_key(OBS_KEY_NUM0, "Numpad 0");
	assert_key(OBS_KEY_NUM9, "Numpad 9");
	assert_key(OBS_KEY_MOUSE1, "Mouse 1");
	assert_key(OBS_KEY_MOUSE29, "Mouse 29");
}

/* Every member set: each named key gets its name, NUM0-NUM9 and
 * MOUSE1-MOUSE29 get the expanded templates. */
static void every_member(void **state)
{
	(void)state;
	struct filled f;
	fill(&f, "A");
	set(&f.t);
	assert_named_keys("A");
	assert_numbered_keys("A");
}

/* A second set replaces every name it sets. */
static void set_twice_replaces(void **state)
{
	(void)state;
	struct filled f;
	fill(&f, "B1");
	set(&f.t);
	fill(&f, "B2");
	set(&f.t);
	assert_named_keys("B2");
	assert_numbered_keys("B2");
}

/* A NULL member leaves that key's name as it was: a set never clears one. */
static void null_member_keeps_name(void **state)
{
	(void)state;
	struct filled f;
	fill(&f, "C");
	set(&f.t);

	struct obs_hotkeys_translations t = {0};
	t.del = "C2 del";
	set(&t);
	assert_key(OBS_KEY_DELETE, "C2 del");
	assert_key(OBS_KEY_INSERT, "C insert");
	assert_key(OBS_KEY_ESCAPE, "C escape");
	assert_key(OBS_KEY_NUMSLASH, "C numpad_divide");
	assert_key(OBS_KEY_NUM3, "C numpad_num 3");
	assert_key(OBS_KEY_MOUSE3, "C mouse 3");

	/* All NULL changes nothing. */
	struct obs_hotkeys_translations none = {0};
	set(&none);
	assert_key(OBS_KEY_DELETE, "C2 del");
	assert_key(OBS_KEY_NUM3, "C numpad_num 3");
	assert_key(OBS_KEY_MOUSE3, "C mouse 3");
}

/* A NULL struct changes nothing. */
static void null_struct(void **state)
{
	(void)state;
	struct filled f;
	fill(&f, "D");
	set(&f.t);
	obs_hotkeys_set_translations_s(NULL, sizeof(struct obs_hotkeys_translations));
	obs_hotkeys_set_translations_s(NULL, 0);
	assert_named_keys("D");
	assert_numbered_keys("D");
}

/* Only min(size, sizeof) bytes are read, so a struct from an older, shorter
 * header reads as NULL tails, which keep their names; a size of 0 reads
 * nothing. */
static void short_size(void **state)
{
	(void)state;
	struct filled f;
	fill(&f, "E1");
	set(&f.t);

	fill(&f, "E2");
	obs_hotkeys_set_translations_s(&f.t, 0);
	assert_named_keys("E1");
	assert_numbered_keys("E1");

	/* insert .. pause */
	obs_hotkeys_set_translations_s(&f.t, offsetof(struct obs_hotkeys_translations, left));
	assert_key(OBS_KEY_INSERT, "E2 insert");
	assert_key(OBS_KEY_PAUSE, "E2 pause");
	assert_key(OBS_KEY_SHIFT, "E1 shift");
	assert_key(OBS_KEY_ESCAPE, "E1 escape");
	assert_key(OBS_KEY_NUMSLASH, "E1 numpad_divide");
	assert_key(OBS_KEY_NUM1, "E1 numpad_num 1");
	assert_key(OBS_KEY_MOUSE1, "E1 mouse 1");

	/* .. apple_keypad_equal: the numpad, not the mouse or escape */
	obs_hotkeys_set_translations_s(&f.t, offsetof(struct obs_hotkeys_translations, mouse_num));
	assert_key(OBS_KEY_SHIFT, "E2 shift");
	assert_key(OBS_KEY_NUMSLASH, "E2 numpad_divide");
	assert_key(OBS_KEY_NUM1, "E2 numpad_num 1");
	assert_key(OBS_KEY_MOUSE1, "E1 mouse 1");
	assert_key(OBS_KEY_ESCAPE, "E1 escape");

	/* .. mouse_num: everything but escape, the last member */
	obs_hotkeys_set_translations_s(&f.t, offsetof(struct obs_hotkeys_translations, escape));
	assert_key(OBS_KEY_MOUSE1, "E2 mouse 1");
	assert_key(OBS_KEY_ESCAPE, "E1 escape");
}

/* A size larger than the struct reads only the struct. */
static void larger_size(void **state)
{
	(void)state;
	struct {
		struct filled f;
		char tail[64];
	} big;
	fill(&big.f, "F");
	memset(big.tail, 0xff, sizeof(big.tail));
	obs_hotkeys_set_translations_s(&big.f.t, sizeof(big));
	assert_named_keys("F");
	assert_numbered_keys("F");
}

/* An empty name is stored as is, so the key shows as nothing, and a
 * combination leaves it out. */
static void empty_named_key(void **state)
{
	(void)state;
	struct obs_hotkeys_translations t = {0};
	t.insert = "";
	t.shift = "";
	t.numpad_plus = "";
	t.control = "Ctl";
	set(&t);
	assert_key(OBS_KEY_INSERT, "");
	assert_key(OBS_KEY_SHIFT, "");
	assert_key(OBS_KEY_NUMPLUS, "");

	struct dstr str = {0};
	obs_key_combination_t combo = {INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_INSERT};
	obs_key_combination_to_str(combo, &str);
	assert_string_equal(str.array, "Ctl");
	dstr_free(&str);

	t.insert = "Ins";
	t.shift = "Shf";
	t.numpad_plus = "N+";
	set(&t);
	assert_key(OBS_KEY_INSERT, "Ins");
}

/* The numpad template: depadded (only ' ', '\t', '\n' and '\r'), " %1"
 * appended when there is no "%1", then every "%1" replaced, left to right,
 * without rescanning. */
static void numpad_templates(void **state)
{
	(void)state;
	assert_numpad_template("Numpad %1", "Numpad 0", "Numpad 5", "Numpad 9");
	assert_numpad_template("%1 (Keypad)", "0 (Keypad)", "5 (Keypad)", "9 (Keypad)");
	assert_numpad_template("Pad", "Pad 0", "Pad 5", "Pad 9");
	assert_numpad_template("%1", "0", "5", "9");
	assert_numpad_template("%1-%1", "0-0", "5-5", "9-9");
	assert_numpad_template("%1%1", "00", "55", "99");
	assert_numpad_template("  Pad %1\t", "Pad 0", "Pad 5", "Pad 9");
	assert_numpad_template(" \t\r\nPad\r\n", "Pad 0", "Pad 5", "Pad 9");
	assert_numpad_template("  %1  ", "0", "5", "9");
	assert_numpad_template("\vPad\v", "\vPad\v 0", "\vPad\v 5", "\vPad\v 9");
	assert_numpad_template("Pad  %1", "Pad  0", "Pad  5", "Pad  9");
	assert_numpad_template("%%1", "%0", "%5", "%9");
	assert_numpad_template("%11", "01", "51", "91");
	assert_numpad_template("%2", "%2 0", "%2 5", "%2 9");
	assert_numpad_template("% 1", "% 1 0", "% 1 5", "% 1 9");
	assert_numpad_template("100%", "100% 0", "100% 5", "100% 9");
	assert_numpad_template("%d %s", "%d %s 0", "%d %s 5", "%d %s 9");
	assert_numpad_template("Ziffernblock %1", "Ziffernblock 0", "Ziffernblock 5", "Ziffernblock 9");
	assert_numpad_template("\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc %1",
			       "\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc 0",
			       "\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc 5",
			       "\xe3\x83\x86\xe3\x83\xb3\xe3\x82\xad\xe3\x83\xbc 9");
	assert_numpad_template("\xff\xfe %1", "\xff\xfe 0", "\xff\xfe 5", "\xff\xfe 9");
	assert_numpad_template("x", "x 0", "x 5", "x 9");
}

/* The mouse template: the same rules, with 1 to 29. */
static void mouse_templates(void **state)
{
	(void)state;
	assert_mouse_template("Mouse %1", "Mouse 1", "Mouse 10", "Mouse 29");
	assert_mouse_template("Maus", "Maus 1", "Maus 10", "Maus 29");
	assert_mouse_template("%1", "1", "10", "29");
	assert_mouse_template("Button %1 (%1)", "Button 1 (1)", "Button 10 (10)", "Button 29 (29)");
	assert_mouse_template("%1%1", "11", "1010", "2929");
	assert_mouse_template("\tM %1 \n", "M 1", "M 10", "M 29");
	assert_mouse_template("%11", "11", "101", "291");
	assert_mouse_template("%%1", "%1", "%10", "%29");
	assert_mouse_template("%2", "%2 1", "%2 10", "%2 29");
	assert_mouse_template("\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 %1",
			      "\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 1",
			      "\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 10",
			      "\xd8\xa7\xd9\x84\xd9\x81\xd8\xa3\xd8\xb1\xd8\xa9 29");
}

/* The two templates are independent: setting one leaves the other's names. */
static void templates_independent(void **state)
{
	(void)state;
	struct obs_hotkeys_translations t = {0};
	t.numpad_num = "G num %1";
	t.mouse_num = "G mouse %1";
	set(&t);

	struct obs_hotkeys_translations n = {0};
	n.numpad_num = "G2 num %1";
	set(&n);
	assert_key(OBS_KEY_NUM4, "G2 num 4");
	assert_key(OBS_KEY_MOUSE4, "G mouse 4");

	struct obs_hotkeys_translations m = {0};
	m.mouse_num = "G2 mouse %1";
	set(&m);
	assert_key(OBS_KEY_NUM4, "G2 num 4");
	assert_key(OBS_KEY_MOUSE4, "G2 mouse 4");
}

/* The apple_* members are for macOS only; elsewhere they change nothing. */
static void apple_members_ignored(void **state)
{
	(void)state;
	struct filled f;
	fill(&f, "H");
	set(&f.t);

	struct obs_hotkeys_translations t = {0};
	t.apple_keypad_num = "H2 apple %1";
	t.apple_keypad_divide = "H2 apple divide";
	t.apple_keypad_multiply = "H2 apple multiply";
	t.apple_keypad_minus = "H2 apple minus";
	t.apple_keypad_plus = "H2 apple plus";
	t.apple_keypad_decimal = "H2 apple decimal";
	t.apple_keypad_equal = "H2 apple equal";
	set(&t);
	assert_named_keys("H");
	assert_numbered_keys("H");
}

/* A combination shows the translated modifiers and key, in the order
 * control, meta, alt, shift, key. */
static void combination_string(void **state)
{
	(void)state;
	struct obs_hotkeys_translations t = {0};
	t.control = "Strg";
	t.meta = "Super-Taste";
	t.alt = "Alt Gr";
	t.shift = "Umschalt";
	t.numpad_num = "Ziffernblock %1";
	t.mouse_num = "Maustaste %1";
	set(&t);

	struct dstr str = {0};
	obs_key_combination_t combo = {INTERACT_SHIFT_KEY | INTERACT_CONTROL_KEY, OBS_KEY_NUM5};
	obs_key_combination_to_str(combo, &str);
	assert_string_equal(str.array, "Strg + Umschalt + Ziffernblock 5");
	dstr_free(&str);

	combo.modifiers = INTERACT_SHIFT_KEY | INTERACT_CONTROL_KEY | INTERACT_ALT_KEY | INTERACT_COMMAND_KEY;
	combo.key = OBS_KEY_MOUSE4;
	obs_key_combination_to_str(combo, &str);
	assert_string_equal(str.array, "Strg + Super-Taste + Alt Gr + Umschalt + Maustaste 4");
	dstr_free(&str);
}

/* The arrow keys take their names from left, right, up and down, like the
 * other named keys. */
static void arrow_keys_translated(void **state)
{
	(void)state;
	struct obs_hotkeys_translations t = {0};
	t.left = "Links";
	t.right = "Rechts";
	t.up = "Hoch";
	t.down = "Runter";
	set(&t);
	assert_key(OBS_KEY_LEFT, "Links");
	assert_key(OBS_KEY_RIGHT, "Rechts");
	assert_key(OBS_KEY_UP, "Hoch");
	assert_key(OBS_KEY_DOWN, "Runter");
}

/* An empty or padding-only template, before any other set: skipped like a
 * NULL one, so NUM0-NUM9 and MOUSE1-MOUSE29 keep the English defaults. */
static void empty_templates_keep_defaults(void **state)
{
	(void)state;
	const char *empties[] = {"", " ", " \t\r\n "};
	for (size_t i = 0; i < sizeof(empties) / sizeof(empties[0]); i++) {
		struct obs_hotkeys_translations t = {0};
		t.numpad_num = empties[i];
		t.mouse_num = empties[i];
		set_checked(&t);
		assert_key(OBS_KEY_NUM0, "Numpad 0");
		assert_key(OBS_KEY_NUM9, "Numpad 9");
		assert_key(OBS_KEY_MOUSE1, "Mouse 1");
		assert_key(OBS_KEY_MOUSE29, "Mouse 29");
	}
}

/* An empty or padding-only template after a set keeps the names it set; the
 * other template and the named keys in the same struct still apply. */
static void empty_templates_keep_names(void **state)
{
	(void)state;
	struct obs_hotkeys_translations t = {0};
	t.numpad_num = "K num %1";
	t.mouse_num = "K mouse %1";
	set(&t);

	const char *empties[] = {"", "  ", "\t", "\r\n"};
	for (size_t i = 0; i < sizeof(empties) / sizeof(empties[0]); i++) {
		char num[32], mouse[32], ins[32];
		snprintf(num, sizeof(num), "K%zu num %%1", i);
		snprintf(mouse, sizeof(mouse), "K%zu mouse %%1", i);
		snprintf(ins, sizeof(ins), "K%zu insert", i);

		struct obs_hotkeys_translations n = {0};
		n.numpad_num = empties[i];
		n.mouse_num = mouse;
		n.insert = ins;
		set_checked(&n);
		snprintf(mouse, sizeof(mouse), "K%zu mouse 7", i);
		assert_key(OBS_KEY_MOUSE7, mouse);
		assert_key(OBS_KEY_INSERT, ins);
		/* The numpad names of the previous round. */
		char prev[32] = "K num 7";
		if (i > 0)
			snprintf(prev, sizeof(prev), "K%zu num 7", i - 1);
		assert_key(OBS_KEY_NUM7, prev);

		struct obs_hotkeys_translations m = {0};
		m.numpad_num = num;
		m.mouse_num = empties[i];
		set_checked(&m);
		snprintf(num, sizeof(num), "K%zu num 7", i);
		assert_key(OBS_KEY_NUM7, num);
		snprintf(mouse, sizeof(mouse), "K%zu mouse 7", i);
		assert_key(OBS_KEY_MOUSE7, mouse);
	}
}

/* ------------------------------------------------------------------------- */

int main(void)
{
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		puts("obs_startup failed, skipping");
		return 0;
	}

	/* In this order: the defaults cases must run before any set. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(defaults_before_any_set),
		cmocka_unit_test(empty_templates_keep_defaults),
		cmocka_unit_test(every_member),
		cmocka_unit_test(set_twice_replaces),
		cmocka_unit_test(null_member_keeps_name),
		cmocka_unit_test(null_struct),
		cmocka_unit_test(short_size),
		cmocka_unit_test(larger_size),
		cmocka_unit_test(empty_named_key),
		cmocka_unit_test(numpad_templates),
		cmocka_unit_test(mouse_templates),
		cmocka_unit_test(templates_independent),
		cmocka_unit_test(apple_members_ignored),
		cmocka_unit_test(combination_string),
		cmocka_unit_test(arrow_keys_translated),
		cmocka_unit_test(empty_templates_keep_names),
	};
	int rc = cmocka_run_group_tests(tests, NULL, NULL);

	obs_shutdown();
	return rc;
}
