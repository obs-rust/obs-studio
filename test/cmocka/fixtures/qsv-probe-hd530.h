/* "QSV probe:" lines of an OBS log on an Intel HD Graphics 530 (Skylake,
 * i915, Ubuntu 24.04 iHD driver), for test_qsv_probe (obs-rust/obs-studio#1863).
 *
 * PROVISIONAL: written from the #1720 reporter's vainfo output (H.264 and
 * HEVC EncSlice, no AV1) before the probe logged these lines. Replace them,
 * verbatim, with the lines from the hardware run on #1863; the assertions
 * in test_qsv_probe.c then pin that hardware's result. */

static const char *const qsv_probe_hd530_log[] = {
	"info: QSV probe: node=/dev/dri/renderD128 opened=1 hevc=1 av1=0 driver=Intel iHD driver for Intel(R) Gen Graphics - 24.1.0 ()",
	NULL,
};
