/* Image Slide Show: the order the images of a folder play in
 * (obsproject/obs-studio#10382 and #3619, forum threads 93496, 140210,
 * 153394 and 184255).
 *
 * Both slideshows (obs-slideshow-mk2.c.h and the original obs-slideshow.c.h)
 * add the images of a folder in os_readdir() order. On Linux and macOS that
 * is the filesystem's order (creation or hash order), not the names, so a
 * folder of slide1..slide10 plays in a jumble. On Windows, NTFS lists a
 * folder sorted by name (case-insensitive ordinal).
 *
 * The slideshow gets a "Folder Order" list: Name (natural: slide2 before
 * slide10), Name (A-Z), and Filesystem order (unsorted, the old behaviour).
 * Only the images of a folder are sorted; files added one by one keep their
 * place in the list. Slideshows saved before the change (slideshow_v2, and
 * the original "slideshow") keep filesystem order; a new slideshow
 * (slideshow_v3) defaults to natural order.
 *
 * Part 1 needs no libobs core: the name comparisons, the folder listing on a
 * real folder whose files are created out of name order, and each slideshow
 * version's defaults.
 * Part 2 runs the real slideshows (built into the test, with the real Cut
 * transition and a stub Image source that records the slides it is asked
 * for, in order) under obs_startup(). libobs needs an X display on Linux, so
 * those cases skip without one (run under xvfb-run); no video or graphics
 * module is needed.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <util/bmem.h>
#include <util/darray.h>
#include <util/dstr.h>
#include <util/platform.h>

#include "slideshow-order.h"

extern struct obs_source_info slideshow_info;
extern struct obs_source_info slideshow_info_mk2;
extern struct obs_source_info slideshow_info_mk3;
extern struct obs_source_info cut_transition;

static bool obs_up = false;

/* ------------------------------------------------------------------------- */
/* what the slideshows use from the rest of the image-source module */

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

uint64_t image_source_get_memory_usage(void *data)
{
	UNUSED_PARAMETER(data);
	return 0;
}

void image_source_preload_image(void *data)
{
	UNUSED_PARAMETER(data);
}

void image_source_ready_slide(void *data)
{
	UNUSED_PARAMETER(data);
}

/* the file of every Image source the slideshow creates, in creation order */
static DARRAY(char *) created;

static void clear_created(void)
{
	for (size_t i = 0; i < created.num; i++)
		bfree(created.array[i]);
	da_free(created);
}

static const char *slide_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test slide";
}

static void *slide_create(obs_data_t *settings, obs_source_t *source)
{
	char *file = bstrdup(obs_data_get_string(settings, "file"));
	da_push_back(created, &file);
	return source;
}

static void slide_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static uint32_t slide_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 16;
}

static void register_types(void)
{
	struct obs_source_info slide = {
		.id = "image_source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = slide_name,
		.create = slide_create,
		.destroy = slide_destroy,
		.get_width = slide_size,
		.get_height = slide_size,
	};
	obs_register_source(&slide);
	obs_register_source(&cut_transition);
	obs_register_source(&slideshow_info);
	obs_register_source(&slideshow_info_mk2);
	obs_register_source(&slideshow_info_mk3);
}

/* ------------------------------------------------------------------------- */
/* a folder whose files are created out of name order */

#define FOLDER "test_slideshow_folder_order.d"

/* creation order; notes.txt is not an image and sub is a folder */
static const char *create_order[] = {"slide10.png", "Slide3.png", "slide1.png", "cover.png", "slide2.png", "notes.txt"};
#define CREATED (sizeof(create_order) / sizeof(create_order[0]))

static const char *natural_order[] = {"cover.png", "slide1.png", "slide2.png", "Slide3.png", "slide10.png"};
static const char *name_order[] = {"cover.png", "slide1.png", "slide10.png", "slide2.png", "Slide3.png"};
#define IMAGES 5
#define NAME_LEN 256

static char folder[512];

/* removes the folder, its files and (empty) subfolders */
static void remove_folder(void)
{
	os_dir_t *dir = os_opendir(folder);
	if (!dir)
		return;
	struct dstr path = {0};
	struct os_dirent *ent;
	while ((ent = os_readdir(dir)) != NULL) {
		if (strcmp(ent->d_name, ".") == 0 || strcmp(ent->d_name, "..") == 0)
			continue;
		dstr_printf(&path, "%s/%s", folder, ent->d_name);
		if (ent->directory)
			os_rmdir(path.array);
		else
			os_unlink(path.array);
	}
	os_closedir(dir);
	dstr_free(&path);
	os_rmdir(folder);
}

static void make_folder(void)
{
	char cwd[400];
	assert_non_null(os_getcwd(cwd, sizeof(cwd)));
	snprintf(folder, sizeof(folder), "%s/%s", cwd, FOLDER);
	for (char *c = folder; *c; c++) {
		if (*c == '\\')
			*c = '/';
	}

	remove_folder();
	assert_int_not_equal(os_mkdirs(folder), MKDIR_ERROR);

	struct dstr path = {0};
	for (size_t i = 0; i < CREATED; i++) {
		dstr_printf(&path, "%s/%s", folder, create_order[i]);
		FILE *f = os_fopen(path.array, "wb");
		assert_non_null(f);
		fputs("not decoded", f);
		fclose(f);
	}
	dstr_printf(&path, "%s/sub.png", folder);
	assert_int_not_equal(os_mkdir(path.array), MKDIR_ERROR);
	dstr_free(&path);
}

/* the images of the folder in os_readdir() order: what "Filesystem order"
 * must keep */
static size_t readdir_order(char names[IMAGES][NAME_LEN])
{
	size_t n = 0;
	os_dir_t *dir = os_opendir(folder);
	assert_non_null(dir);
	struct os_dirent *ent;
	while ((ent = os_readdir(dir)) != NULL) {
		if (ent->directory || strcmp(os_get_path_extension(ent->d_name), ".png") != 0)
			continue;
		assert_true(n < IMAGES);
		snprintf(names[n++], NAME_LEN, "%s", ent->d_name);
	}
	os_closedir(dir);
	return n;
}

static const char *base_name(const char *path)
{
	const char *slash = strrchr(path, '/');
	return slash ? slash + 1 : path;
}

static void assert_names(char **paths, size_t count, const char *const *expected, size_t expected_count)
{
	for (size_t i = 0; i < count && i < expected_count; i++) {
		if (strcmp(base_name(paths[i]), expected[i]) != 0) {
			print_error("position %zu: got %s, expected %s\n", i, base_name(paths[i]), expected[i]);
			for (size_t j = 0; j < count; j++)
				print_error("  %zu: %s\n", j, base_name(paths[j]));
			fail();
		}
	}
	assert_int_equal(count, expected_count);
}

static int folder_setup(void **state)
{
	UNUSED_PARAMETER(state);
	make_folder();
	return 0;
}

static int folder_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	remove_folder();
	return 0;
}

/* ------------------------------------------------------------------------- */
/* Part 1: no libobs core */

static int sign(int v)
{
	return (v > 0) - (v < 0);
}

#define assert_before(cmp, a, b)                                   \
	do {                                                       \
		if (!(cmp(a, b) < 0 && cmp(b, a) > 0)) {           \
			print_error("expected %s before %s\n", a, b); \
			fail();                                    \
		}                                                  \
	} while (0)

static void test_natural_numbers_compare_as_numbers(void **state)
{
	UNUSED_PARAMETER(state);
	assert_before(slideshow_name_cmp_natural, "slide2.png", "slide10.png");
	assert_before(slideshow_name_cmp_natural, "slide9.png", "slide10.png");
	assert_before(slideshow_name_cmp_natural, "1.png", "2.png");
	assert_before(slideshow_name_cmp_natural, "2.png", "10.png");
	assert_before(slideshow_name_cmp_natural, "10.png", "100.png");
	assert_before(slideshow_name_cmp_natural, "img2-1.png", "img2-10.png");
	assert_before(slideshow_name_cmp_natural, "img2-10.png", "img10-1.png");
	/* larger than any integer type */
	assert_before(slideshow_name_cmp_natural, "99999999999999999999999.png", "100000000000000000000000.png");
	/* leading zeros: same value, so the rest of the name decides */
	assert_before(slideshow_name_cmp_natural, "slide01.png", "slide2.png");
	assert_before(slideshow_name_cmp_natural, "slide002.png", "slide10.png");
	assert_before(slideshow_name_cmp_natural, "slide01a.png", "slide1b.png");
	/* same value, fewer zeros first; a total order */
	assert_before(slideshow_name_cmp_natural, "slide1.png", "slide01.png");
	assert_int_equal(slideshow_name_cmp_natural("slide10.png", "slide10.png"), 0);
}

static void test_natural_ignores_case(void **state)
{
	UNUSED_PARAMETER(state);
	assert_before(slideshow_name_cmp_natural, "apple.png", "Banana.png");
	assert_before(slideshow_name_cmp_natural, "Apple.png", "banana.png");
	assert_before(slideshow_name_cmp_natural, "slide2.png", "Slide3.png");
	assert_before(slideshow_name_cmp_natural, "Slide3.png", "slide10.png");
	/* names that differ only in case still get a fixed order */
	assert_int_not_equal(slideshow_name_cmp_natural("A.png", "a.png"), 0);
	assert_int_equal(sign(slideshow_name_cmp_natural("A.png", "a.png")),
			 -sign(slideshow_name_cmp_natural("a.png", "A.png")));
}

static void test_natural_equal_prefixes(void **state)
{
	UNUSED_PARAMETER(state);
	assert_before(slideshow_name_cmp_natural, "slide", "slide1");
	assert_before(slideshow_name_cmp_natural, "slide1", "slide1a");
	assert_before(slideshow_name_cmp_natural, "slide1.png", "slide1a.png");
	assert_before(slideshow_name_cmp_natural, "slide.png", "slide1.png");
	assert_before(slideshow_name_cmp_natural, "", "a");
	assert_int_equal(slideshow_name_cmp_natural("", ""), 0);
}

static void test_name_is_case_insensitive_ordinal(void **state)
{
	UNUSED_PARAMETER(state);
	/* character by character: no numbers */
	assert_before(slideshow_name_cmp, "slide10.png", "slide2.png");
	assert_before(slideshow_name_cmp, "slide1.png", "slide10.png");
	assert_before(slideshow_name_cmp, "apple.png", "Banana.png");
	assert_before(slideshow_name_cmp, "Apple.png", "banana.png");
	assert_before(slideshow_name_cmp, "slide2.png", "Slide3.png");
	/* NTFS compares upper-cased names: '_' (0x5F) sorts after letters */
	assert_before(slideshow_name_cmp, "abc.png", "_abc.png");
	assert_before(slideshow_name_cmp, "slide", "slide1");
	assert_int_equal(slideshow_name_cmp("slide2.png", "slide2.png"), 0);
	assert_int_not_equal(slideshow_name_cmp("A.png", "a.png"), 0);
}

static void test_order_from_string(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(slideshow_folder_order_from_string("natural"), SLIDESHOW_FOLDER_ORDER_NATURAL);
	assert_int_equal(slideshow_folder_order_from_string("name"), SLIDESHOW_FOLDER_ORDER_NAME);
	assert_int_equal(slideshow_folder_order_from_string("filesystem"), SLIDESHOW_FOLDER_ORDER_FILESYSTEM);
	/* a missing or unknown value keeps the old behaviour */
	assert_int_equal(slideshow_folder_order_from_string(""), SLIDESHOW_FOLDER_ORDER_FILESYSTEM);
	assert_int_equal(slideshow_folder_order_from_string(NULL), SLIDESHOW_FOLDER_ORDER_FILESYSTEM);
	assert_int_equal(slideshow_folder_order_from_string("bogus"), SLIDESHOW_FOLDER_ORDER_FILESYSTEM);
}

static void list_folder(enum slideshow_folder_order order, slideshow_paths_t *paths)
{
	da_init(*paths);
	os_dir_t *dir = os_opendir(folder);
	assert_non_null(dir);
	slideshow_folder_files(dir, folder, order, paths);
	os_closedir(dir);
}

static void test_folder_listing_each_order(void **state)
{
	UNUSED_PARAMETER(state);
	slideshow_paths_t paths;

	list_folder(SLIDESHOW_FOLDER_ORDER_NATURAL, &paths);
	assert_names(paths.array, paths.num, natural_order, IMAGES);
	/* full paths: "<folder>/<name>" */
	assert_true(strncmp(paths.array[0], folder, strlen(folder)) == 0);
	assert_int_equal(paths.array[0][strlen(folder)], '/');
	slideshow_paths_free(&paths);

	list_folder(SLIDESHOW_FOLDER_ORDER_NAME, &paths);
	assert_names(paths.array, paths.num, name_order, IMAGES);
	slideshow_paths_free(&paths);

	char fs[IMAGES][NAME_LEN];
	const char *fs_order[IMAGES];
	size_t n = readdir_order(fs);
	assert_int_equal(n, IMAGES);
	for (size_t i = 0; i < n; i++)
		fs_order[i] = fs[i];

	list_folder(SLIDESHOW_FOLDER_ORDER_FILESYSTEM, &paths);
	assert_names(paths.array, paths.num, fs_order, IMAGES);
	slideshow_paths_free(&paths);
}

static void test_defaults_per_version(void **state)
{
	UNUSED_PARAMETER(state);
	struct {
		struct obs_source_info *info;
		const char *expected;
	} cases[] = {
		{&slideshow_info, SLIDESHOW_S_FOLDER_ORDER_FILESYSTEM},     /* saved before slideshow_v2 */
		{&slideshow_info_mk2, SLIDESHOW_S_FOLDER_ORDER_FILESYSTEM}, /* saved by older builds */
		{&slideshow_info_mk3, SLIDESHOW_S_FOLDER_ORDER_NATURAL},    /* new slideshows */
	};

	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		obs_data_t *settings = obs_data_create();
		cases[i].info->get_defaults(settings);
		const char *value = obs_data_get_string(settings, SLIDESHOW_S_FOLDER_ORDER);
		if (strcmp(value, cases[i].expected) != 0) {
			print_error("%s v%u: default folder order \"%s\", expected \"%s\"\n", cases[i].info->id,
				    cases[i].info->version, value, cases[i].expected);
			fail();
		}
		obs_data_release(settings);
	}

	/* only the current version is offered for new sources */
	assert_int_equal(slideshow_info_mk3.version, 3);
	assert_int_equal(slideshow_info_mk3.output_flags & OBS_SOURCE_CAP_OBSOLETE, 0);
	assert_int_not_equal(slideshow_info_mk2.output_flags & OBS_SOURCE_CAP_OBSOLETE, 0);
	assert_int_not_equal(slideshow_info.output_flags & OBS_SOURCE_CAP_OBSOLETE, 0);
}

/* ------------------------------------------------------------------------- */
/* Part 2: the real slideshows */

#define REQUIRE_OBS()                                                  \
	do {                                                           \
		if (!obs_up) {                                         \
			print_message("libobs did not start, skipping\n"); \
			skip();                                        \
		}                                                      \
	} while (0)

static obs_data_array_t *files_array(const char *const *values, size_t count)
{
	obs_data_array_t *files = obs_data_array_create();
	for (size_t i = 0; i < count; i++) {
		obs_data_t *item = obs_data_create();
		obs_data_set_string(item, "value", values[i]);
		obs_data_array_push_back(files, item);
		obs_data_release(item);
	}
	return files;
}

/* settings of a slideshow playing the folder; order NULL leaves the
 * setting unset (the version's default) */
static obs_data_t *folder_settings(const char *order, const char *playback_mode)
{
	obs_data_t *settings = obs_data_create();
	const char *values[] = {folder};
	obs_data_array_t *files = files_array(values, 1);
	obs_data_set_array(settings, "files", files);
	obs_data_array_release(files);
	obs_data_set_string(settings, "transition", "cut");
	obs_data_set_string(settings, "use_custom_size", "64x64");
	obs_data_set_string(settings, "playback_mode", playback_mode);
	obs_data_set_bool(settings, "loop", true);
	if (order)
		obs_data_set_string(settings, SLIDESHOW_S_FOLDER_ORDER, order);
	return settings;
}

static long long total_files(obs_source_t *ss)
{
	calldata_t cd = {0};
	proc_handler_call(obs_source_get_proc_handler(ss), "total_files", &cd);
	long long n = calldata_int(&cd, "total_files");
	calldata_free(&cd);
	return n;
}

/* The slideshow creates an Image source for the first slide and the slides
 * after it (and the original slideshow one for every file), in play order;
 * with at most a few files that is the whole list. */
static void assert_plays(obs_source_t *ss, const char *const *expected, size_t count)
{
	assert_non_null(ss);
	assert_int_equal(total_files(ss), count);
	assert_names(created.array, created.num < count ? created.num : count, expected, count);
}

/* libobs runs a video source's update on its next video tick
 * (obs_source_deferred_update(), which calls the source's update with its
 * settings). There is no video thread here, so do that by hand. */
static void tick(obs_source_t *ss)
{
	const char *id = obs_source_get_id(ss);
	struct obs_source_info *info = &slideshow_info;
	if (strcmp(id, "slideshow_v3") == 0)
		info = &slideshow_info_mk3;
	else if (strcmp(id, "slideshow_v2") == 0)
		info = &slideshow_info_mk2;

	clear_created();
	obs_data_t *settings = obs_source_get_settings(ss);
	info->update(obs_obj_get_data(ss), settings);
	obs_data_release(settings);
}

static obs_source_t *create(const char *kind, obs_data_t *settings)
{
	obs_source_t *ss = obs_source_create(kind, "slideshow", settings, NULL);
	obs_data_release(settings);
	if (ss)
		tick(ss);
	return ss;
}

static void test_play_each_order(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();

	char fs[IMAGES][NAME_LEN];
	const char *fs_order[IMAGES];
	readdir_order(fs);
	for (size_t i = 0; i < IMAGES; i++)
		fs_order[i] = fs[i];

	const char *kinds[] = {"slideshow_v3", "slideshow_v2", "slideshow"};
	for (size_t k = 0; k < sizeof(kinds) / sizeof(kinds[0]); k++) {
		print_message("%s\n", kinds[k]);
		obs_source_t *ss = create(kinds[k], folder_settings("natural", "loop"));
		assert_plays(ss, natural_order, IMAGES);
		obs_source_release(ss);

		ss = create(kinds[k], folder_settings("name", "loop"));
		assert_plays(ss, name_order, IMAGES);
		obs_source_release(ss);

		ss = create(kinds[k], folder_settings("filesystem", "loop"));
		assert_plays(ss, fs_order, IMAGES);
		obs_source_release(ss);
	}
}

/* changing the setting on a running slideshow re-reads the folder */
static void test_update_changes_order(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();

	obs_source_t *ss = create("slideshow_v3", folder_settings("filesystem", "loop"));
	assert_non_null(ss);

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, SLIDESHOW_S_FOLDER_ORDER, "natural");
	obs_source_update(ss, settings);
	obs_data_release(settings);
	tick(ss);
	assert_plays(ss, natural_order, IMAGES);
	obs_source_release(ss);
}

/* Files added one by one keep their place; only the folder is sorted. */
static void test_single_files_keep_list_order(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();

	struct dstr z = {0}, a = {0};
	dstr_printf(&z, "%s/slide10.png", folder);
	dstr_printf(&a, "%s/cover.png", folder);
	const char *values[] = {z.array, folder, a.array};
	const char *expected[] = {"slide10.png", "cover.png",   "slide1.png", "slide2.png",
				  "Slide3.png",  "slide10.png", "cover.png"};

	obs_data_t *settings = folder_settings("natural", "loop");
	obs_data_array_t *files = files_array(values, 3);
	obs_data_set_array(settings, "files", files);
	obs_data_array_release(files);

	obs_source_t *ss = create("slideshow_v3", settings);
	assert_plays(ss, expected, IMAGES + 2);
	obs_source_release(ss);
	dstr_free(&z);
	dstr_free(&a);
}

/* Random playback still picks at random, from every image of the folder. */
static void test_random_uses_whole_folder(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();

	obs_source_t *ss = create("slideshow_v3", folder_settings(NULL, "random"));
	assert_non_null(ss);
	assert_int_equal(total_files(ss), IMAGES);
	obs_source_release(ss);
}

/* the folder order a source would show in its properties */
static const char *source_order(obs_source_t *ss, char *buf, size_t size)
{
	obs_data_t *settings = obs_source_get_settings(ss);
	snprintf(buf, size, "%s", obs_data_get_string(settings, SLIDESHOW_S_FOLDER_ORDER));
	obs_data_release(settings);
	return buf;
}

/* A new slideshow, as the Add Source menu creates it (the latest version, no
 * settings but the files), plays in natural order. */
static void test_new_slideshow_defaults_to_natural(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();

	const char *kind = obs_get_latest_input_type_id("slideshow");
	assert_non_null(kind);
	assert_string_equal(kind, "slideshow_v3");
	assert_int_equal(obs_get_source_output_flags("slideshow_v2") & OBS_SOURCE_CAP_OBSOLETE,
			 OBS_SOURCE_CAP_OBSOLETE);

	obs_data_t *defaults = obs_get_source_defaults(kind);
	assert_string_equal(obs_data_get_string(defaults, SLIDESHOW_S_FOLDER_ORDER), "natural");
	obs_data_release(defaults);

	obs_source_t *ss = create(kind, folder_settings(NULL, "loop"));
	char buf[32];
	assert_string_equal(source_order(ss, buf, sizeof(buf)), "natural");
	assert_plays(ss, natural_order, IMAGES);

	/* the properties list offers the three orders */
	obs_properties_t *props = obs_source_properties(ss);
	obs_property_t *p = obs_properties_get(props, SLIDESHOW_S_FOLDER_ORDER);
	assert_non_null(p);
	assert_int_equal(obs_property_list_item_count(p), 3);
	assert_string_equal(obs_property_list_item_string(p, 0), "natural");
	assert_string_equal(obs_property_list_item_string(p, 1), "name");
	assert_string_equal(obs_property_list_item_string(p, 2), "filesystem");
	obs_properties_destroy(props);
	obs_source_release(ss);
}

/* A slideshow from a scene collection saved before the change: the source
 * as an older build saves it (no folder order setting). */
static obs_source_t *load_saved(const char *id, const char *versioned_id)
{
	struct dstr json = {0};
	struct dstr escaped = {0};
	dstr_copy(&escaped, folder);
	dstr_replace(&escaped, "\\", "\\\\");
	dstr_printf(&json,
		    "{\"balance\": 0.5, \"deinterlace_field_order\": 0, \"deinterlace_mode\": 0, \"enabled\": true,"
		    " \"flags\": 0, \"hotkeys\": {}, \"id\": \"%s\", \"mixers\": 255, \"monitoring_type\": 0,"
		    " \"muted\": false, \"name\": \"SS\", \"prev_ver\": 553648128, \"private_settings\": {},"
		    " \"push-to-mute\": false, \"push-to-mute-delay\": 0, \"push-to-talk\": false,"
		    " \"push-to-talk-delay\": 0, \"settings\": {\"files\": [{\"hidden\": false, \"selected\": false,"
		    " \"value\": \"%s\"}], \"playback_mode\": \"loop\", \"slide_time\": 600, \"transition\": \"cut\","
		    " \"use_custom_size\": \"64x64\"}, \"sync\": 0, \"versioned_id\": \"%s\", \"volume\": 1.0}",
		    id, escaped.array, versioned_id);
	obs_data_t *data = obs_data_create_from_json(json.array);
	assert_non_null(data);
	dstr_free(&json);
	dstr_free(&escaped);

	obs_source_t *ss = obs_load_source(data);
	obs_data_release(data);
	assert_non_null(ss);
	obs_source_load(ss);
	tick(ss);
	return ss;
}

static void test_saved_slideshow_keeps_filesystem_order(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_OBS();

	char fs[IMAGES][NAME_LEN];
	const char *fs_order[IMAGES];
	readdir_order(fs);
	for (size_t i = 0; i < IMAGES; i++)
		fs_order[i] = fs[i];

	const char *versions[] = {"slideshow_v2", "slideshow"};
	for (size_t v = 0; v < 2; v++) {
		obs_source_t *ss = load_saved("slideshow", versions[v]);
		assert_string_equal(obs_source_get_id(ss), versions[v]);
		char buf[32];
		assert_string_equal(source_order(ss, buf, sizeof(buf)), "filesystem");
		assert_plays(ss, fs_order, IMAGES);

		/* saving it again keeps it a slideshow_v2 (or v1) */
		obs_data_t *saved = obs_save_source(ss);
		assert_string_equal(obs_data_get_string(saved, "versioned_id"), versions[v]);
		obs_data_release(saved);
		obs_source_release(ss);
	}
}

/* ------------------------------------------------------------------------- */

static int setup(void **state)
{
	folder_setup(state);
	if (!obs_startup("en-US", NULL, NULL))
		return 0;
	obs_up = true;
	register_types();
	return 0;
}

static int teardown(void **state)
{
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	clear_created();
	return folder_teardown(state);
}

int main(void)
{
	const struct CMUnitTest no_core[] = {
		cmocka_unit_test(test_natural_numbers_compare_as_numbers),
		cmocka_unit_test(test_natural_ignores_case),
		cmocka_unit_test(test_natural_equal_prefixes),
		cmocka_unit_test(test_name_is_case_insensitive_ordinal),
		cmocka_unit_test(test_order_from_string),
		cmocka_unit_test(test_folder_listing_each_order),
		cmocka_unit_test(test_defaults_per_version),
	};
	const struct CMUnitTest with_core[] = {
		cmocka_unit_test(test_play_each_order),
		cmocka_unit_test(test_update_changes_order),
		cmocka_unit_test(test_single_files_keep_list_order),
		cmocka_unit_test(test_random_uses_whole_folder),
		cmocka_unit_test(test_new_slideshow_defaults_to_natural),
		cmocka_unit_test(test_saved_slideshow_keeps_filesystem_order),
	};

	int failed = cmocka_run_group_tests_name("no libobs core", no_core, folder_setup, folder_teardown);
	failed += cmocka_run_group_tests_name("real slideshows", with_core, setup, teardown);
	return failed;
}
