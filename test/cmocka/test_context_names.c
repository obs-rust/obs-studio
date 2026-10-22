/* Characterization of the context names in libobs/obs.c: how a name is
 * made unique when a context is inserted or renamed (the "%s %d" suffix
 * from 2 up), and the "__unnamed%04lld" name a context gets for a NULL or
 * empty name. It pins the CURRENT behaviour before the Rust port, through
 * the public API only (obs_source_get_name() and friends), so the same test
 * runs on libobs with the C backend, the hybrid libobs and
 * libobs-c-reference. Cases marked "characterized, not endorsed" are odd
 * but real.
 *
 * Names come from scene collections, obs-websocket and scripts, so they
 * are any bytes: the cases include names that already end in a number,
 * " (2)", "%" sequences, bytes that are not UTF-8, a long name and many
 * names taken at once.
 *
 * Each case starts its own libobs (the unnamed counter starts at 0) and
 * needs no video. obs_startup() needs a display on Linux, so the test runs
 * under xvfb-run there and the cases skip without a display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/dstr.h>

#define INPUT_ID "context_names_input"
#define SCENE_LIKE_ID "context_names_canvas_source"
#define OUTPUT_ID "context_names_output"
#define MISSING_ID "context_names_missing"

/* ------------------------------------------------------------------------ */
/* Log capture: the duplicate-name warnings                                  */

#define MAX_WARNINGS 8
#define WARNING_SIZE 512

static char warnings[MAX_WARNINGS][WARNING_SIZE];
static size_t num_warnings;

static void capture_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl != LOG_WARNING || strncmp(msg, "Attempted to ", 13) != 0)
		return;
	if (num_warnings < MAX_WARNINGS)
		vsnprintf(warnings[num_warnings], WARNING_SIZE, msg, args);
	num_warnings++;
}

static void clear_warnings(void)
{
	num_warnings = 0;
	memset(warnings, 0, sizeof(warnings));
}

/* ------------------------------------------------------------------------ */
/* A minimal input source type                                               */

static const char *input_name(void *type_data)
{
	(void)type_data;
	return "context names input";
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void input_destroy(void *data)
{
	(void)data;
}

static const char *output_name(void *type_data)
{
	(void)type_data;
	return "context names output";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void output_destroy(void *data)
{
	(void)data;
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void output_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static bool obs_up;

static int setup(void **state)
{
	(void)state;
	clear_warnings();
	base_set_log_handler(capture_log, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&input);

	/* A source type with OBS_SOURCE_REQUIRES_CANVAS, as scenes have: its
	 * names live in the canvas, not in the global table. */
	struct obs_source_info canvas_source = input;
	canvas_source.id = SCENE_LIKE_ID;
	canvas_source.output_flags = OBS_SOURCE_REQUIRES_CANVAS;
	obs_register_source(&canvas_source);

	struct obs_output_info output = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_ENCODED,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_packet,
	};
	obs_register_output(&output);
	clear_warnings();
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	base_set_log_handler(NULL, NULL);
	return 0;
}

#define REQUIRE_OBS()          \
	do {                   \
		if (!obs_up) { \
			skip(); \
		}              \
	} while (false)

static obs_source_t *input(const char *name)
{
	obs_source_t *source = obs_source_create(INPUT_ID, name, NULL, NULL);
	assert_non_null(source);
	return source;
}

static obs_source_t *private_input(const char *name)
{
	obs_source_t *source = obs_source_create_private(INPUT_ID, name, NULL);
	assert_non_null(source);
	return source;
}

/* The source the global table finds by that name is this one. */
static void assert_found(const char *name, obs_source_t *source)
{
	obs_source_t *found = obs_get_source_by_name(name);
	assert_ptr_equal(found, source);
	obs_source_release(found);
}

static void release_all(obs_source_t **sources, size_t num)
{
	for (size_t i = 0; i < num; i++)
		obs_source_release(sources[i]);
}

/* ------------------------------------------------------------------------ */
/* Insert: duplicates get " 2", " 3", ...                                    */

static void test_insert_duplicate(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *s[4];
	s[0] = input("A");
	s[1] = input("A");
	s[2] = input("A");
	assert_string_equal(obs_source_get_name(s[0]), "A");
	assert_string_equal(obs_source_get_name(s[1]), "A 2");
	assert_string_equal(obs_source_get_name(s[2]), "A 3");
	assert_found("A", s[0]);
	assert_found("A 2", s[1]);
	assert_found("A 3", s[2]);

	/* The warning names the requested name and the new one. */
	assert_int_equal(num_warnings, 2);
	assert_string_equal(warnings[0], "Attempted to insert context with duplicate name \"A\"!"
					 " Name has been changed to \"A 2\"");
	assert_string_equal(warnings[1], "Attempted to insert context with duplicate name \"A\"!"
					 " Name has been changed to \"A 3\"");

	/* A free name is kept as it is, with no warning. */
	clear_warnings();
	s[3] = input("B");
	assert_string_equal(obs_source_get_name(s[3]), "B");
	assert_int_equal(num_warnings, 0);

	release_all(s, 4);
}

static void test_insert_first_free_suffix(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *s[8];
	/* "A 2" taken by its own name: "A" goes to "A 3". */
	s[0] = input("A");
	s[1] = input("A 2");
	s[2] = input("A");
	assert_string_equal(obs_source_get_name(s[1]), "A 2");
	assert_string_equal(obs_source_get_name(s[2]), "A 3");

	/* A gap is filled: with "B" and "B 3", "B" goes to "B 2". */
	s[3] = input("B");
	s[4] = input("B 3");
	s[5] = input("B");
	assert_string_equal(obs_source_get_name(s[5]), "B 2");

	/* "C 2" without "C": "C" is free. */
	s[6] = input("C 2");
	s[7] = input("C");
	assert_string_equal(obs_source_get_name(s[6]), "C 2");
	assert_string_equal(obs_source_get_name(s[7]), "C");

	release_all(s, 8);
}

/* A name is not parsed: a trailing number or " (2)" is just more bytes, and
 * the suffix goes after it. */
static void test_insert_names_ending_in_numbers(void **state)
{
	(void)state;
	REQUIRE_OBS();

	static const struct {
		const char *name;
		const char *second;
	} cases[] = {
		{"Cam 2", "Cam 2 2"}, {"X (2)", "X (2) 2"}, {"Y2", "Y2 2"},
		{"Z 0", "Z 0 2"},     {"N -1", "N -1 2"},   {"9", "9 2"},
		{" ", "  2"},         {"T ", "T  2"},       {"Big 2147483647", "Big 2147483647 2"},
	};
	obs_source_t *s[2 * (sizeof(cases) / sizeof(cases[0]))];
	size_t n = 0;

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		obs_source_t *first = input(cases[i].name);
		obs_source_t *second = input(cases[i].name);
		assert_string_equal(obs_source_get_name(first), cases[i].name);
		assert_string_equal(obs_source_get_name(second), cases[i].second);
		s[n++] = first;
		s[n++] = second;
	}

	/* "Cam 2 2" is taken now, so a third "Cam 2" goes on counting. */
	obs_source_t *third = input("Cam 2");
	assert_string_equal(obs_source_get_name(third), "Cam 2 3");
	obs_source_release(third);

	release_all(s, n);
}

/* Names are compared as bytes: case, spaces and bytes that are not UTF-8
 * count, and a '%' is not a format. */
static void test_insert_bytes(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *s[12];
	s[0] = input("A");
	s[1] = input("a");
	s[2] = input("A ");
	s[3] = input(" A");
	assert_string_equal(obs_source_get_name(s[1]), "a");
	assert_string_equal(obs_source_get_name(s[2]), "A ");
	assert_string_equal(obs_source_get_name(s[3]), " A");

	s[4] = input("%s %d %n");
	s[5] = input("%s %d %n");
	assert_string_equal(obs_source_get_name(s[5]), "%s %d %n 2");

	s[6] = input("\xff\xfe\x80");
	s[7] = input("\xff\xfe\x80");
	assert_string_equal(obs_source_get_name(s[7]), "\xff\xfe\x80 2");

	/* Unicode is not normalized: precomposed and combining e-acute are two
	 * names. */
	s[8] = input("caf\xc3\xa9");
	s[9] = input("cafe\xcc\x81");
	assert_string_equal(obs_source_get_name(s[9]), "cafe\xcc\x81");
	s[10] = input("caf\xc3\xa9");
	assert_string_equal(obs_source_get_name(s[10]), "caf\xc3\xa9 2");

	s[11] = input("\t\n");
	assert_string_equal(obs_source_get_name(s[11]), "\t\n");

	assert_int_equal(num_warnings, 3);
	release_all(s, 12);
}

static void test_insert_long_name(void **state)
{
	(void)state;
	REQUIRE_OBS();

	const size_t len = 64 * 1024;
	char *name = bmalloc(len + 1);
	memset(name, 'L', len);
	name[len] = 0;

	obs_source_t *first = input(name);
	obs_source_t *second = input(name);
	const char *got = obs_source_get_name(second);
	assert_int_equal(strlen(got), len + 2);
	assert_memory_equal(got, name, len);
	assert_string_equal(got + len, " 2");
	assert_found(got, second);
	assert_found(name, first);

	obs_source_release(second);
	obs_source_release(first);
	bfree(name);
}

static void test_insert_many(void **state)
{
	(void)state;
	REQUIRE_OBS();

	enum { COUNT = 300 };
	obs_source_t *s[COUNT];
	struct dstr expected = {0};

	for (size_t i = 0; i < COUNT; i++)
		s[i] = input("M");

	assert_string_equal(obs_source_get_name(s[0]), "M");
	for (size_t i = 1; i < COUNT; i++) {
		dstr_printf(&expected, "M %d", (int)i + 1);
		assert_string_equal(obs_source_get_name(s[i]), expected.array);
		assert_found(expected.array, s[i]);
	}
	assert_int_equal(num_warnings, COUNT - 1);

	/* Releasing one frees its name for the next duplicate. */
	obs_source_release(s[150]);
	s[150] = input("M");
	assert_string_equal(obs_source_get_name(s[150]), "M 151");

	dstr_free(&expected);
	release_all(s, COUNT);
}

/* ------------------------------------------------------------------------ */
/* Unnamed names                                                             */

static void test_unnamed(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *s[4];
	s[0] = input(NULL);
	s[1] = input("");
	assert_string_equal(obs_source_get_name(s[0]), "__unnamed0000");
	assert_string_equal(obs_source_get_name(s[1]), "__unnamed0001");
	assert_found("__unnamed0000", s[0]);
	assert_found("__unnamed0001", s[1]);
	assert_int_equal(num_warnings, 0);

	/* A source named like the next unnamed name takes it first: the
	 * unnamed name is then made unique like any other. */
	s[2] = input("__unnamed0002");
	s[3] = input(NULL);
	assert_string_equal(obs_source_get_name(s[3]), "__unnamed0002 2");
	assert_int_equal(num_warnings, 1);

	release_all(s, 4);
}

/* Sources, canvases, encoders, outputs and services all count from the
 * same counter. Encoders, outputs and services are not in a name table:
 * their names may repeat. */
static void test_unnamed_counter_shared(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *a = input(NULL);
	/* An encoder with a NULL name is refused before it takes a number. */
	assert_null(obs_video_encoder_create(MISSING_ID, NULL, NULL, NULL));
	assert_null(obs_audio_encoder_create(MISSING_ID "_audio", NULL, NULL, 0, NULL));
	obs_encoder_t *venc = obs_video_encoder_create(MISSING_ID, "", NULL, NULL);
	obs_encoder_t *aenc = obs_audio_encoder_create(MISSING_ID "_audio", "", NULL, 0, NULL);
	obs_output_t *out = obs_output_create(OUTPUT_ID, NULL, NULL, NULL);
	obs_canvas_t *canvas = obs_canvas_create(NULL, NULL, 0);
	obs_source_t *b = input("");
	assert_non_null(venc);
	assert_non_null(aenc);
	assert_non_null(out);
	assert_non_null(canvas);

	assert_string_equal(obs_source_get_name(a), "__unnamed0000");
	assert_string_equal(obs_encoder_get_name(venc), "__unnamed0001");
	assert_string_equal(obs_encoder_get_name(aenc), "__unnamed0002");
	assert_string_equal(obs_output_get_name(out), "__unnamed0003");
	assert_string_equal(obs_canvas_get_name(canvas), "__unnamed0004");
	assert_string_equal(obs_source_get_name(b), "__unnamed0005");

	/* A service of a type that is not registered is not created and
	 * takes no number. */
	obs_service_t *service = obs_service_create(MISSING_ID, NULL, NULL, NULL);
	assert_null(service);
	obs_source_t *c = input(NULL);
	assert_string_equal(obs_source_get_name(c), "__unnamed0006");

	/* Outputs and encoders are not made unique. */
	obs_output_t *out2 = obs_output_create(OUTPUT_ID, "O", NULL, NULL);
	obs_output_t *out3 = obs_output_create(OUTPUT_ID, "O", NULL, NULL);
	assert_string_equal(obs_output_get_name(out2), "O");
	assert_string_equal(obs_output_get_name(out3), "O");
	obs_encoder_t *enc2 = obs_video_encoder_create(MISSING_ID, "E", NULL, NULL);
	obs_encoder_t *enc3 = obs_video_encoder_create(MISSING_ID, "E", NULL, NULL);
	assert_string_equal(obs_encoder_get_name(enc2), "E");
	assert_string_equal(obs_encoder_get_name(enc3), "E");

	/* obs_encoder_set_name() ignores NULL, empty and the same name, and
	 * does not make a name unique. */
	obs_encoder_set_name(enc3, NULL);
	obs_encoder_set_name(enc3, "");
	assert_string_equal(obs_encoder_get_name(enc3), "E");
	obs_encoder_set_name(enc3, "__unnamed0000");
	assert_string_equal(obs_encoder_get_name(enc3), "__unnamed0000");

	obs_source_t *d = input(NULL);
	assert_string_equal(obs_source_get_name(d), "__unnamed0007");
	assert_int_equal(num_warnings, 0);

	obs_encoder_release(enc3);
	obs_encoder_release(enc2);
	obs_output_release(out3);
	obs_output_release(out2);
	obs_canvas_release(canvas);
	obs_output_release(out);
	obs_encoder_release(aenc);
	obs_encoder_release(venc);
	obs_source_release(d);
	obs_source_release(c);
	obs_source_release(b);
	obs_source_release(a);
}

/* The number has at least four digits, and more once it passes 9999. */
static void test_unnamed_width(void **state)
{
	(void)state;
	REQUIRE_OBS();

	/* Private sources with an empty name count too, and are in no table. */
	for (int i = 0; i < 9999; i++)
		obs_source_release(private_input(""));

	obs_source_t *a = input(NULL);
	obs_source_t *b = input(NULL);
	assert_string_equal(obs_source_get_name(a), "__unnamed9999");
	assert_string_equal(obs_source_get_name(b), "__unnamed10000");

	obs_source_release(b);
	obs_source_release(a);
}

/* ------------------------------------------------------------------------ */
/* Private sources                                                           */

static void test_private(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *pub = input("A");

	/* A private source keeps its NULL name, and takes no number. */
	obs_source_t *p0 = private_input(NULL);
	assert_null(obs_source_get_name(p0));
	/* An empty name gets an unnamed name. */
	obs_source_t *p1 = private_input("");
	assert_string_equal(obs_source_get_name(p1), "__unnamed0000");

	/* Private names are not made unique, against public or private ones. */
	obs_source_t *p2 = private_input("A");
	obs_source_t *p3 = private_input("A");
	assert_string_equal(obs_source_get_name(p2), "A");
	assert_string_equal(obs_source_get_name(p3), "A");
	assert_found("A", pub);
	assert_int_equal(num_warnings, 0);

	/* A rename of a private source is not made unique either, and NULL
	 * makes it nameless again. */
	obs_source_set_name(p0, "A");
	assert_string_equal(obs_source_get_name(p0), "A");
	obs_source_set_name(p0, NULL);
	assert_null(obs_source_get_name(p0));
	obs_source_set_name(p0, "");
	assert_string_equal(obs_source_get_name(p0), "__unnamed0001");
	assert_int_equal(num_warnings, 0);

	obs_source_t *next = input(NULL);
	assert_string_equal(obs_source_get_name(next), "__unnamed0002");

	obs_source_release(next);
	obs_source_release(p3);
	obs_source_release(p2);
	obs_source_release(p1);
	obs_source_release(p0);
	obs_source_release(pub);
}

/* ------------------------------------------------------------------------ */
/* Rename                                                                    */

struct renames {
	int count;
	char new_name[64];
	char prev_name[64];
};

static void on_rename(void *data, calldata_t *cd)
{
	struct renames *r = data;
	const char *new_name = calldata_string(cd, "new_name");
	const char *prev_name = calldata_string(cd, "prev_name");
	r->count++;
	snprintf(r->new_name, sizeof(r->new_name), "%s", new_name ? new_name : "(null)");
	snprintf(r->prev_name, sizeof(r->prev_name), "%s", prev_name ? prev_name : "(null)");
}

static void test_rename(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *a = input("A");
	obs_source_t *b = input("B");
	struct renames r = {0};
	signal_handler_connect(obs_source_get_signal_handler(b), "rename", on_rename, &r);

	/* To a free name. */
	obs_source_set_name(b, "C");
	assert_string_equal(obs_source_get_name(b), "C");
	assert_int_equal(r.count, 1);
	assert_string_equal(r.prev_name, "B");
	assert_string_equal(r.new_name, "C");
	assert_null(obs_get_source_by_name("B"));
	assert_found("C", b);
	assert_int_equal(num_warnings, 0);

	/* To its own name: nothing happens, no signal. */
	obs_source_set_name(b, "C");
	assert_string_equal(obs_source_get_name(b), "C");
	assert_int_equal(r.count, 1);

	/* To a taken name: the suffix, and a warning. */
	obs_source_set_name(b, "A");
	assert_string_equal(obs_source_get_name(b), "A 2");
	assert_int_equal(r.count, 2);
	assert_string_equal(r.prev_name, "C");
	assert_string_equal(r.new_name, "A 2");
	assert_found("A", a);
	assert_found("A 2", b);
	assert_null(obs_get_source_by_name("C"));
	assert_int_equal(num_warnings, 1);
	/* Characterized, not endorsed: the warning names the old name ("C"),
	 * not the requested one ("A"). */
	assert_string_equal(warnings[0], "Attempted to rename context to duplicate name \"C\"!"
					 " New name has been changed to \"A 2\"");

	/* Characterized, not endorsed: "A 2" renamed to "A" ends up as "A 2"
	 * again (its own name is free while it is renamed), still with the
	 * warning and a rename signal from "A 2" to "A 2". */
	clear_warnings();
	obs_source_set_name(b, "A");
	assert_string_equal(obs_source_get_name(b), "A 2");
	assert_int_equal(r.count, 3);
	assert_string_equal(r.prev_name, "A 2");
	assert_string_equal(r.new_name, "A 2");
	assert_int_equal(num_warnings, 1);
	assert_string_equal(warnings[0], "Attempted to rename context to duplicate name \"A 2\"!"
					 " New name has been changed to \"A 2\"");
	assert_found("A 2", b);

	/* A rename frees the old name. */
	obs_source_t *c = input("C");
	assert_string_equal(obs_source_get_name(c), "C");

	signal_handler_disconnect(obs_source_get_signal_handler(b), "rename", on_rename, &r);
	obs_source_release(c);
	obs_source_release(b);
	obs_source_release(a);
}

static void test_rename_unnamed(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *a = input("A");
	obs_source_t *taken = input("__unnamed0001");

	/* To an empty name: the next unnamed name. */
	obs_source_set_name(a, "");
	assert_string_equal(obs_source_get_name(a), "__unnamed0000");
	assert_found("__unnamed0000", a);

	/* Characterized, not endorsed: the unnamed name of a rename is not
	 * made unique, so two sources now have the same name, and no warning
	 * is logged. */
	obs_source_t *b = input("B");
	obs_source_set_name(b, "");
	assert_string_equal(obs_source_get_name(b), "__unnamed0001");
	assert_string_equal(obs_source_get_name(taken), "__unnamed0001");
	assert_int_equal(num_warnings, 0);

	obs_source_release(b);
	obs_source_release(taken);
	obs_source_release(a);
}

/* ------------------------------------------------------------------------ */
/* Canvas-bound sources and canvases                                         */

/* Sources that need a canvas (scenes, groups) are made unique within their
 * canvas, not against inputs. */
static void test_canvas_sources(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *in = input("S");
	obs_source_t *s1 = obs_source_create(SCENE_LIKE_ID, "S", NULL, NULL);
	obs_source_t *s2 = obs_source_create(SCENE_LIKE_ID, "S", NULL, NULL);
	assert_non_null(s1);
	assert_non_null(s2);
	assert_string_equal(obs_source_get_name(in), "S");
	assert_string_equal(obs_source_get_name(s1), "S");
	assert_string_equal(obs_source_get_name(s2), "S 2");

	/* A scene takes the same rule. */
	obs_scene_t *scene = obs_scene_create("S");
	assert_non_null(scene);
	assert_string_equal(obs_source_get_name(obs_scene_get_source(scene)), "S 3");

	/* Renamed within the canvas. */
	obs_source_set_name(s2, "S");
	assert_string_equal(obs_source_get_name(s2), "S 2");
	obs_source_set_name(s2, "T");
	assert_string_equal(obs_source_get_name(s2), "T");
	obs_source_set_name(s1, "T");
	assert_string_equal(obs_source_get_name(s1), "T 2");

	obs_scene_release(scene);
	obs_source_release(s2);
	obs_source_release(s1);
	obs_source_release(in);
}

static void test_canvases(void **state)
{
	(void)state;
	REQUIRE_OBS();

	/* The main canvas is "Main". */
	obs_canvas_t *c1 = obs_canvas_create("Main", NULL, 0);
	obs_canvas_t *c2 = obs_canvas_create("K", NULL, 0);
	obs_canvas_t *c3 = obs_canvas_create("K", NULL, 0);
	obs_canvas_t *p = obs_canvas_create_private("K", NULL, 0);
	assert_non_null(c1);
	assert_non_null(c2);
	assert_non_null(c3);
	assert_non_null(p);
	assert_string_equal(obs_canvas_get_name(c1), "Main 2");
	assert_string_equal(obs_canvas_get_name(c2), "K");
	assert_string_equal(obs_canvas_get_name(c3), "K 2");
	assert_string_equal(obs_canvas_get_name(p), "K");

	/* obs_canvas_set_name() ignores NULL, empty and the same name. */
	obs_canvas_set_name(c3, NULL);
	obs_canvas_set_name(c3, "");
	obs_canvas_set_name(c3, "K 2");
	assert_string_equal(obs_canvas_get_name(c3), "K 2");
	obs_canvas_set_name(c3, "K");
	assert_string_equal(obs_canvas_get_name(c3), "K 2");
	obs_canvas_set_name(c3, "Main");
	assert_string_equal(obs_canvas_get_name(c3), "Main 3");

	obs_canvas_t *found = obs_get_canvas_by_name("Main 3");
	assert_ptr_equal(found, c3);
	obs_canvas_release(found);

	obs_canvas_release(p);
	obs_canvas_remove(c3);
	obs_canvas_release(c3);
	obs_canvas_remove(c2);
	obs_canvas_release(c2);
	obs_canvas_remove(c1);
	obs_canvas_release(c1);
}

#define CASE(fn) cmocka_unit_test_setup_teardown(fn, setup, teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(test_insert_duplicate),
		CASE(test_insert_first_free_suffix),
		CASE(test_insert_names_ending_in_numbers),
		CASE(test_insert_bytes),
		CASE(test_insert_long_name),
		CASE(test_insert_many),
		CASE(test_unnamed),
		CASE(test_unnamed_counter_shared),
		CASE(test_unnamed_width),
		CASE(test_private),
		CASE(test_rename),
		CASE(test_rename_unnamed),
		CASE(test_canvas_sources),
		CASE(test_canvases),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
