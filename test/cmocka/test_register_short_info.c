/* Regression test for obs-rust/obs-studio#2841: obs_register_source_s() and
 * obs_register_encoder_s() in libobs/obs-module.c.h read a field of the
 * caller's structure past the size it passed. A plugin built against an
 * older header passes the size of its shorter structure, so what lies past
 * that size is not its structure.
 *
 * - The source duplicate check read `version` before the size was looked
 *   at, while the copy, cut at the size, registers version 0. The check and
 *   the registration must use the same version: the one the copy has.
 * - The encoder rules read `caps` whatever the size, while the copy
 *   registers caps 0. They must see 0 when caps lies past the size, as
 *   HAS_VAL() does for the callbacks.
 *
 * Each structure is registered from a buffer whose bytes after the size are
 * the rest of the caller's full structure, with the field past the size set,
 * so a read past the size shows in what is registered. The guarded cases
 * then place the short structure at the very end of a readable page, with
 * an unreadable page after it, so any read past the size faults.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. It skips only when no display is set at all; with one set, a failed
 * obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <sys/mman.h>
#include <unistd.h>
#endif

#include <obs.h>
#include <obs-module.h>

/* --------------------------------------------------------------- callbacks */

static char type_data_tag[] = "Registered";
static int frees;

static const char *get_name(void *type_data)
{
	return type_data ? (const char *)type_data : "(no type data)";
}

static void free_type_data(void *type_data)
{
	(void)type_data;
	frees++;
}

static void *create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return NULL;
}

static void destroy(void *data)
{
	(void)data;
}

static bool encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received_packet;
	return false;
}

/* ----------------------------------------------------------------- helpers */

static bool obs_up;

#define REQUIRE_OBS()          \
	do {                   \
		if (!obs_up)   \
			skip(); \
	} while (false)

/* How many entries of a type list are `id`. */
static size_t listed(bool (*enum_types)(size_t, const char **), const char *id)
{
	const char *t;
	size_t n = 0;
	for (size_t i = 0; enum_types(i, &t); i++)
		if (strcmp(t, id) == 0)
			n++;
	return n;
}

static struct obs_source_info source(const char *id, uint32_t version)
{
	struct obs_source_info info = {0};
	info.id = id;
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = OBS_SOURCE_AUDIO;
	info.get_name = get_name;
	info.type_data = type_data_tag;
	info.free_type_data = free_type_data;
	info.version = version;
	return info;
}

static struct obs_encoder_info encoder(const char *id, uint32_t caps)
{
	struct obs_encoder_info info = {0};
	info.id = id;
	info.type = OBS_ENCODER_VIDEO;
	info.get_name = get_name;
	info.create = create;
	info.destroy = destroy;
	info.encode = encode;
	info.type_data = type_data_tag;
	info.free_type_data = free_type_data;
	info.caps = caps;
	return info;
}

/* A structure cut before `version`, the size an older plugin passes. */
static const size_t source_short = offsetof(struct obs_source_info, version);
/* A structure cut before `caps`. */
static const size_t encoder_short = offsetof(struct obs_encoder_info, caps);

/* ------------------------------------------------------------------ source */

/* A short source whose bytes past the size hold version 3 is version 0, as
 * its copy is: the same id already registered unversioned is a duplicate. */
static void test_source_short_is_unversioned_duplicate(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_source_info info = source("rsi_src_dup", 0);
	obs_register_source_s(&info, sizeof(info));
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_dup"), 1);

	info = source("rsi_src_dup", 3);
	frees = 0;
	obs_register_source_s(&info, source_short);
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_dup"), 1);
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_dup_v3"), 0);
	assert_int_equal(frees, 1);
}

/* Version 3 registered in full is not a duplicate of the short
 * registration, which is version 0. */
static void test_source_short_is_not_versioned_duplicate(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_source_info info = source("rsi_src_v", 3);
	obs_register_source_s(&info, sizeof(info));
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_v_v3"), 1);

	frees = 0;
	obs_register_source_s(&info, source_short);
	assert_int_equal(frees, 0);
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_v"), 1);
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_v_v3"), 1);
}

/* ----------------------------------------------------------------- encoder */

/* A short encoder whose bytes past the size hold texture and self-scaling
 * caps has no caps: it registers as the raw encoder its copy is. */
static void test_encoder_short_scaling_caps(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_encoder_info info = encoder("rsi_enc_scale", OBS_ENCODER_CAP_PASS_TEXTURE | OBS_ENCODER_CAP_SCALING);
	frees = 0;
	obs_register_encoder_s(&info, encoder_short);
	assert_int_equal(listed(obs_enum_encoder_types, "rsi_enc_scale"), 1);
	assert_int_equal(obs_get_encoder_caps("rsi_enc_scale"), 0);
	assert_int_equal(frees, 0);
}

/* Texture caps past the size do not ask for the texture callbacks. */
static void test_encoder_short_texture_caps(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_encoder_info info = encoder("rsi_enc_tex", OBS_ENCODER_CAP_PASS_TEXTURE);
	frees = 0;
	obs_register_encoder_s(&info, encoder_short);
	assert_int_equal(listed(obs_enum_encoder_types, "rsi_enc_tex"), 1);
	assert_int_equal(obs_get_encoder_caps("rsi_enc_tex"), 0);
	assert_int_equal(frees, 0);
}

/* With caps within the size, they are the caller's. */
static void test_encoder_caps_within_size(void **state)
{
	(void)state;
	REQUIRE_OBS();

	const size_t caps_end = encoder_short + sizeof(uint32_t);
	struct obs_encoder_info info = encoder("rsi_enc_in", OBS_ENCODER_CAP_PASS_TEXTURE | OBS_ENCODER_CAP_SCALING);
	frees = 0;
	obs_register_encoder_s(&info, caps_end);
	assert_int_equal(listed(obs_enum_encoder_types, "rsi_enc_in"), 0);
	assert_int_equal(frees, 1);

	info = encoder("rsi_enc_in2", OBS_ENCODER_CAP_DYN_BITRATE);
	obs_register_encoder_s(&info, caps_end);
	assert_int_equal(listed(obs_enum_encoder_types, "rsi_enc_in2"), 1);
	assert_int_equal(obs_get_encoder_caps("rsi_enc_in2"), OBS_ENCODER_CAP_DYN_BITRATE);
}

/* ----------------------------------------------------------------- guarded */

/* Two pages, the second unreadable. */
struct guarded {
	unsigned char *base;
	size_t page;
};

static struct guarded guarded_alloc(void)
{
	struct guarded g = {0};
#ifdef _WIN32
	SYSTEM_INFO si;
	GetSystemInfo(&si);
	g.page = si.dwPageSize;
	g.base = VirtualAlloc(NULL, 2 * g.page, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
	assert_non_null(g.base);
	DWORD old;
	assert_true(VirtualProtect(g.base + g.page, g.page, PAGE_NOACCESS, &old));
#else
	g.page = (size_t)sysconf(_SC_PAGESIZE);
	void *p = mmap(NULL, 2 * g.page, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	assert_true(p != MAP_FAILED);
	g.base = p;
	assert_int_equal(mprotect(g.base + g.page, g.page, PROT_NONE), 0);
#endif
	return g;
}

static void guarded_free(struct guarded g)
{
#ifdef _WIN32
	VirtualFree(g.base, 0, MEM_RELEASE);
#else
	munmap(g.base, 2 * g.page);
#endif
}

/* The first `size` bytes of `info`, ending exactly at the unreadable page. */
static void *guarded_copy(struct guarded g, const void *info, size_t size)
{
	/* The structure stays aligned: each cut is at a pointer boundary. */
	assert_int_equal(size % sizeof(void *), 0);
	unsigned char *at = g.base + g.page - size;
	memcpy(at, info, size);
	return at;
}

/* A source cut before version, at the end of a page: registered
 * unversioned, with nothing read past the size. */
static void test_source_short_guarded(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_source_info info = source("rsi_src_guard", 3);
	struct guarded g = guarded_alloc();
	obs_register_source_s(guarded_copy(g, &info, source_short), source_short);
	guarded_free(g);
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_guard"), 1);
	assert_int_equal(listed(obs_enum_input_types, "rsi_src_guard_v3"), 0);
}

/* An encoder cut before caps, at the end of a page: registered with no
 * caps, with nothing read past the size. */
static void test_encoder_short_guarded(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_encoder_info info = encoder("rsi_enc_guard", OBS_ENCODER_CAP_PASS_TEXTURE);
	struct guarded g = guarded_alloc();
	obs_register_encoder_s(guarded_copy(g, &info, encoder_short), encoder_short);
	guarded_free(g);
	assert_int_equal(listed(obs_enum_encoder_types, "rsi_enc_guard"), 1);
	assert_int_equal(obs_get_encoder_caps("rsi_enc_guard"), 0);
}

/* ------------------------------------------------------------------- setup */

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_source_short_is_unversioned_duplicate),
		cmocka_unit_test(test_source_short_is_not_versioned_duplicate),
		cmocka_unit_test(test_encoder_short_scaling_caps),
		cmocka_unit_test(test_encoder_short_texture_caps),
		cmocka_unit_test(test_encoder_caps_within_size),
		cmocka_unit_test(test_source_short_guarded),
		cmocka_unit_test(test_encoder_short_guarded),
	};
	return cmocka_run_group_tests_name("test_register_short_info", tests, group_setup, group_teardown);
}
