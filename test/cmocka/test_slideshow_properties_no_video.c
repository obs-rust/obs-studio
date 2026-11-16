/* Image Slide Show properties with no video (#4894).
 *
 * Both slideshows' ss_properties() put the canvas size in the "Bounding
 * Size/Aspect Ratio" list as "<base_width>x<base_height>", read from
 * obs_get_video_info(). With no video (no core, or a core before
 * obs_reset_video()), obs_get_video_info() returns false and does not write
 * the struct, and ss_properties() ignored that, so the entry was made from
 * uninitialized stack memory (the valgrind nightly reports it from
 * test_slideshow_folder_order). ss_update() already starts from a zeroed
 * obs_video_info, so with no video it sizes the source from 0x0; the list
 * offers the same.
 *
 * The test fills the stack below the call with a known non-zero pattern
 * first, so an uninitialized struct shows up as a wrong entry instead of
 * depending on what an earlier call left there. No libobs core is started,
 * so obs_get_video_info() has no video to report. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>

#include <obs-module.h>

extern struct obs_source_info slideshow_info;
extern struct obs_source_info slideshow_info_mk2;
extern struct obs_source_info slideshow_info_mk3;

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

/* ------------------------------------------------------------------------- */

#if defined(_MSC_VER)
#define NOINLINE __declspec(noinline)
#else
#define NOINLINE __attribute__((noinline))
#endif

/* Fill the stack the next call will use with 0x5a bytes. */
static NOINLINE void poison_stack(void)
{
	volatile unsigned char fill[16384];
	for (size_t i = 0; i < sizeof(fill); i++)
		fill[i] = 0x5a;
}

static NOINLINE obs_properties_t *get_properties(const struct obs_source_info *info)
{
	poison_stack();
	return info->get_properties(NULL);
}

/* The "WxH" entry of the size list: the one entry that is neither
 * "Automatic" nor an aspect ratio. */
static void assert_canvas_size_entry(const struct obs_source_info *info, const char *want)
{
	struct obs_video_info ovi = {0};
	assert_false(obs_get_video_info(&ovi));

	obs_properties_t *props = get_properties(info);
	obs_property_t *p = obs_properties_get(props, "use_custom_size");
	assert_non_null(p);

	const char *found = NULL;
	size_t count = obs_property_list_item_count(p);
	for (size_t i = 0; i < count; i++) {
		const char *value = obs_property_list_item_string(p, i);
		if (value && strchr(value, 'x')) {
			assert_null(found);
			found = value;
		}
	}

	print_message("%s: canvas size entry \"%s\" (expected \"%s\")\n", info->id, found ? found : "(none)", want);
	assert_non_null(found);
	assert_string_equal(found, want);
	obs_properties_destroy(props);
}

static void legacy_slideshow_offers_0x0_with_no_video(void **state)
{
	UNUSED_PARAMETER(state);
	assert_canvas_size_entry(&slideshow_info, "0x0");
}

static void slideshow_v2_offers_0x0_with_no_video(void **state)
{
	UNUSED_PARAMETER(state);
	assert_canvas_size_entry(&slideshow_info_mk2, "0x0");
}

static void slideshow_v3_offers_0x0_with_no_video(void **state)
{
	UNUSED_PARAMETER(state);
	assert_canvas_size_entry(&slideshow_info_mk3, "0x0");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(legacy_slideshow_offers_0x0_with_no_video),
		cmocka_unit_test(slideshow_v2_offers_0x0_with_no_video),
		cmocka_unit_test(slideshow_v3_offers_0x0_with_no_video),
	};

	return cmocka_run_group_tests_name("slideshow properties with no video", tests, NULL, NULL);
}
