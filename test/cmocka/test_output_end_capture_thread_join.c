/* libobs: every completed end of data capture must have its thread joined
 * (#2063).
 *
 * obs_output_end_data_capture() ends capture on end_data_capture_thread,
 * which cleared end_data_capture_thread_active itself when it finished. The
 * next start, the next end of capture and the destroy joined the thread only
 * while that flag was set, so a thread that had already finished was never
 * joined: each stop leaked a joinable thread and its stack (TSan: thread leak
 * in obs_output_end_data_capture_internal). null-output had the same bug
 * (test_null_output_stop_thread).
 *
 * libobs is a shared library here, so --wrap cannot reach its calls: this
 * executable defines pthread_create, pthread_join and pthread_detach itself,
 * which the dynamic linker binds libobs to, counts them and forwards to the
 * next definition. A raw video output is started and stopped several times
 * and released; afterwards no thread created meanwhile may be left neither
 * joined nor detached. Needs core video, so it skips without a display. */

#define _GNU_SOURCE
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <dlfcn.h>
#include <pthread.h>
#include <stdio.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>

#define RAW_OUTPUT_ID "end_capture_thread_join_output"
#define CYCLES 4

/* ------------------------------------------------------------------------- */
/* pthread interposition */

static long threads_created;
static long threads_joined;
static long threads_detached;

int pthread_create(pthread_t *thread, const pthread_attr_t *attr, void *(*routine)(void *), void *arg)
{
	static int (*real)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
	if (!real)
		real = (int (*)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *))dlsym(RTLD_NEXT,
												      "pthread_create");
	int err = real(thread, attr, routine, arg);
	if (err == 0)
		__atomic_add_fetch(&threads_created, 1, __ATOMIC_SEQ_CST);
	return err;
}

int pthread_join(pthread_t thread, void **result)
{
	static int (*real)(pthread_t, void **);
	if (!real)
		real = (int (*)(pthread_t, void **))dlsym(RTLD_NEXT, "pthread_join");
	int err = real(thread, result);
	if (err == 0)
		__atomic_add_fetch(&threads_joined, 1, __ATOMIC_SEQ_CST);
	return err;
}

int pthread_detach(pthread_t thread)
{
	static int (*real)(pthread_t);
	if (!real)
		real = (int (*)(pthread_t))dlsym(RTLD_NEXT, "pthread_detach");
	int err = real(thread);
	if (err == 0)
		__atomic_add_fetch(&threads_detached, 1, __ATOMIC_SEQ_CST);
	return err;
}

/* Threads created and neither joined nor detached yet. */
static long unjoined(void)
{
	return __atomic_load_n(&threads_created, __ATOMIC_SEQ_CST) -
	       __atomic_load_n(&threads_joined, __ATOMIC_SEQ_CST) -
	       __atomic_load_n(&threads_detached, __ATOMIC_SEQ_CST);
}

/* ------------------------------------------------------------------------- */
/* a raw video output */

static const char *raw_output_name(void *unused)
{
	(void)unused;
	return "end capture thread join output";
}

static void *raw_output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void raw_output_destroy(void *data)
{
	(void)data;
}

static bool raw_output_start(void *data)
{
	obs_output_t *output = data;
	if (!obs_output_can_begin_data_capture(output, 0))
		return false;
	return obs_output_begin_data_capture(output, 0);
}

static void raw_output_stop(void *data, uint64_t ts)
{
	(void)ts;
	obs_output_end_data_capture(data);
}

static void raw_output_video(void *data, struct video_data *frame)
{
	(void)data;
	(void)frame;
}

/* ------------------------------------------------------------------------- */

static bool obs_up;

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up)
		return 0;
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: %d\n", ret);
		return -1;
	}
	struct obs_output_info info = {
		.id = RAW_OUTPUT_ID,
		.flags = OBS_OUTPUT_VIDEO,
		.get_name = raw_output_name,
		.create = raw_output_create,
		.destroy = raw_output_destroy,
		.start = raw_output_start,
		.stop = raw_output_stop,
		.raw_video = raw_output_video,
	};
	obs_register_output(&info);
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

/* Waits up to 5 s for the output to finish stopping. */
static void wait_stopped(obs_output_t *output)
{
	for (int i = 0; i < 500 && obs_output_active(output); i++)
		os_sleep_ms(10);
	assert_false(obs_output_active(output));
	/* end_data_capture_thread returns right after deactivating. */
	os_sleep_ms(50);
}

static void stopped_outputs_join_their_capture_threads(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	long before = unjoined();
	obs_output_t *output = obs_output_create(RAW_OUTPUT_ID, "end capture thread join", NULL, NULL);
	assert_non_null(output);
	for (int i = 0; i < CYCLES; i++) {
		assert_true(obs_output_start(output));
		obs_output_stop(output);
		wait_stopped(output);
	}
	obs_output_release(output);
	obs_wait_for_destroy_queue();

	/* Without the interposition every count stays 0 and nothing is checked. */
	assert_true(__atomic_load_n(&threads_created, __ATOMIC_SEQ_CST) >= CYCLES);
	long left = unjoined() - before;
	fprintf(stderr, "%d start/stop cycles: %ld threads created, %ld left unjoined\n", CYCLES,
		__atomic_load_n(&threads_created, __ATOMIC_SEQ_CST), left);
	assert_int_equal(left, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(stopped_outputs_join_their_capture_threads),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
