/* obs_reset_video() must not destroy and re-create the video core mutexes.
 *
 * The audio thread (audio_callback() takes obs->video.mixes_mutex), any
 * thread queueing a graphics task (obs->video.task_mutex) and encoder
 * starts (obs->video.encoder_group_mutex) keep running while the video is
 * reset. Destroying a mutex another thread may hold or wait on is undefined;
 * on Windows it frees the lock under the waiting thread and crashes
 * (upstream obsproject/obs-studio#14192). On glibc the re-init silently
 * wipes a lock another thread holds, so two threads enter the same critical
 * section.
 *
 * Each case takes one mutex on a helper thread in the middle of a reset,
 * when obs_reset_video() logs "video settings reset:" (after the old video
 * is freed, before the new one is initialized), and holds it. When the reset
 * returns while the helper still holds the lock, the lock must still be
 * held. A reset that needs the lock itself waits for the helper, which then
 * lets go after HOLD_TIMEOUT_MS.
 *
 * The core (`obs`) is hidden in libobs.so, so the test finds it through the
 * first mutex obs_init() creates, obs->video.stats.mutex: this executable
 * wraps pthread_mutex_init() and records the first call made on the main
 * thread inside obs_startup(), then checks the result against
 * obs_get_signal_handler(). */
#define _GNU_SOURCE
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <pthread.h>
#include <time.h>
#include <dlfcn.h>
#include <cmocka.h>
#include "obs-internal.h"

#define HOLD_TIMEOUT_MS 1000

struct holder {
	pthread_mutex_t *target;
	pthread_mutex_t state_lock;
	pthread_cond_t state_cond;
	bool armed;
	bool started;
	bool holding;
	bool reset_returned;
	bool released_on_timeout;
	pthread_t thread;
};

static struct holder holder;
static pthread_t main_thread;
static bool capture_core;
static pthread_mutex_t *first_startup_mutex;

__attribute__((visibility("default"))) int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr)
{
	static int (*real_init)(pthread_mutex_t *, const pthread_mutexattr_t *);
	if (!real_init)
		*(void **)&real_init = dlsym(RTLD_NEXT, "pthread_mutex_init");
	if (capture_core && !first_startup_mutex && pthread_equal(pthread_self(), main_thread))
		first_startup_mutex = mutex;
	return real_init(mutex, attr);
}

static struct obs_core *startup_core(void)
{
	main_thread = pthread_self();
	first_startup_mutex = NULL;
	capture_core = true;
	bool started = obs_startup("en-US", NULL, NULL);
	capture_core = false;
	assert_true(started);
	assert_non_null(first_startup_mutex);
	struct obs_core *core =
		(struct obs_core *)((uint8_t *)first_startup_mutex - offsetof(struct obs_core, video.stats.mutex));
	assert_ptr_equal(core->signals, obs_get_signal_handler());
	assert_ptr_equal(core->procs, obs_get_proc_handler());
	return core;
}
static log_handler_t previous_handler;
static void *previous_param;

static void *hold_thread(void *arg)
{
	struct holder *h = arg;
	pthread_mutex_lock(h->target);

	struct timespec deadline;
	clock_gettime(CLOCK_REALTIME, &deadline);
	deadline.tv_sec += HOLD_TIMEOUT_MS / 1000;
	deadline.tv_nsec += (long)(HOLD_TIMEOUT_MS % 1000) * 1000000L;
	if (deadline.tv_nsec >= 1000000000L) {
		deadline.tv_sec++;
		deadline.tv_nsec -= 1000000000L;
	}

	pthread_mutex_lock(&h->state_lock);
	h->holding = true;
	pthread_cond_broadcast(&h->state_cond);
	while (!h->reset_returned) {
		if (pthread_cond_timedwait(&h->state_cond, &h->state_lock, &deadline) == ETIMEDOUT) {
			h->released_on_timeout = !h->reset_returned;
			break;
		}
	}
	/* Clear holding before letting go of the target. A reset that waits
	 * for the target can take it the moment it is free and return; it must
	 * not then see holding still true and test a lock nobody holds. */
	h->holding = false;
	pthread_cond_broadcast(&h->state_cond);
	pthread_mutex_unlock(&h->state_lock);

	pthread_mutex_unlock(h->target);
	return NULL;
}

static void reset_log_handler(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (holder.armed && !holder.started && strncmp(format, "video settings reset:", 21) == 0) {
		holder.started = true;
		assert_int_equal(pthread_create(&holder.thread, NULL, hold_thread, &holder), 0);
		pthread_mutex_lock(&holder.state_lock);
		while (!holder.holding)
			pthread_cond_wait(&holder.state_cond, &holder.state_lock);
		pthread_mutex_unlock(&holder.state_lock);
	}
	if (previous_handler) {
		va_list copy;
		va_copy(copy, args);
		previous_handler(level, format, copy, previous_param);
		va_end(copy);
	}
}

static struct obs_video_info video_info(uint32_t size)
{
	struct obs_video_info ovi = {.graphics_module = GRAPHICS_MODULE_PATH,
				     .fps_num = 30,
				     .fps_den = 1,
				     .base_width = size,
				     .base_height = size,
				     .output_width = size,
				     .output_height = size,
				     .output_format = VIDEO_FORMAT_NV12,
				     .colorspace = VIDEO_CS_709,
				     .range = VIDEO_RANGE_PARTIAL,
				     .scale_type = OBS_SCALE_BICUBIC};
	return ovi;
}

enum core_mutex { MIXES_MUTEX, TASK_MUTEX, ENCODER_GROUP_MUTEX };

static pthread_mutex_t *core_mutex(struct obs_core *core, enum core_mutex which)
{
	switch (which) {
	case MIXES_MUTEX:
		return &core->video.mixes_mutex;
	case TASK_MUTEX:
		return &core->video.task_mutex;
	case ENCODER_GROUP_MUTEX:
		return &core->video.encoder_group_mutex;
	}
	return NULL;
}

static void lock_survives_reset(enum core_mutex which, const char *name)
{
	struct obs_core *core = startup_core();
	struct obs_video_info first = video_info(32);
	assert_int_equal(obs_reset_video(&first), OBS_VIDEO_SUCCESS);

	memset(&holder, 0, sizeof(holder));
	holder.target = core_mutex(core, which);
	assert_int_equal(pthread_mutex_init(&holder.state_lock, NULL), 0);
	assert_int_equal(pthread_cond_init(&holder.state_cond, NULL), 0);
	base_get_log_handler(&previous_handler, &previous_param);
	base_set_log_handler(reset_log_handler, NULL);
	holder.armed = true;

	struct obs_video_info second = video_info(64);
	int reset = obs_reset_video(&second);

	holder.armed = false;
	base_set_log_handler(previous_handler, previous_param);
	assert_int_equal(reset, OBS_VIDEO_SUCCESS);
	assert_true(holder.started);

	pthread_mutex_lock(&holder.state_lock);
	bool still_held = holder.holding;
	pthread_mutex_unlock(&holder.state_lock);

	int trylock = -1;
	if (still_held)
		trylock = pthread_mutex_trylock(holder.target);
	if (trylock == 0)
		pthread_mutex_unlock(holder.target);

	pthread_mutex_lock(&holder.state_lock);
	holder.reset_returned = true;
	pthread_cond_broadcast(&holder.state_cond);
	pthread_mutex_unlock(&holder.state_lock);
	assert_int_equal(pthread_join(holder.thread, NULL), 0);

	if (still_held && trylock != EBUSY)
		fail_msg("obs_reset_video() re-created obs->video.%s while another thread held it "
			 "(pthread_mutex_trylock returned %d, expected EBUSY)",
			 name, trylock);

	pthread_cond_destroy(&holder.state_cond);
	pthread_mutex_destroy(&holder.state_lock);
	obs_shutdown();
}

static void mixes_mutex_survives_reset(void **state)
{
	(void)state;
	lock_survives_reset(MIXES_MUTEX, "mixes_mutex");
}

static void task_mutex_survives_reset(void **state)
{
	(void)state;
	lock_survives_reset(TASK_MUTEX, "task_mutex");
}

static void encoder_group_mutex_survives_reset(void **state)
{
	(void)state;
	lock_survives_reset(ENCODER_GROUP_MUTEX, "encoder_group_mutex");
}

static int teardown(void **state)
{
	(void)state;
	if (holder.armed) {
		holder.armed = false;
		base_set_log_handler(previous_handler, previous_param);
	}
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(mixes_mutex_survives_reset, teardown),
		cmocka_unit_test_teardown(task_mutex_survives_reset, teardown),
		cmocka_unit_test_teardown(encoder_group_mutex_survives_reset, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
