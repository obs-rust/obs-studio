#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <cmocka.h>

#include <errno.h>
#include <pthread.h>

/*
 * Changing the bitrate while streaming could stall every QSV encoder at
 * 0 kbps (obs-rust/obs-studio#2291, obsproject/obs-studio#13228): the
 * encoder update reset the encoder without g_QsvLock, the lock every other
 * QSV call takes because all sessions share one device. libobs runs the
 * update on the encoder's own thread, so the reset raced the encode calls of
 * the other QSV encoders.
 *
 * Compiles plugins/obs-qsv11/obs-qsv11.c.h with fake qsv_encoder_* functions:
 * no Intel GPU needed.
 */

#include "plugins/obs-qsv11/build/obs-qsv11-c.pch.h"
#include "plugins/obs-qsv11/obs-qsv11.c.h"

/* ------------------------------------------------------------------------- */
/* What obs-qsv11.c.h needs from the rest of the plugin                      */

struct adapter_info adapters[MAX_ADAPTERS];
size_t adapter_count;
size_t adapter_index;

const char *obs_module_text(const char *lookup_string)
{
	return lookup_string;
}

bool qsv_host_texture_valid(const struct encoder_texture *tex)
{
	return tex != NULL;
}

enum qsv_cpu_platform qsv_get_cpu_platform(void)
{
	return QSV_CPU_PLATFORM_INTEL;
}

/* ------------------------------------------------------------------------- */
/* Fake encoder                                                              */

static pthread_mutex_t fake_mutex = PTHREAD_MUTEX_INITIALIZER;
static int reconfig_calls;
static bool reconfig_saw_lock_free;
static bool reconfig_during_encode;
static volatile bool encode_in_progress;
static mfxU16 reconfig_target;
static mfxU16 reconfig_max;

static void *probe_lock(void *arg)
{
	bool *free_out = arg;

	/* Another thread, like another encoder's encode call. */
	if (pthread_mutex_trylock(&g_QsvLock) == 0) {
		*free_out = true;
		pthread_mutex_unlock(&g_QsvLock);
	}
	return NULL;
}

int qsv_encoder_reconfig(qsv_t *ctx, qsv_param_t *params)
{
	(void)ctx;
	bool lock_free = false;
	pthread_t thread;

	pthread_create(&thread, NULL, probe_lock, &lock_free);
	pthread_join(thread, NULL);

	pthread_mutex_lock(&fake_mutex);
	reconfig_calls++;
	reconfig_saw_lock_free |= lock_free;
	reconfig_during_encode |= encode_in_progress;
	reconfig_target = params->nTargetBitRate;
	reconfig_max = params->nMaxBitRate;
	pthread_mutex_unlock(&fake_mutex);
	return true;
}

int qsv_encoder_close(qsv_t *ctx)
{
	(void)ctx;
	return 0;
}

void qsv_encoder_version(unsigned short *major, unsigned short *minor)
{
	*major = 2;
	*minor = 9;
}

qsv_t *qsv_encoder_open(qsv_param_t *params, enum qsv_codec codec, bool useTexAlloc)
{
	(void)params;
	(void)codec;
	(void)useTexAlloc;
	return NULL;
}

void qsv_encoder_add_roi(qsv_t *ctx, const struct obs_encoder_roi *roi)
{
	(void)ctx;
	(void)roi;
}

void qsv_encoder_clear_roi(qsv_t *ctx)
{
	(void)ctx;
}

int qsv_encoder_encode(qsv_t *ctx, uint64_t ts, uint8_t *y, uint8_t *uv, uint32_t sy, uint32_t suv, mfxBitstream **bs)
{
	(void)ctx;
	(void)ts;
	(void)y;
	(void)uv;
	(void)sy;
	(void)suv;
	*bs = NULL;
	return -1;
}

int qsv_encoder_encode_tex(qsv_t *ctx, uint64_t ts, void *tex, uint64_t lock_key, uint64_t *next_key, mfxBitstream **bs)
{
	(void)ctx;
	(void)ts;
	(void)tex;
	*next_key = lock_key;
	*bs = NULL;
	return -1;
}

int qsv_encoder_headers(qsv_t *ctx, uint8_t **sps, uint8_t **pps, uint16_t *nsps, uint16_t *npps)
{
	(void)ctx;
	*sps = *pps = NULL;
	*nsps = *npps = 0;
	return 0;
}

int qsv_hevc_encoder_headers(qsv_t *ctx, uint8_t **vps, uint8_t **sps, uint8_t **pps, uint16_t *nvps, uint16_t *nsps,
			     uint16_t *npps)
{
	(void)ctx;
	*vps = *sps = *pps = NULL;
	*nvps = *nsps = *npps = 0;
	return 0;
}

/* ------------------------------------------------------------------------- */

static void reset_fake(void)
{
	pthread_mutex_lock(&fake_mutex);
	reconfig_calls = 0;
	reconfig_saw_lock_free = false;
	reconfig_during_encode = false;
	encode_in_progress = false;
	reconfig_target = 0;
	reconfig_max = 0;
	pthread_mutex_unlock(&fake_mutex);
}

static int get_reconfig_calls(void)
{
	pthread_mutex_lock(&fake_mutex);
	int calls = reconfig_calls;
	pthread_mutex_unlock(&fake_mutex);
	return calls;
}

struct update_job {
	struct obs_qsv *obsqsv;
	obs_data_t *settings;
	bool result;
};

static void *run_update(void *arg)
{
	struct update_job *job = arg;
	job->result = obs_qsv_update(job->obsqsv, job->settings);
	return NULL;
}

/* The reconfigure runs with g_QsvLock held: another thread cannot take it. */
static void test_update_holds_qsv_lock(void **state)
{
	(void)state;
	struct obs_qsv obsqsv = {0};
	obs_data_t *settings = obs_data_create();

	reset_fake();
	obsqsv.context = (qsv_t *)(uintptr_t)0x1;
	obsqsv.params.nRateControl = MFX_RATECONTROL_CBR;
	obsqsv.params.nTargetBitRate = 2500;
	obs_data_set_int(settings, "bitrate", 6000);

	assert_true(obs_qsv_update(&obsqsv, settings));
	assert_int_equal(get_reconfig_calls(), 1);
	if (reconfig_saw_lock_free) {
		fprintf(stderr, "the encoder reset ran without g_QsvLock\n");
		fail();
	}
	assert_int_equal(reconfig_target, 6000);

	/* The update leaves the lock free. */
	assert_int_equal(pthread_mutex_trylock(&g_QsvLock), 0);
	pthread_mutex_unlock(&g_QsvLock);

	obs_data_release(settings);
}

/*
 * The race itself: another QSV encoder is inside its encode call (holding
 * g_QsvLock, as obs_qsv_encode does) when this encoder's update arrives on
 * its own thread. The reset must wait for the encode to finish.
 */
static void test_update_waits_for_other_encode(void **state)
{
	(void)state;
	struct obs_qsv obsqsv = {0};
	obs_data_t *settings = obs_data_create();
	struct update_job job = {&obsqsv, settings, false};
	pthread_t thread;

	reset_fake();
	obsqsv.context = (qsv_t *)(uintptr_t)0x1;
	obsqsv.params.nRateControl = MFX_RATECONTROL_CBR;
	obsqsv.params.nTargetBitRate = 2500;
	obs_data_set_int(settings, "bitrate", 4000);

	/* The other encoder's encode call. */
	pthread_mutex_lock(&g_QsvLock);
	encode_in_progress = true;

	pthread_create(&thread, NULL, run_update, &job);

	/* Give the update thread time to reach the reset. */
	for (int i = 0; i < 20 && get_reconfig_calls() == 0; i++)
		os_sleep_ms(10);

	encode_in_progress = false;
	pthread_mutex_unlock(&g_QsvLock);

	pthread_join(thread, NULL);

	assert_true(job.result);
	assert_int_equal(get_reconfig_calls(), 1);
	if (reconfig_during_encode) {
		fprintf(stderr, "the encoder reset ran while another QSV encode held g_QsvLock\n");
		fail();
	}
	assert_int_equal(reconfig_target, 4000);

	obs_data_release(settings);
}

/* VBR: a live change carries the maximum bitrate as well as the target. */
static void test_update_passes_max_bitrate(void **state)
{
	(void)state;
	struct obs_qsv obsqsv = {0};
	obs_data_t *settings = obs_data_create();

	reset_fake();
	obsqsv.context = (qsv_t *)(uintptr_t)0x1;
	obsqsv.params.nRateControl = MFX_RATECONTROL_VBR;
	obsqsv.params.nTargetBitRate = 2500;
	obsqsv.params.nMaxBitRate = 3000;
	obs_data_set_int(settings, "bitrate", 6000);
	obs_data_set_int(settings, "max_bitrate", 9000);

	assert_true(obs_qsv_update(&obsqsv, settings));
	assert_int_equal(reconfig_target, 6000);
	assert_int_equal(reconfig_max, 9000);

	obs_data_release(settings);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_update_holds_qsv_lock),
		cmocka_unit_test(test_update_waits_for_other_encode),
		cmocka_unit_test(test_update_passes_max_bitrate),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
