/* A scene's size can change on one thread while another thread asks for it:
 * scene_load() writes custom_size, cx and cy, and obs_source_get_width() and
 * obs_source_get_height() read them with no lock. The checks here pass with
 * or without the race. The ThreadSanitizer pass of the sanitizers job is what
 * reports it (#3569). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define WRITES 200

static const uint32_t SIZES[2][2] = {{640, 360}, {1280, 720}};

struct reader {
	obs_source_t *source;
	volatile bool stop;
	/* A read that is not one of the sizes the writer stores. */
	volatile long wrong;
	volatile long reads;
};

static bool size_is_known(uint32_t value, int axis)
{
	return value == 0 || value == SIZES[0][axis] || value == SIZES[1][axis];
}

static void *read_size(void *data)
{
	struct reader *reader = data;

	while (!os_atomic_load_bool(&reader->stop)) {
		if (!size_is_known(obs_source_get_width(reader->source), 0) ||
		    !size_is_known(obs_source_get_height(reader->source), 1))
			os_atomic_inc_long(&reader->wrong);
		os_atomic_inc_long(&reader->reads);
	}
	return NULL;
}

static void start_reader(struct reader *reader, obs_source_t *source, pthread_t *thread)
{
	reader->source = source;
	reader->stop = false;
	reader->wrong = 0;
	reader->reads = 0;
	assert_int_equal(pthread_create(thread, NULL, read_size, reader), 0);
	while (os_atomic_load_long(&reader->reads) == 0)
		os_sleep_ms(1);
}

static void stop_reader(struct reader *reader, pthread_t thread)
{
	os_atomic_set_bool(&reader->stop, true);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_int_equal(os_atomic_load_long(&reader->wrong), 0);
}

/* scene_load() stores a saved custom size, as a scene collection load does
 * while the sources of the collection are already visible to other threads. */
static void scene_load_against_getters(void **state)
{
	(void)state;

	obs_scene_t *scene = obs_scene_create_private("scene");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	obs_data_t *settings = obs_source_get_settings(source);
	obs_data_array_t *items = obs_data_array_create();
	obs_data_set_array(settings, "items", items);
	obs_data_array_release(items);
	obs_data_set_bool(settings, "custom_size", true);

	struct reader reader;
	pthread_t thread;
	/* The scene has no main canvas here, so its size is 0 until the load. */
	start_reader(&reader, source, &thread);

	for (int i = 0; i < WRITES; i++) {
		const uint32_t *size = SIZES[i % 2];
		obs_data_set_int(settings, "cx", size[0]);
		obs_data_set_int(settings, "cy", size[1]);
		obs_source_load(source);
	}

	stop_reader(&reader, thread);

	const uint32_t *last = SIZES[(WRITES - 1) % 2];
	assert_int_equal(obs_source_get_width(source), last[0]);
	assert_int_equal(obs_source_get_height(source), last[1]);

	obs_data_release(settings);
	obs_scene_release(scene);
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(scene_load_against_getters),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
