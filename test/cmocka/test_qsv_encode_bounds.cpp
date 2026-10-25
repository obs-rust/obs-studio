#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
extern "C" {
#include <cmocka.h>
}

#include <atomic>
#include <chrono>
#include <thread>

/*
 * A live bitrate change could leave every QSV encoder streaming nothing
 * (obs-rust/obs-studio#2291, obsproject/obs-studio#13228). Two things in the
 * encode path turned a wedged device into a silent 0 kbps stall instead of an
 * error that stops the output:
 *  - EncodeFrameAsync was retried forever while it reported the device busy,
 *    with the lock every QSV encoder shares held;
 *  - a SyncOperation timeout (MFX_WRN_IN_EXECUTION) was taken for a finished
 *    frame, so the encode "succeeded" with an empty bitstream.
 * The live update itself dropped VBR changes, CBR changes with lookahead,
 * and counted a reset that applied with a warning as a failure.
 *
 * Compiles the plugin's real encoder (QSV_Encoder.cpp, QSV_Encoder_Internal.cpp)
 * against a scripted fake of the libvpl C API: no Intel GPU needed.
 */

#include "plugins/obs-qsv11/build/obs-qsv11.core+.hpp.cpp"

/* ------------------------------------------------------------------------- */
/* Fake libvpl                                                               */

enum fake_encode_mode { ENCODE_OK, ENCODE_BUSY };
enum fake_sync_mode { SYNC_OK, SYNC_TIMEOUT };

static std::atomic<int> encode_mode{ENCODE_OK};
static std::atomic<int> sync_mode{SYNC_OK};
static mfxU16 fake_platform;
static mfxStatus reset_status;
static int reset_calls;
static mfxVideoParam reset_params;
static mfxU16 reset_winbrc_max_avg;
static int fake_session_obj;
/* The encoder's startup probe: loaders live, and whether its session fails
 * (#2396). */
static int loaders;
static bool session_fails;

static void fake_defaults(void)
{
	loaders = 0;
	session_fails = false;
	encode_mode = ENCODE_OK;
	sync_mode = SYNC_OK;
	fake_platform = 0;
	reset_status = MFX_ERR_NONE;
	reset_calls = 0;
	memset(&reset_params, 0, sizeof(reset_params));
	reset_winbrc_max_avg = 0;
}

extern "C" {

mfxLoader MFXLoad(void)
{
	loaders++;
	return (mfxLoader)&fake_session_obj;
}

void MFXUnload(mfxLoader loader)
{
	(void)loader;
	loaders--;
}

mfxConfig MFXCreateConfig(mfxLoader loader)
{
	(void)loader;
	return (mfxConfig)&fake_session_obj;
}

mfxStatus MFXSetConfigFilterProperty(mfxConfig config, const mfxU8 *name, mfxVariant value)
{
	(void)config;
	(void)name;
	(void)value;
	return MFX_ERR_NONE;
}

mfxStatus MFXCreateSession(mfxLoader loader, mfxU32 i, mfxSession *session)
{
	(void)loader;
	(void)i;
	if (session_fails) {
		return MFX_ERR_NOT_FOUND;
	}
	*session = (mfxSession)&fake_session_obj;
	return MFX_ERR_NONE;
}

mfxStatus MFXClose(mfxSession session)
{
	(void)session;
	return MFX_ERR_NONE;
}

mfxStatus MFXQueryVersion(mfxSession session, mfxVersion *version)
{
	(void)session;
	version->Major = 2;
	version->Minor = 13;
	return MFX_ERR_NONE;
}

mfxStatus MFXVideoCORE_QueryPlatform(mfxSession session, mfxPlatform *platform)
{
	(void)session;
	memset(platform, 0, sizeof(*platform));
	PRAGMA_WARN_PUSH
	PRAGMA_WARN_DEPRECATION
	platform->CodeName = fake_platform;
	PRAGMA_WARN_POP
	return MFX_ERR_NONE;
}

/* The fake's sync point is the task's bitstream: the "hardware" fills it in
 * when the frame completes. */
mfxStatus MFXVideoCORE_SyncOperation(mfxSession session, mfxSyncPoint syncp, mfxU32 wait)
{
	(void)session;
	(void)wait;
	if (sync_mode == SYNC_TIMEOUT) {
		return MFX_WRN_IN_EXECUTION;
	}

	mfxBitstream *bs = (mfxBitstream *)syncp;
	static const mfxU8 frame[] = {0, 0, 0, 1, 0x65, 0x88};
	memcpy(bs->Data, frame, sizeof(frame));
	bs->DataOffset = 0;
	bs->DataLength = sizeof(frame);
	bs->FrameType = MFX_FRAMETYPE_I | MFX_FRAMETYPE_IDR;
	return MFX_ERR_NONE;
}

mfxStatus MFXVideoENCODE_Query(mfxSession session, mfxVideoParam *in, mfxVideoParam *out)
{
	(void)session;
	(void)in;
	(void)out;
	return MFX_ERR_NONE;
}

mfxStatus MFXVideoENCODE_QueryIOSurf(mfxSession session, mfxVideoParam *par, mfxFrameAllocRequest *request)
{
	(void)session;
	request->Info = par->mfx.FrameInfo;
	request->NumFrameMin = 2;
	request->NumFrameSuggested = 2;
	return MFX_ERR_NONE;
}

mfxStatus MFXVideoENCODE_Init(mfxSession session, mfxVideoParam *par)
{
	(void)session;
	(void)par;
	return MFX_ERR_NONE;
}

mfxStatus MFXVideoENCODE_Reset(mfxSession session, mfxVideoParam *par)
{
	(void)session;
	reset_calls++;
	reset_params = *par;
	for (mfxU16 i = 0; i < par->NumExtParam; i++) {
		if (par->ExtParam[i]->BufferId == MFX_EXTBUFF_CODING_OPTION3) {
			reset_winbrc_max_avg = ((mfxExtCodingOption3 *)par->ExtParam[i])->WinBRCMaxAvgKbps;
		}
	}
	return reset_status;
}

mfxStatus MFXVideoENCODE_Close(mfxSession session)
{
	(void)session;
	return MFX_ERR_NONE;
}

mfxStatus MFXVideoENCODE_GetVideoParam(mfxSession session, mfxVideoParam *par)
{
	(void)session;
	par->AsyncDepth = 2;
	par->mfx.BufferSizeInKB = 4;
	return MFX_ERR_NONE;
}

mfxStatus MFXVideoENCODE_GetEncodeStat(mfxSession session, mfxEncodeStat *stat)
{
	(void)session;
	(void)stat;
	return MFX_ERR_UNSUPPORTED;
}

mfxStatus MFXVideoENCODE_EncodeFrameAsync(mfxSession session, mfxEncodeCtrl *ctrl, mfxFrameSurface1 *surface,
					  mfxBitstream *bs, mfxSyncPoint *syncp)
{
	(void)session;
	(void)ctrl;
	(void)surface;
	if (encode_mode == ENCODE_BUSY) {
		return MFX_WRN_DEVICE_BUSY;
	}
	*syncp = (mfxSyncPoint)bs;
	return MFX_ERR_NONE;
}

mfxStatus MFXMemory_GetSurfaceForEncode(mfxSession session, mfxFrameSurface1 **surface)
{
	(void)session;
	(void)surface;
	return MFX_ERR_UNSUPPORTED;
}

void util_cpuid(int cpuinfo[4], int flags)
{
	(void)flags;
	memset(cpuinfo, 0, sizeof(int) * 4);
}

} /* extern "C" */

/* What the host part of the plugin (common_utils_<os>.cpp) provides. */

mfxStatus Initialize(mfxVersion version, mfxSession *pSession, mfxFrameAllocator *pmfxAllocator, mfxHDL *deviceHandle,
		     bool bCreateSharedHandles, enum qsv_codec codec, void **data)
{
	(void)version;
	(void)pmfxAllocator;
	(void)deviceHandle;
	(void)bCreateSharedHandles;
	(void)codec;
	*pSession = (mfxSession)&fake_session_obj;
	*data = NULL;
	return MFX_ERR_NONE;
}

void Release() {}

void ReleaseSessionData(void *data)
{
	(void)data;
}

mfxStatus simple_copytex(mfxHDL pthis, mfxMemId mid, void *tex, mfxU64 lock_key, mfxU64 *next_key)
{
	(void)pthis;
	(void)mid;
	(void)tex;
	*next_key = lock_key;
	return MFX_ERR_NONE;
}

void qsv_host_select_accel_mode(mfxConfig cfg, bool *use_d3d11, bool *use_tex_alloc)
{
	(void)cfg;
	*use_d3d11 = false;
	*use_tex_alloc = false;
}

void qsv_host_add_video_signal_info(mfxExtVideoSignalInfo *info, const qsv_param_t *pParams,
				    std::vector<mfxExtBuffer *> &extendedBuffers)
{
	(void)info;
	(void)pParams;
	(void)extendedBuffers;
}

/* ------------------------------------------------------------------------- */

#define WIDTH 64
#define HEIGHT 64

/* mfxPlatform code names (deprecated enum fields in libvpl). */
#define PLATFORM_ICELAKE 30
#define PLATFORM_BATTLEMAGE 52

static uint8_t plane_y[WIDTH * HEIGHT];
static uint8_t plane_uv[WIDTH * HEIGHT / 2];

static void default_params(qsv_param_t *params, mfxU16 rate_control)
{
	memset(params, 0, sizeof(*params));
	params->nTargetUsage = MFX_TARGETUSAGE_BALANCED;
	params->nWidth = WIDTH;
	params->nHeight = HEIGHT;
	params->nAsyncDepth = 2;
	params->nFpsNum = 30;
	params->nFpsDen = 1;
	params->nTargetBitRate = 2500;
	params->nMaxBitRate = 3000;
	params->nCodecProfile = MFX_PROFILE_AVC_HIGH;
	params->nRateControl = rate_control;
	params->nKeyIntSec = 2;
}

static qsv_t *open_encoder(qsv_param_t *params)
{
	qsv_t *ctx = qsv_encoder_open(params, QSV_CODEC_AVC, false);
	assert_non_null(ctx);
	return ctx;
}

static int encode_frame(qsv_t *ctx, uint64_t ts, mfxBitstream **bs)
{
	return qsv_encoder_encode(ctx, ts, plane_y, plane_uv, WIDTH, WIDTH, bs);
}

struct encode_job {
	qsv_t *ctx;
	std::atomic<bool> done{false};
	int ret = 0;
	mfxBitstream *bs = nullptr;
};

/* The device reports busy for good: the encode gives up with an error in
 * bounded time instead of spinning forever under the shared lock. */
static void test_busy_device_fails_in_bounded_time(void **state)
{
	(void)state;
	qsv_param_t params;

	fake_defaults();
	default_params(&params, MFX_RATECONTROL_CBR);
	qsv_t *ctx = open_encoder(&params);

	encode_mode = ENCODE_BUSY;
	encode_job job;
	job.ctx = ctx;
	auto start = std::chrono::steady_clock::now();
	std::thread worker([&job] {
		job.ret = encode_frame(job.ctx, 0, &job.bs);
		job.done = true;
	});

	/* Watchdog: well above the encoder's own limit. */
	while (!job.done && std::chrono::steady_clock::now() - start < std::chrono::seconds(15)) {
		std::this_thread::sleep_for(std::chrono::milliseconds(10));
	}

	bool hung = !job.done;
	auto elapsed = std::chrono::steady_clock::now() - start;

	/* Let a stuck retry loop out before failing. */
	encode_mode = ENCODE_OK;
	worker.join();

	if (hung) {
		fprintf(stderr, "EncodeFrameAsync retried for more than 15 s on a busy device\n");
		qsv_encoder_close(ctx);
		fail();
	}
	fprintf(stderr, "busy device: encode returned %d after %lld ms\n", job.ret,
		(long long)std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count());
	assert_int_equal(job.ret, -1);

	qsv_encoder_close(ctx);
}

/* The device stops completing frames: a SyncOperation timeout is an error,
 * not a finished frame with an empty bitstream. */
static void test_sync_timeout_is_an_error(void **state)
{
	(void)state;
	qsv_param_t params;
	mfxBitstream *bs = nullptr;

	fake_defaults();
	default_params(&params, MFX_RATECONTROL_CBR);
	qsv_t *ctx = open_encoder(&params);

	/* Fill the two-task pool. */
	assert_int_equal(encode_frame(ctx, 0, &bs), 0);
	assert_int_equal(encode_frame(ctx, 1, &bs), 0);

	/* A working device: the next frame waits for the first. */
	assert_int_equal(encode_frame(ctx, 2, &bs), 0);
	assert_non_null(bs);
	assert_true(bs->DataLength > 0);

	sync_mode = SYNC_TIMEOUT;
	bs = nullptr;
	int ret = encode_frame(ctx, 3, &bs);
	if (ret == 0 && bs && bs->DataLength == 0) {
		fprintf(stderr, "a sync timeout was reported as a frame with an empty bitstream\n");
		qsv_encoder_close(ctx);
		fail();
	}
	assert_int_equal(ret, -1);

	qsv_encoder_close(ctx);
}

/* VBR: a live change reaches the reset with the new target and maximum. */
static void test_reconfig_vbr(void **state)
{
	(void)state;
	qsv_param_t params;

	fake_defaults();
	default_params(&params, MFX_RATECONTROL_VBR);
	qsv_t *ctx = open_encoder(&params);

	params.nTargetBitRate = 6000;
	params.nMaxBitRate = 8000;
	assert_true(qsv_encoder_reconfig(ctx, &params));
	assert_int_equal(reset_calls, 1);
	assert_int_equal(reset_params.mfx.RateControlMethod, MFX_RATECONTROL_VBR);
	assert_int_equal(reset_params.mfx.TargetKbps, 6000);
	assert_int_equal(reset_params.mfx.MaxKbps, 8000);

	qsv_encoder_close(ctx);
}

/* CBR with lookahead ("low"/"normal" latency) on a platform without
 * LowPower: the encoder's own rate control is still CBR, so the new target
 * applies. */
static void test_reconfig_cbr_lookahead(void **state)
{
	(void)state;
	qsv_param_t params;

	fake_defaults();
	fake_platform = PLATFORM_ICELAKE;
	default_params(&params, MFX_RATECONTROL_CBR);
	params.nLADEPTH = 30;
	qsv_t *ctx = open_encoder(&params);

	params.nTargetBitRate = 6000;
	assert_true(qsv_encoder_reconfig(ctx, &params));
	assert_int_equal(reset_calls, 1);
	assert_int_equal(reset_params.mfx.TargetKbps, 6000);

	qsv_encoder_close(ctx);
}

/* The runtime adjusted some parameter but applied the reset: success. */
static void test_reconfig_warning_is_applied(void **state)
{
	(void)state;
	qsv_param_t params;

	fake_defaults();
	default_params(&params, MFX_RATECONTROL_CBR);
	qsv_t *ctx = open_encoder(&params);

	reset_status = MFX_WRN_INCOMPATIBLE_VIDEO_PARAM;
	params.nTargetBitRate = 4000;
	assert_true(qsv_encoder_reconfig(ctx, &params));

	reset_status = MFX_ERR_INVALID_VIDEO_PARAM;
	params.nTargetBitRate = 5000;
	assert_false(qsv_encoder_reconfig(ctx, &params));

	qsv_encoder_close(ctx);
}

/* Applying settings that leave the bitrate alone (the recording encoder when
 * only the stream bitrate changed) does not reset the encoder. */
static void test_reconfig_unchanged_skips_reset(void **state)
{
	(void)state;
	qsv_param_t params;

	fake_defaults();
	default_params(&params, MFX_RATECONTROL_CBR);
	qsv_t *ctx = open_encoder(&params);

	assert_true(qsv_encoder_reconfig(ctx, &params));
	assert_int_equal(reset_calls, 0);

	qsv_encoder_close(ctx);
}

/* Optimized CBR (Battlemage+): the sliding-window maximum follows the new
 * target instead of staying at 1.3x the starting bitrate. */
static void test_reconfig_winbrc_follows_target(void **state)
{
	(void)state;
#if (MFX_VERSION_MAJOR >= 2 && MFX_VERSION_MINOR >= 13) || MFX_VERSION_MAJOR > 2
	qsv_param_t params;

	fake_defaults();
	fake_platform = PLATFORM_BATTLEMAGE;
	default_params(&params, MFX_RATECONTROL_CBR);
	params.nTargetBitRate = 2000;
	qsv_t *ctx = open_encoder(&params);

	params.nTargetBitRate = 6000;
	assert_true(qsv_encoder_reconfig(ctx, &params));
	assert_int_equal(reset_params.mfx.TargetKbps, 6000);
	assert_int_equal(reset_winbrc_max_avg, 7800);

	qsv_encoder_close(ctx);
#else
	skip();
#endif
}

/* The constructor's probe unloads its VPL loader whether or not it gets a
 * session (#2396). */
static void test_probe_unloads_loader(void **state)
{
	(void)state;

	for (int fails = 0; fails < 2; fails++) {
		fake_defaults();
		session_fails = fails;
		mfxVersion version = {{0, 1}};
		delete new QSV_Encoder_Internal(version, false);
		if (loaders) {
			fprintf(stderr, "the probe left %d VPL loaders when its session %s\n", loaders,
				fails ? "failed" : "opened");
			fail();
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_busy_device_fails_in_bounded_time),
		cmocka_unit_test(test_sync_timeout_is_an_error),
		cmocka_unit_test(test_reconfig_vbr),
		cmocka_unit_test(test_reconfig_cbr_lookahead),
		cmocka_unit_test(test_reconfig_warning_is_applied),
		cmocka_unit_test(test_reconfig_unchanged_skips_reset),
		cmocka_unit_test(test_reconfig_winbrc_follows_target),
		cmocka_unit_test(test_probe_unloads_loader),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
