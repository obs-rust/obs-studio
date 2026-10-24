/* Characterization of the output, encoder and service registration rules of
 * libobs/obs-module.c.h (obs-rust/obs-studio#1876, slice 5), moved to
 * libobs/obs-module-type-rules.c before their port to rust/obs-core
 * (module_type_rules). It pins the CURRENT behaviour; cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * Two parts:
 * - the steps: the four hidden functions of obs-module-type-rules.c,
 *   compiled into this test from the moved file: every required callback
 *   for each output flag combination, each encoder type and capability set,
 *   each service callback, and the protocol dedupe;
 * - the callers: obs_register_output_s(), obs_register_encoder_s() and
 *   obs_register_service_s() of the real libobs, through the public API
 *   (obs_enum_*_types(), obs_get_output_flags(), obs_get_encoder_caps(),
 *   obs_get_encoder_type(), obs_enum_output_protocols(), the display names
 *   and the log handler): every message and its level, in order, the sizes
 *   at 0, below, at and above sizeof(), the order of the checks, the
 *   protocol merge and when the type data is freed. These run on libobs with
 *   the C backend, the hybrid libobs and libobs-c-reference.
 *
 * The checks run in this order, and the first that fails is the only one
 * logged: the duplicate id, (encoders) a texture encoder that scales itself,
 * the required callbacks, then the size. Every case registers its own ids,
 * because a registration is never undone.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. The caller cases skip only when no display is set at all; with one
 * set, a failed obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <obs-module-type-rules.h>
#include <util/base.h>
#include <util/bmem.h>

/* ================================================================ steps */

#define OUT_V OBS_MODULE_TYPE_RULES_OUTPUT_VIDEO
#define OUT_A OBS_MODULE_TYPE_RULES_OUTPUT_AUDIO
#define OUT_E OBS_MODULE_TYPE_RULES_OUTPUT_ENCODED
#define OUT_S OBS_MODULE_TYPE_RULES_OUTPUT_SERVICE
#define OUT_M OBS_MODULE_TYPE_RULES_OUTPUT_MULTI_TRACK

/* The callbacks the rules look at, in the order the C checks them. */
enum cb {
	GET_NAME,
	CREATE,
	DESTROY,
	START,
	STOP,
	PROTOCOLS,
	ENCODED_PACKET,
	RAW_VIDEO,
	RAW_AUDIO,
	RAW_AUDIO2,
	ENCODE,
	ENCODE_TEXTURE,
	ENCODE_TEXTURE2,
	GET_FRAME_SIZE,
	GET_PROTOCOL,
	CB_NUM,
	END = CB_NUM,
};

static const uint32_t cb_bit[CB_NUM] = {
	OBS_MODULE_TYPE_RULES_HAS_GET_NAME,
	OBS_MODULE_TYPE_RULES_HAS_CREATE,
	OBS_MODULE_TYPE_RULES_HAS_DESTROY,
	OBS_MODULE_TYPE_RULES_HAS_START,
	OBS_MODULE_TYPE_RULES_HAS_STOP,
	OBS_MODULE_TYPE_RULES_HAS_PROTOCOLS,
	OBS_MODULE_TYPE_RULES_HAS_ENCODED_PACKET,
	OBS_MODULE_TYPE_RULES_HAS_RAW_VIDEO,
	OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO,
	OBS_MODULE_TYPE_RULES_HAS_RAW_AUDIO2,
	OBS_MODULE_TYPE_RULES_HAS_ENCODE,
	OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE,
	OBS_MODULE_TYPE_RULES_HAS_ENCODE_TEXTURE2,
	OBS_MODULE_TYPE_RULES_HAS_GET_FRAME_SIZE,
	OBS_MODULE_TYPE_RULES_HAS_GET_PROTOCOL,
};

/* The code for a missing callback. encode_texture and encode_texture2 share
 * NO_ENCODE_TEXTURE ("neither"). */
static const enum obs_module_type_rules_error cb_missing[CB_NUM] = {
	OBS_MODULE_TYPE_RULES_NO_GET_NAME,       OBS_MODULE_TYPE_RULES_NO_CREATE,
	OBS_MODULE_TYPE_RULES_NO_DESTROY,        OBS_MODULE_TYPE_RULES_NO_START,
	OBS_MODULE_TYPE_RULES_NO_STOP,           OBS_MODULE_TYPE_RULES_NO_PROTOCOLS,
	OBS_MODULE_TYPE_RULES_NO_ENCODED_PACKET, OBS_MODULE_TYPE_RULES_NO_RAW_VIDEO,
	OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO,      OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO2,
	OBS_MODULE_TYPE_RULES_NO_ENCODE,         OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE,
	OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE, OBS_MODULE_TYPE_RULES_NO_GET_FRAME_SIZE,
	OBS_MODULE_TYPE_RULES_NO_GET_PROTOCOL,
};

#define ALL_CBS ((1u << 15) - 1)

static uint32_t without(uint32_t callbacks, enum cb c)
{
	return callbacks & ~cb_bit[c];
}

/* Every output flag combination of the five flags the rules read, and the
 * callbacks it needs after get_name, create, destroy, start and stop, in the
 * order they are checked. */
static const struct {
	uint32_t flags;
	enum cb needs[4];
} output_needs[] = {
	{0, {END}},
	{OUT_V, {RAW_VIDEO, END}},
	{OUT_A, {RAW_AUDIO, END}},
	{OUT_V | OUT_A, {RAW_VIDEO, RAW_AUDIO, END}},
	{OUT_E, {ENCODED_PACKET, END}},
	{OUT_V | OUT_E, {ENCODED_PACKET, END}},
	{OUT_A | OUT_E, {ENCODED_PACKET, END}},
	{OUT_V | OUT_A | OUT_E, {ENCODED_PACKET, END}},
	{OUT_S, {PROTOCOLS, END}},
	{OUT_V | OUT_S, {PROTOCOLS, RAW_VIDEO, END}},
	{OUT_A | OUT_S, {PROTOCOLS, RAW_AUDIO, END}},
	{OUT_V | OUT_A | OUT_S, {PROTOCOLS, RAW_VIDEO, RAW_AUDIO, END}},
	{OUT_E | OUT_S, {PROTOCOLS, ENCODED_PACKET, END}},
	{OUT_V | OUT_E | OUT_S, {PROTOCOLS, ENCODED_PACKET, END}},
	{OUT_A | OUT_E | OUT_S, {PROTOCOLS, ENCODED_PACKET, END}},
	{OUT_V | OUT_A | OUT_E | OUT_S, {PROTOCOLS, ENCODED_PACKET, END}},
	{OUT_M, {END}},
	{OUT_V | OUT_M, {RAW_VIDEO, END}},
	{OUT_A | OUT_M, {RAW_AUDIO2, END}},
	{OUT_V | OUT_A | OUT_M, {RAW_VIDEO, RAW_AUDIO2, END}},
	{OUT_E | OUT_M, {ENCODED_PACKET, END}},
	{OUT_V | OUT_E | OUT_M, {ENCODED_PACKET, END}},
	{OUT_A | OUT_E | OUT_M, {ENCODED_PACKET, END}},
	{OUT_V | OUT_A | OUT_E | OUT_M, {ENCODED_PACKET, END}},
	{OUT_S | OUT_M, {PROTOCOLS, END}},
	{OUT_V | OUT_S | OUT_M, {PROTOCOLS, RAW_VIDEO, END}},
	{OUT_A | OUT_S | OUT_M, {PROTOCOLS, RAW_AUDIO2, END}},
	{OUT_V | OUT_A | OUT_S | OUT_M, {PROTOCOLS, RAW_VIDEO, RAW_AUDIO2, END}},
	{OUT_E | OUT_S | OUT_M, {PROTOCOLS, ENCODED_PACKET, END}},
	{OUT_V | OUT_E | OUT_S | OUT_M, {PROTOCOLS, ENCODED_PACKET, END}},
	{OUT_A | OUT_E | OUT_S | OUT_M, {PROTOCOLS, ENCODED_PACKET, END}},
	{OUT_V | OUT_A | OUT_E | OUT_S | OUT_M, {PROTOCOLS, ENCODED_PACKET, END}},
};

static const enum cb output_base[] = {GET_NAME, CREATE, DESTROY, START, STOP};

/* The callbacks a combination needs, in order, ending with END. */
static size_t output_required(size_t row, enum cb *out)
{
	size_t n = 0;
	for (size_t i = 0; i < 5; i++)
		out[n++] = output_base[i];
	for (size_t i = 0; output_needs[row].needs[i] != END; i++)
		out[n++] = output_needs[row].needs[i];
	return n;
}

static bool in(const enum cb *list, size_t n, enum cb c)
{
	for (size_t i = 0; i < n; i++)
		if (list[i] == c)
			return true;
	return false;
}

static void expect_code(enum obs_module_type_rules_error got, enum obs_module_type_rules_error want, const char *what,
			uint32_t a, uint32_t b)
{
	if (got != want) {
		fprintf(stderr, "%s 0x%x 0x%x: got %d, want %d\n", what, (unsigned)a, (unsigned)b, (int)got, (int)want);
		fail();
	}
}

#define output_check obs_module_type_rules_output_check
#define encoder_check obs_module_type_rules_encoder_check
#define service_check obs_module_type_rules_service_check

/* Each combination: with every callback it registers; without one it needs,
 * that one is reported; without one it does not need, it still registers;
 * with none, get_name is reported; and when several are missing the first in
 * the C's order wins. */
static void test_output_flag_combinations(void **state)
{
	(void)state;
	assert_int_equal(sizeof(output_needs) / sizeof(output_needs[0]), 32);

	for (size_t row = 0; row < 32; row++) {
		uint32_t flags = output_needs[row].flags;
		enum cb req[CB_NUM];
		size_t n = output_required(row, req);

		expect_code(output_check(flags, ALL_CBS), OBS_MODULE_TYPE_RULES_OK, "all", flags, ALL_CBS);
		expect_code(output_check(flags, 0), OBS_MODULE_TYPE_RULES_NO_GET_NAME, "none", flags, 0);

		uint32_t only = 0;
		for (size_t i = 0; i < n; i++)
			only |= cb_bit[req[i]];
		expect_code(output_check(flags, only), OBS_MODULE_TYPE_RULES_OK, "only required", flags, only);

		for (int c = 0; c < CB_NUM; c++) {
			uint32_t cbs = without(ALL_CBS, (enum cb)c);
			enum obs_module_type_rules_error want = in(req, n, (enum cb)c) ? cb_missing[c]
										       : OBS_MODULE_TYPE_RULES_OK;
			expect_code(output_check(flags, cbs), want, "without one", flags, cbs);
		}

		/* Missing the last two required: the earlier one is reported. */
		for (size_t i = 0; i + 1 < n; i++) {
			uint32_t cbs = without(without(only, req[i]), req[n - 1]);
			expect_code(output_check(flags, cbs), cb_missing[req[i]], "two missing", flags, cbs);
		}

		/* Missing every required one from the i-th on: the i-th. */
		uint32_t cbs = only;
		for (size_t i = n; i-- > 0;) {
			cbs = without(cbs, req[i]);
			expect_code(output_check(flags, cbs), cb_missing[req[i]], "tail missing", flags, cbs);
		}
	}
}

/* Flag bits the rules do not read change nothing: CAN_PAUSE, multi-track
 * video and anything above. */
static void test_output_other_flags(void **state)
{
	(void)state;
	const uint32_t others[] = {OBS_OUTPUT_CAN_PAUSE, OBS_OUTPUT_MULTI_TRACK_VIDEO, 1u << 7, 1u << 31,
				   ~(uint32_t)(OUT_V | OUT_A | OUT_E | OUT_S | OUT_M)};
	for (size_t row = 0; row < 32; row++) {
		uint32_t flags = output_needs[row].flags;
		for (size_t o = 0; o < sizeof(others) / sizeof(others[0]); o++) {
			for (int c = 0; c <= CB_NUM; c++) {
				uint32_t cbs = c == CB_NUM ? ALL_CBS : without(ALL_CBS, (enum cb)c);
				expect_code(output_check(flags | others[o], cbs), output_check(flags, cbs),
					    "other flags", flags | others[o], cbs);
			}
		}
	}

	/* Callback bits the output rules do not read change nothing either. */
	uint32_t foreign = cb_bit[ENCODE] | cb_bit[ENCODE_TEXTURE] | cb_bit[ENCODE_TEXTURE2] | cb_bit[GET_FRAME_SIZE] |
			   cb_bit[GET_PROTOCOL] | 0xFFFF8000u;
	for (size_t row = 0; row < 32; row++)
		expect_code(output_check(output_needs[row].flags, ALL_CBS & ~foreign), OBS_MODULE_TYPE_RULES_OK,
			    "foreign bits", output_needs[row].flags, ALL_CBS & ~foreign);
	expect_code(output_check(0, foreign), OBS_MODULE_TYPE_RULES_NO_GET_NAME, "foreign bits", 0, foreign);

	/* The documented combinations by name. */
	expect_code(output_check(OBS_OUTPUT_AV, without(ALL_CBS, RAW_AUDIO)), OBS_MODULE_TYPE_RULES_NO_RAW_AUDIO, "AV",
		    OBS_OUTPUT_AV, 0);
	expect_code(output_check(OBS_OUTPUT_AV | OBS_OUTPUT_MULTI_TRACK_AV, without(ALL_CBS, RAW_AUDIO)),
		    OBS_MODULE_TYPE_RULES_OK, "AV multi-track", OBS_OUTPUT_AV | OBS_OUTPUT_MULTI_TRACK_AV, 0);
	expect_code(output_check(OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_SERVICE, without(ALL_CBS, PROTOCOLS)),
		    OBS_MODULE_TYPE_RULES_NO_PROTOCOLS, "rtmp-like", 0, 0);
	/* Multi-track alone needs no audio callback (characterized, not
	 * endorsed: it is an audio flag only with OBS_OUTPUT_AUDIO). */
	expect_code(output_check(OBS_OUTPUT_MULTI_TRACK, without(without(ALL_CBS, RAW_AUDIO), RAW_AUDIO2)),
		    OBS_MODULE_TYPE_RULES_OK, "multi-track alone", OBS_OUTPUT_MULTI_TRACK, 0);
}

#define CAP_PASS OBS_ENCODER_CAP_PASS_TEXTURE
#define CAP_SCALE OBS_ENCODER_CAP_SCALING
#define CAP_OTHERS                                                                                    \
	(OBS_ENCODER_CAP_DEPRECATED | OBS_ENCODER_CAP_DYN_BITRATE | OBS_ENCODER_CAP_INTERNAL | \
	 OBS_ENCODER_CAP_ROI | OBS_ENCODER_CAP_MULTITRACK_DYN_BITRATE)

/* The encoder types the rules tell apart: audio, and everything else. */
static const uint32_t encoder_types[] = {OBS_ENCODER_AUDIO, OBS_ENCODER_VIDEO, 2, 0x80000000u, UINT32_MAX};

static void test_encoder_texture_scaling(void **state)
{
	(void)state;
	const uint32_t caps[] = {CAP_PASS | CAP_SCALE, CAP_PASS | CAP_SCALE | CAP_OTHERS, UINT32_MAX};
	for (size_t t = 0; t < sizeof(encoder_types) / sizeof(encoder_types[0]); t++) {
		for (size_t c = 0; c < sizeof(caps) / sizeof(caps[0]); c++) {
			/* Before any callback: with all, with none. */
			expect_code(encoder_check(encoder_types[t], caps[c], ALL_CBS),
				    OBS_MODULE_TYPE_RULES_TEXTURE_SCALING, "scaling", encoder_types[t], caps[c]);
			expect_code(encoder_check(encoder_types[t], caps[c], 0), OBS_MODULE_TYPE_RULES_TEXTURE_SCALING,
				    "scaling", encoder_types[t], caps[c]);
		}
		/* Scaling alone, and a texture encoder that does not scale, are
		 * fine. */
		expect_code(encoder_check(encoder_types[t], CAP_SCALE, ALL_CBS), OBS_MODULE_TYPE_RULES_OK, "scale",
			    encoder_types[t], CAP_SCALE);
		expect_code(encoder_check(encoder_types[t], CAP_PASS, ALL_CBS), OBS_MODULE_TYPE_RULES_OK, "pass",
			    encoder_types[t], CAP_PASS);
		expect_code(encoder_check(encoder_types[t], UINT32_MAX & ~CAP_SCALE, ALL_CBS), OBS_MODULE_TYPE_RULES_OK,
			    "all but scale", encoder_types[t], UINT32_MAX & ~CAP_SCALE);
		expect_code(encoder_check(encoder_types[t], UINT32_MAX & ~CAP_PASS, ALL_CBS), OBS_MODULE_TYPE_RULES_OK,
			    "all but pass", encoder_types[t], UINT32_MAX & ~CAP_PASS);
	}
}

static void test_encoder_callbacks(void **state)
{
	(void)state;
	const uint32_t caps[] = {0, CAP_SCALE, CAP_OTHERS, CAP_PASS, CAP_PASS | CAP_OTHERS};
	for (size_t t = 0; t < sizeof(encoder_types) / sizeof(encoder_types[0]); t++) {
		uint32_t type = encoder_types[t];
		bool audio = type == OBS_ENCODER_AUDIO;
		for (size_t c = 0; c < sizeof(caps) / sizeof(caps[0]); c++) {
			bool texture = (caps[c] & CAP_PASS) != 0;
			enum cb req[6] = {GET_NAME, CREATE, DESTROY};
			size_t n = 3;
			if (!texture)
				req[n++] = ENCODE;
			if (audio)
				req[n++] = GET_FRAME_SIZE;

			expect_code(encoder_check(type, caps[c], ALL_CBS), OBS_MODULE_TYPE_RULES_OK, "all", type,
				    caps[c]);
			expect_code(encoder_check(type, caps[c], 0), OBS_MODULE_TYPE_RULES_NO_GET_NAME, "none", type,
				    caps[c]);

			for (int cb = 0; cb < CB_NUM; cb++) {
				uint32_t cbs = without(ALL_CBS, (enum cb)cb);
				enum obs_module_type_rules_error want =
					in(req, n, (enum cb)cb) ? cb_missing[cb] : OBS_MODULE_TYPE_RULES_OK;
				expect_code(encoder_check(type, caps[c], cbs), want, "without one", type, cbs);
			}

			/* encode for a plain encoder, either texture callback for a
			 * texture one; the other kind does not count. */
			uint32_t base = cb_bit[GET_NAME] | cb_bit[CREATE] | cb_bit[DESTROY] |
					(audio ? cb_bit[GET_FRAME_SIZE] : 0);
			uint32_t tex = cb_bit[ENCODE_TEXTURE] | cb_bit[ENCODE_TEXTURE2];
			if (texture) {
				expect_code(encoder_check(type, caps[c], base | cb_bit[ENCODE_TEXTURE]),
					    OBS_MODULE_TYPE_RULES_OK, "texture", type, caps[c]);
				expect_code(encoder_check(type, caps[c], base | cb_bit[ENCODE_TEXTURE2]),
					    OBS_MODULE_TYPE_RULES_OK, "texture2", type, caps[c]);
				expect_code(encoder_check(type, caps[c], base | cb_bit[ENCODE]),
					    OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE, "encode only", type, caps[c]);
				expect_code(encoder_check(type, caps[c], base), OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE,
					    "neither", type, caps[c]);
			} else {
				expect_code(encoder_check(type, caps[c], base | cb_bit[ENCODE]),
					    OBS_MODULE_TYPE_RULES_OK, "encode", type, caps[c]);
				expect_code(encoder_check(type, caps[c], base | tex), OBS_MODULE_TYPE_RULES_NO_ENCODE,
					    "texture only", type, caps[c]);
			}

			/* The encode rule comes before get_frame_size. */
			if (audio) {
				uint32_t cbs = cb_bit[GET_NAME] | cb_bit[CREATE] | cb_bit[DESTROY];
				expect_code(encoder_check(type, caps[c], cbs),
					    texture ? OBS_MODULE_TYPE_RULES_NO_ENCODE_TEXTURE
						    : OBS_MODULE_TYPE_RULES_NO_ENCODE,
					    "encode before frame size", type, caps[c]);
			}
		}
	}
}

static void test_service_callbacks(void **state)
{
	(void)state;
	const enum cb req[] = {GET_NAME, CREATE, DESTROY, GET_PROTOCOL};
	expect_code(service_check(ALL_CBS), OBS_MODULE_TYPE_RULES_OK, "all", 0, ALL_CBS);
	expect_code(service_check(UINT32_MAX), OBS_MODULE_TYPE_RULES_OK, "all bits", 0, UINT32_MAX);
	expect_code(service_check(0), OBS_MODULE_TYPE_RULES_NO_GET_NAME, "none", 0, 0);

	uint32_t only = 0;
	for (size_t i = 0; i < 4; i++)
		only |= cb_bit[req[i]];
	expect_code(service_check(only), OBS_MODULE_TYPE_RULES_OK, "only required", 0, only);

	for (int c = 0; c < CB_NUM; c++) {
		uint32_t cbs = without(ALL_CBS, (enum cb)c);
		enum obs_module_type_rules_error want = in(req, 4, (enum cb)c) ? cb_missing[c]
									       : OBS_MODULE_TYPE_RULES_OK;
		expect_code(service_check(cbs), want, "without one", 0, cbs);
	}
	uint32_t cbs = only;
	for (size_t i = 4; i-- > 0;) {
		cbs = without(cbs, req[i]);
		expect_code(service_check(cbs), cb_missing[req[i]], "tail missing", 0, cbs);
	}
}

#define listed obs_module_type_rules_protocol_listed

static void test_protocol_listed(void **state)
{
	(void)state;
	char a[] = "RTMP", b[] = "RTMPS", c[] = "SRT", e[] = "", lower[] = "rtmp", sp[] = " RIST";
	char *list[] = {a, b, c, e, lower, sp, a};

	assert_false(listed("RTMP", NULL, 0));
	assert_false(listed("", NULL, 0));
	assert_true(listed("RTMP", list, 7));
	assert_true(listed("RTMPS", list, 7));
	assert_true(listed("SRT", list, 7));
	assert_true(listed("rtmp", list, 7));
	assert_true(listed(" RIST", list, 7));
	/* An empty protocol matches an empty entry only. */
	assert_true(listed("", list, 7));
	assert_false(listed("", list, 3));
	/* Whole strings, case-sensitive, nothing trimmed. */
	assert_false(listed("RTM", list, 7));
	assert_false(listed("RTMPSX", list, 7));
	assert_false(listed("Rtmp", list, 7));
	assert_false(listed("RIST", list, 7));
	assert_false(listed("SRT ", list, 7));
	/* Only the first num entries are looked at. */
	assert_false(listed("SRT", list, 2));
	assert_true(listed("SRT", list, 3));
	assert_false(listed("RTMP", list + 1, 5));
	assert_true(listed("RTMP", list + 1, 6));
	/* Bytes past ASCII compare as bytes. */
	char u1[] = "\xc3\xa9t\xc3\xa9", u2[] = "\xc3\x89T\xc3\x89";
	char *ul[] = {u1};
	assert_true(listed("\xc3\xa9t\xc3\xa9", ul, 1));
	assert_false(listed(u2, ul, 1));
}

/* =============================================================== callers */

/* ------------------------------------------------------------ log capture */

#define LOG_MAX 8

struct logged {
	int level;
	char msg[512];
};

static struct logged logged[LOG_MAX];
static size_t logged_num;
static bool capturing;
static log_handler_t prev_handler;
static void *prev_param;

/* Only the registration's messages are kept: they name obs_register_* or
 * start "Tried to register". Anything else goes to the previous handler. */
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (capturing && (strstr(format, "obs_register_") || strstr(format, "Tried to register"))) {
		if (logged_num < LOG_MAX) {
			logged[logged_num].level = level;
			vsnprintf(logged[logged_num].msg, sizeof(logged[0].msg), format, args);
		}
		logged_num++;
		return;
	}
	if (prev_handler)
		prev_handler(level, format, args, prev_param);
}

static void expect_logged(size_t num, ...)
{
	va_list args;
	va_start(args, num);
	if (logged_num != num) {
		for (size_t i = 0; i < logged_num && i < LOG_MAX; i++)
			fprintf(stderr, "logged %zu: %s\n", i, logged[i].msg);
	}
	assert_int_equal(logged_num, num);
	for (size_t i = 0; i < num; i++) {
		int level = va_arg(args, int);
		const char *msg = va_arg(args, const char *);
		assert_string_equal(logged[i].msg, msg);
		assert_int_equal(logged[i].level, level);
	}
	va_end(args);
}

#define W LOG_WARNING
#define E LOG_ERROR
#define REQUIRED(val, id, func) "Required value '" val "' for '" id "' not found.  " func " failed."
#define OUT_REQUIRED(val, id) REQUIRED(val, id, "obs_register_output")
#define ENC_REQUIRED(val, id) REQUIRED(val, id, "obs_register_encoder")
#define SVC_REQUIRED(val, id) REQUIRED(val, id, "obs_register_service")

/* --------------------------------------------------------------- callbacks */

static char type_data_tag[] = "Registered";
static int frees;
static void *freed;

static void free_type_data(void *type_data)
{
	frees++;
	freed = type_data;
}

static const char *get_name(void *type_data)
{
	return type_data ? (const char *)type_data : "(no type data)";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return NULL;
}

static void destroy(void *data)
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

static void raw_video(void *data, struct video_data *frame)
{
	(void)data;
	(void)frame;
}

static void raw_audio(void *data, struct audio_data *frames)
{
	(void)data;
	(void)frames;
}

static void raw_audio2(void *data, size_t idx, struct audio_data *frames)
{
	(void)data;
	(void)idx;
	(void)frames;
}

static void encoded_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return NULL;
}

static bool encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received_packet;
	return false;
}

static size_t get_frame_size(void *data)
{
	(void)data;
	return 1024;
}

static bool encode_texture(void *data, uint32_t handle, int64_t pts, uint64_t lock_key, uint64_t *next_key,
			   struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)handle;
	(void)pts;
	(void)lock_key;
	(void)next_key;
	(void)packet;
	(void)received_packet;
	return false;
}

static bool encode_texture2(void *data, struct encoder_texture *texture, int64_t pts, uint64_t lock_key,
			    uint64_t *next_key, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)texture;
	(void)pts;
	(void)lock_key;
	(void)next_key;
	(void)packet;
	(void)received_packet;
	return false;
}

static void *service_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	(void)service;
	return NULL;
}

static const char *get_protocol(void *data)
{
	(void)data;
	return "RTMP";
}

/* Types with every callback the registration checks, type data and
 * free_type_data. */
static struct obs_output_info full_output(const char *id, uint32_t flags, const char *protocols)
{
	struct obs_output_info info = {0};
	info.id = id;
	info.flags = flags;
	info.get_name = get_name;
	info.create = output_create;
	info.destroy = destroy;
	info.start = output_start;
	info.stop = output_stop;
	info.raw_video = raw_video;
	info.raw_audio = raw_audio;
	info.raw_audio2 = raw_audio2;
	info.encoded_packet = encoded_packet;
	info.protocols = protocols;
	info.type_data = type_data_tag;
	info.free_type_data = free_type_data;
	return info;
}

static struct obs_encoder_info full_encoder(const char *id, enum obs_encoder_type type, uint32_t caps)
{
	struct obs_encoder_info info = {0};
	info.id = id;
	info.type = type;
	info.codec = "mtr_codec";
	info.caps = caps;
	info.get_name = get_name;
	info.create = encoder_create;
	info.destroy = destroy;
	info.encode = encode;
	info.get_frame_size = get_frame_size;
	info.encode_texture = encode_texture;
	info.encode_texture2 = encode_texture2;
	info.type_data = type_data_tag;
	info.free_type_data = free_type_data;
	return info;
}

static struct obs_service_info full_service(const char *id)
{
	struct obs_service_info info = {0};
	info.id = id;
	info.get_name = get_name;
	info.create = service_create;
	info.destroy = destroy;
	info.get_protocol = get_protocol;
	info.type_data = type_data_tag;
	info.free_type_data = free_type_data;
	return info;
}

/* Room after each structure, for a size above its sizeof(). The bytes after
 * the structure are a pattern. */
#define PADDED(T)                         \
	struct {                          \
		T info;                   \
		unsigned char after[64];  \
	}

static void begin(void)
{
	logged_num = 0;
	frees = 0;
	freed = NULL;
	capturing = true;
}

static void reg_output(const struct obs_output_info *info, size_t size)
{
	PADDED(struct obs_output_info) padded;
	memset(&padded, 0xA5, sizeof(padded));
	padded.info = *info;
	begin();
	obs_register_output_s(&padded.info, size);
	capturing = false;
}

static void reg_encoder(const struct obs_encoder_info *info, size_t size)
{
	PADDED(struct obs_encoder_info) padded;
	memset(&padded, 0xA5, sizeof(padded));
	padded.info = *info;
	begin();
	obs_register_encoder_s(&padded.info, size);
	capturing = false;
}

static void reg_service(const struct obs_service_info *info, size_t size)
{
	PADDED(struct obs_service_info) padded;
	memset(&padded, 0xA5, sizeof(padded));
	padded.info = *info;
	begin();
	obs_register_service_s(&padded.info, size);
	capturing = false;
}

/* ----------------------------------------------------------- expectations */

static bool type_listed(bool (*enum_types)(size_t, const char **), const char *id)
{
	const char *t;
	for (size_t i = 0; enum_types(i, &t); i++)
		if (strcmp(t, id) == 0)
			return true;
	return false;
}

static size_t type_count(bool (*enum_types)(size_t, const char **))
{
	const char *t;
	size_t i = 0;
	while (enum_types(i, &t))
		i++;
	return i;
}

static void expect_freed(bool type_data_freed)
{
	assert_int_equal(frees, type_data_freed ? 1 : 0);
	if (type_data_freed)
		assert_ptr_equal(freed, type_data_tag);
}

static void expect_output(const char *id, uint32_t flags)
{
	assert_true(type_listed(obs_enum_output_types, id));
	assert_int_equal(obs_get_output_flags(id), flags);
	assert_non_null(obs_output_get_display_name(id));
	assert_string_equal(obs_output_get_display_name(id), type_data_tag);
}

static void expect_no_output(const char *id, size_t types, bool type_data_freed)
{
	assert_false(type_listed(obs_enum_output_types, id));
	assert_int_equal(type_count(obs_enum_output_types), types);
	assert_int_equal(obs_get_output_flags(id), 0);
	expect_freed(type_data_freed);
}

static void expect_encoder(const char *id, enum obs_encoder_type type, uint32_t caps)
{
	assert_true(type_listed(obs_enum_encoder_types, id));
	assert_int_equal(obs_get_encoder_type(id), type);
	assert_int_equal(obs_get_encoder_caps(id), caps);
	assert_non_null(obs_encoder_get_display_name(id));
	assert_string_equal(obs_encoder_get_display_name(id), type_data_tag);
}

static void expect_no_encoder(const char *id, size_t types, bool type_data_freed)
{
	assert_false(type_listed(obs_enum_encoder_types, id));
	assert_int_equal(type_count(obs_enum_encoder_types), types);
	assert_null(obs_encoder_get_display_name(id));
	expect_freed(type_data_freed);
}

static void expect_service(const char *id)
{
	assert_true(type_listed(obs_enum_service_types, id));
	assert_non_null(obs_service_get_display_name(id));
	assert_string_equal(obs_service_get_display_name(id), type_data_tag);
}

static void expect_no_service(const char *id, size_t types, bool type_data_freed)
{
	assert_false(type_listed(obs_enum_service_types, id));
	assert_int_equal(type_count(obs_enum_service_types), types);
	assert_null(obs_service_get_display_name(id));
	expect_freed(type_data_freed);
}

/* The registered protocols from index from on, joined with ',' */
static void protocols_from(size_t from, char *out, size_t out_size)
{
	char *p;
	out[0] = 0;
	for (size_t i = from; obs_enum_output_protocols(i, &p); i++) {
		if (out[0])
			strncat(out, ",", out_size - strlen(out) - 1);
		strncat(out, p, out_size - strlen(out) - 1);
	}
}

static size_t protocol_count(void)
{
	char *p;
	size_t i = 0;
	while (obs_enum_output_protocols(i, &p))
		i++;
	return i;
}

/* ------------------------------------------------------------------ setup */

static bool obs_up;

static int group_setup(void **state)
{
	(void)state;
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, NULL);
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
	base_set_log_handler(prev_handler, prev_param);
	return 0;
}

#define REQUIRE_OBS()           \
	do {                    \
		if (!obs_up)    \
			skip(); \
	} while (false)

/* ---------------------------------------------------------------- outputs */

static void test_output_register(void **state)
{
	(void)state;
	REQUIRE_OBS();

	/* Every flag combination with every callback registers, with its flags
	 * as given. */
	/* libobs keeps the caller's id pointer, so the ids outlive the test. */
	static char ids[32][16];
	for (size_t row = 0; row < 32; row++) {
		char *id = ids[row];
		snprintf(id, sizeof(ids[0]), "mtr_out_%zu", row);
		struct obs_output_info info = full_output(id, output_needs[row].flags, "MTR_ALL");
		reg_output(&info, sizeof(info));
		expect_logged(0);
		expect_freed(false);
		expect_output(id, output_needs[row].flags);
	}

	/* Only the callbacks a type needs. */
	struct obs_output_info info = full_output("mtr_out_min_av", OBS_OUTPUT_AV, NULL);
	info.encoded_packet = NULL;
	info.raw_audio2 = NULL;
	reg_output(&info, sizeof(info));
	expect_logged(0);
	expect_output("mtr_out_min_av", OBS_OUTPUT_AV);

	info = full_output("mtr_out_min_enc", OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED, NULL);
	info.raw_video = NULL;
	info.raw_audio = NULL;
	info.raw_audio2 = NULL;
	reg_output(&info, sizeof(info));
	expect_logged(0);
	expect_output("mtr_out_min_enc", OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED);

	info = full_output("mtr_out_min_mt", OBS_OUTPUT_AUDIO | OBS_OUTPUT_MULTI_TRACK, NULL);
	info.raw_audio = NULL;
	reg_output(&info, sizeof(info));
	expect_logged(0);
	expect_output("mtr_out_min_mt", OBS_OUTPUT_AUDIO | OBS_OUTPUT_MULTI_TRACK);
}

static void test_output_required(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = type_count(obs_enum_output_types);
	const uint32_t every = OBS_OUTPUT_AV | OBS_OUTPUT_SERVICE;

#define OUTPUT_MISSING(field, flags, id, val)                       \
	do {                                                        \
		struct obs_output_info i_ = full_output(id, flags, "MTR_X"); \
		i_.field = NULL;                                    \
		reg_output(&i_, sizeof(i_));                        \
		expect_logged(1, E, OUT_REQUIRED(val, id));         \
		expect_no_output(id, types, true);                  \
	} while (false)

	OUTPUT_MISSING(get_name, every, "mtr_out_no_get_name", "get_name");
	OUTPUT_MISSING(create, every, "mtr_out_no_create", "create");
	OUTPUT_MISSING(destroy, every, "mtr_out_no_destroy", "destroy");
	OUTPUT_MISSING(start, every, "mtr_out_no_start", "start");
	OUTPUT_MISSING(stop, every, "mtr_out_no_stop", "stop");
	OUTPUT_MISSING(protocols, every, "mtr_out_no_protocols", "protocols");
	OUTPUT_MISSING(encoded_packet, OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED, "mtr_out_no_encoded", "encoded_packet");
	OUTPUT_MISSING(raw_video, OBS_OUTPUT_AV, "mtr_out_no_raw_video", "raw_video");
	OUTPUT_MISSING(raw_audio, OBS_OUTPUT_AV, "mtr_out_no_raw_audio", "raw_audio");
	OUTPUT_MISSING(raw_audio2, OBS_OUTPUT_AV | OBS_OUTPUT_MULTI_TRACK, "mtr_out_no_raw_audio2", "raw_audio2");

	/* The first missing one in the C's order is the only one logged. */
	struct obs_output_info info = full_output("mtr_out_no_many", every | OBS_OUTPUT_ENCODED, NULL);
	info.stop = NULL;
	info.encoded_packet = NULL;
	reg_output(&info, sizeof(info));
	expect_logged(1, E, OUT_REQUIRED("stop", "mtr_out_no_many"));
	expect_no_output("mtr_out_no_many", types, true);

	/* A failed registration adds no protocol. */
	size_t protocols = protocol_count();
	info = full_output("mtr_out_failed_service", OBS_OUTPUT_SERVICE, "MTR_NEVER");
	info.start = NULL;
	reg_output(&info, sizeof(info));
	expect_logged(1, E, OUT_REQUIRED("start", "mtr_out_failed_service"));
	assert_int_equal(protocol_count(), protocols);
	assert_false(obs_is_output_protocol_registered("MTR_NEVER"));

	/* Without free_type_data, or without type data, nothing is freed. */
	info = full_output("mtr_out_nofree", 0, NULL);
	info.get_name = NULL;
	info.free_type_data = NULL;
	reg_output(&info, sizeof(info));
	expect_no_output("mtr_out_nofree", types, false);
	info.free_type_data = free_type_data;
	info.type_data = NULL;
	reg_output(&info, sizeof(info));
	expect_no_output("mtr_out_nofree", types, false);
}

static void test_output_duplicate(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_output_info info = full_output("mtr_out_dup", OBS_OUTPUT_VIDEO, NULL);
	reg_output(&info, sizeof(info));
	expect_logged(0);
	size_t types = type_count(obs_enum_output_types);

	/* The duplicate is checked first: before the callbacks and the size. */
	info = full_output("mtr_out_dup", OBS_OUTPUT_AUDIO, NULL);
	info.get_name = NULL;
	reg_output(&info, sizeof(info) + 8);
	expect_logged(1, W, "obs_register_output: Output id 'mtr_out_dup' already exists!  Duplicate library?");
	assert_int_equal(type_count(obs_enum_output_types), types);
	expect_freed(true);
	/* The first registration stays. */
	expect_output("mtr_out_dup", OBS_OUTPUT_VIDEO);

	/* With a size of 0, nothing is copied, so nothing is freed. */
	reg_output(&info, 0);
	expect_logged(1, W, "obs_register_output: Output id 'mtr_out_dup' already exists!  Duplicate library?");
	expect_freed(false);
}

static void test_output_size(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = type_count(obs_enum_output_types);
	const size_t full = sizeof(struct obs_output_info);
	char msg[256];

	/* 0: no callback is inside the size, so get_name is missing (the
	 * "outside of obs_module_load" message is never reached:
	 * characterized, not endorsed), and nothing is freed. */
	struct obs_output_info info = full_output("mtr_out_size0", 0, NULL);
	reg_output(&info, 0);
	expect_logged(1, E, OUT_REQUIRED("get_name", "mtr_out_size0"));
	expect_no_output("mtr_out_size0", types, false);

	/* At the size: registered. */
	info = full_output("mtr_out_size_at", OBS_OUTPUT_AV | OBS_OUTPUT_SERVICE, "MTR_SIZE_AT");
	reg_output(&info, full);
	expect_logged(0);
	expect_output("mtr_out_size_at", OBS_OUTPUT_AV | OBS_OUTPUT_SERVICE);
	assert_true(obs_is_output_protocol_registered("MTR_SIZE_AT"));
	types++;

	/* Above: rejected after the callbacks, freed. */
	snprintf(msg, sizeof(msg),
		 "Tried to register obs_output_info with size %llu which is more than libobs currently supports (%llu)",
		 (unsigned long long)(full + 1), (unsigned long long)full);
	info = full_output("mtr_out_size_above", OBS_OUTPUT_AV, NULL);
	reg_output(&info, full + 1);
	expect_logged(1, E, msg);
	expect_no_output("mtr_out_size_above", types, true);

	/* Above, with a callback missing: the callback is reported. */
	info.get_name = NULL;
	reg_output(&info, full + 8);
	expect_logged(1, E, OUT_REQUIRED("get_name", "mtr_out_size_above"));
	expect_no_output("mtr_out_size_above", types, true);

	/* Below: a callback that ends past the size is missing, by one byte. */
	size_t protocols_end = offsetof(struct obs_output_info, protocols) + sizeof(info.protocols);
	info = full_output("mtr_out_size_cut", OBS_OUTPUT_SERVICE, "MTR_CUT");
	reg_output(&info, protocols_end - 1);
	expect_logged(1, E, OUT_REQUIRED("protocols", "mtr_out_size_cut"));
	expect_no_output("mtr_out_size_cut", types, true);
	assert_false(obs_is_output_protocol_registered("MTR_CUT"));

	size_t raw_audio2_end = offsetof(struct obs_output_info, raw_audio2) + sizeof(info.raw_audio2);
	info = full_output("mtr_out_size_cut2", OBS_OUTPUT_AUDIO | OBS_OUTPUT_MULTI_TRACK, NULL);
	reg_output(&info, raw_audio2_end - 1);
	expect_logged(1, E, OUT_REQUIRED("raw_audio2", "mtr_out_size_cut2"));
	expect_no_output("mtr_out_size_cut2", types, true);

	/* The same structure cut after raw_audio2 registers, without the parts
	 * past the size. */
	reg_output(&info, raw_audio2_end);
	expect_logged(0);
	expect_output("mtr_out_size_cut2", OBS_OUTPUT_AUDIO | OBS_OUTPUT_MULTI_TRACK);
	types++;

	/* Below, with no service: protocols is not needed and not copied, so
	 * nothing is split. */
	size_t protocols_count = protocol_count();
	info = full_output("mtr_out_size_noservice", OBS_OUTPUT_AV, "MTR_NOSERVICE");
	reg_output(&info, offsetof(struct obs_output_info, protocols));
	expect_logged(0);
	expect_output("mtr_out_size_noservice", OBS_OUTPUT_AV);
	assert_int_equal(protocol_count(), protocols_count);

	/* Cut before type_data or free_type_data: a failure frees nothing. */
	info = full_output("mtr_out_size_nofree", OBS_OUTPUT_SERVICE, "MTR_X");
	reg_output(&info, offsetof(struct obs_output_info, free_type_data));
	expect_logged(1, E, OUT_REQUIRED("protocols", "mtr_out_size_nofree"));
	expect_freed(false);
	reg_output(&info, offsetof(struct obs_output_info, free_type_data) + sizeof(info.free_type_data));
	expect_logged(1, E, OUT_REQUIRED("protocols", "mtr_out_size_nofree"));
	expect_freed(true);
}

static void test_output_protocols(void **state)
{
	(void)state;
	REQUIRE_OBS();
	char got[512];
	size_t from = protocol_count();

	/* Each new protocol once, in order; empty entries are dropped. */
	struct obs_output_info info = full_output("mtr_out_proto1", OBS_OUTPUT_SERVICE, "MTR_P1;MTR_P2");
	reg_output(&info, sizeof(info));
	expect_logged(0);
	protocols_from(from, got, sizeof(got));
	assert_string_equal(got, "MTR_P1,MTR_P2");

	info = full_output("mtr_out_proto2", OBS_OUTPUT_SERVICE | OBS_OUTPUT_ENCODED, ";MTR_P2;MTR_P3;;MTR_P3;MTR_P1;");
	reg_output(&info, sizeof(info));
	expect_logged(0);
	protocols_from(from, got, sizeof(got));
	assert_string_equal(got, "MTR_P1,MTR_P2,MTR_P3");

	/* Case-sensitive, nothing trimmed. */
	info = full_output("mtr_out_proto3", OBS_OUTPUT_SERVICE, "mtr_p1; MTR_P2;MTR_P2 ");
	reg_output(&info, sizeof(info));
	protocols_from(from, got, sizeof(got));
	assert_string_equal(got, "MTR_P1,MTR_P2,MTR_P3,mtr_p1, MTR_P2,MTR_P2 ");
	assert_true(obs_is_output_protocol_registered("mtr_p1"));
	assert_true(obs_is_output_protocol_registered(" MTR_P2"));

	/* Only the service flag splits: an output without it adds nothing. */
	size_t n = protocol_count();
	info = full_output("mtr_out_proto4", OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED, "MTR_P4");
	reg_output(&info, sizeof(info));
	expect_logged(0);
	assert_int_equal(protocol_count(), n);
	assert_false(obs_is_output_protocol_registered("MTR_P4"));

	/* An empty list, or only separators, adds nothing but registers. */
	info = full_output("mtr_out_proto5", OBS_OUTPUT_SERVICE, "");
	reg_output(&info, sizeof(info));
	expect_logged(0);
	expect_output("mtr_out_proto5", OBS_OUTPUT_SERVICE);
	info = full_output("mtr_out_proto6", OBS_OUTPUT_SERVICE, ";;");
	reg_output(&info, sizeof(info));
	expect_logged(0);
	assert_int_equal(protocol_count(), n);

	/* A duplicate output id adds nothing either. */
	info = full_output("mtr_out_proto1", OBS_OUTPUT_SERVICE, "MTR_P9");
	reg_output(&info, sizeof(info));
	expect_logged(1, W, "obs_register_output: Output id 'mtr_out_proto1' already exists!  Duplicate library?");
	assert_int_equal(protocol_count(), n);
}

/* --------------------------------------------------------------- encoders */

static void test_encoder_register(void **state)
{
	(void)state;
	REQUIRE_OBS();

	struct obs_encoder_info info = full_encoder("mtr_enc_video", OBS_ENCODER_VIDEO, 0);
	info.get_frame_size = NULL;
	info.encode_texture = NULL;
	info.encode_texture2 = NULL;
	reg_encoder(&info, sizeof(info));
	expect_logged(0);
	expect_freed(false);
	expect_encoder("mtr_enc_video", OBS_ENCODER_VIDEO, 0);

	info = full_encoder("mtr_enc_audio", OBS_ENCODER_AUDIO, OBS_ENCODER_CAP_DYN_BITRATE);
	info.encode_texture = NULL;
	info.encode_texture2 = NULL;
	reg_encoder(&info, sizeof(info));
	expect_logged(0);
	expect_encoder("mtr_enc_audio", OBS_ENCODER_AUDIO, OBS_ENCODER_CAP_DYN_BITRATE);

	/* A texture encoder: either texture callback, no encode. */
	info = full_encoder("mtr_enc_tex1", OBS_ENCODER_VIDEO, CAP_PASS);
	info.encode = NULL;
	info.encode_texture2 = NULL;
	reg_encoder(&info, sizeof(info));
	expect_logged(0);
	expect_encoder("mtr_enc_tex1", OBS_ENCODER_VIDEO, CAP_PASS);

	info = full_encoder("mtr_enc_tex2", OBS_ENCODER_VIDEO, CAP_PASS | CAP_OTHERS);
	info.encode = NULL;
	info.encode_texture = NULL;
	reg_encoder(&info, sizeof(info));
	expect_logged(0);
	expect_encoder("mtr_enc_tex2", OBS_ENCODER_VIDEO, CAP_PASS | CAP_OTHERS);

	/* A scaling encoder that is not a texture one is fine. */
	info = full_encoder("mtr_enc_scale", OBS_ENCODER_VIDEO, CAP_SCALE);
	reg_encoder(&info, sizeof(info));
	expect_logged(0);
	expect_encoder("mtr_enc_scale", OBS_ENCODER_VIDEO, CAP_SCALE);

	/* A type that is neither audio nor video is treated as video
	 * (characterized, not endorsed). */
	info = full_encoder("mtr_enc_type7", (enum obs_encoder_type)7, 0);
	info.get_frame_size = NULL;
	reg_encoder(&info, sizeof(info));
	expect_logged(0);
	expect_encoder("mtr_enc_type7", (enum obs_encoder_type)7, 0);
}

static void test_encoder_required(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = type_count(obs_enum_encoder_types);

#define ENCODER_MISSING(field, type, caps, id, val)                     \
	do {                                                            \
		struct obs_encoder_info i_ = full_encoder(id, type, caps); \
		i_.field = NULL;                                        \
		reg_encoder(&i_, sizeof(i_));                           \
		expect_logged(1, E, ENC_REQUIRED(val, id));             \
		expect_no_encoder(id, types, true);                     \
	} while (false)

	ENCODER_MISSING(get_name, OBS_ENCODER_AUDIO, 0, "mtr_enc_no_get_name", "get_name");
	ENCODER_MISSING(create, OBS_ENCODER_AUDIO, 0, "mtr_enc_no_create", "create");
	ENCODER_MISSING(destroy, OBS_ENCODER_AUDIO, 0, "mtr_enc_no_destroy", "destroy");
	ENCODER_MISSING(encode, OBS_ENCODER_VIDEO, 0, "mtr_enc_no_encode", "encode");
	ENCODER_MISSING(get_frame_size, OBS_ENCODER_AUDIO, 0, "mtr_enc_no_frame_size", "get_frame_size");

	/* A texture encoder with neither texture callback. */
	struct obs_encoder_info info = full_encoder("mtr_enc_no_tex", OBS_ENCODER_VIDEO, CAP_PASS);
	info.encode_texture = NULL;
	info.encode_texture2 = NULL;
	reg_encoder(&info, sizeof(info));
	expect_logged(1, E,
		      "Neither 'encode_texture' nor 'encode_texture2' for 'mtr_enc_no_tex' found.  "
		      "obs_register_encoder failed.");
	expect_no_encoder("mtr_enc_no_tex", types, true);

	/* The encode rule before get_frame_size. */
	info = full_encoder("mtr_enc_no_both", OBS_ENCODER_AUDIO, 0);
	info.encode = NULL;
	info.get_frame_size = NULL;
	reg_encoder(&info, sizeof(info));
	expect_logged(1, E, ENC_REQUIRED("encode", "mtr_enc_no_both"));
	expect_no_encoder("mtr_enc_no_both", types, true);
}

static void test_encoder_texture_scaling_order(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = type_count(obs_enum_encoder_types);

	/* A texture encoder that scales itself: a warning, before the
	 * callbacks and the size. */
	struct obs_encoder_info info = full_encoder("mtr_enc_texscale", OBS_ENCODER_VIDEO, CAP_PASS | CAP_SCALE);
	info.get_name = NULL;
	reg_encoder(&info, sizeof(info) + 8);
	expect_logged(1, W,
		      "obs_register_encoder: Texture encoders cannot self-scale. Encoder id 'mtr_enc_texscale' not "
		      "registered.");
	expect_no_encoder("mtr_enc_texscale", types, true);

	/* The duplicate before that. */
	info = full_encoder("mtr_enc_dupscale", OBS_ENCODER_VIDEO, 0);
	reg_encoder(&info, sizeof(info));
	expect_logged(0);
	types++;
	info.caps = CAP_PASS | CAP_SCALE;
	reg_encoder(&info, sizeof(info));
	expect_logged(1, W, "obs_register_encoder: Encoder id 'mtr_enc_dupscale' already exists!  Duplicate library?");
	assert_int_equal(type_count(obs_enum_encoder_types), types);
	expect_freed(true);
	expect_encoder("mtr_enc_dupscale", OBS_ENCODER_VIDEO, 0);
}

static void test_encoder_size(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = type_count(obs_enum_encoder_types);
	const size_t full = sizeof(struct obs_encoder_info);
	const size_t caps_off = offsetof(struct obs_encoder_info, caps);
	char msg[256];

	struct obs_encoder_info info = full_encoder("mtr_enc_size0", OBS_ENCODER_VIDEO, 0);
	reg_encoder(&info, 0);
	expect_logged(1, E, ENC_REQUIRED("get_name", "mtr_enc_size0"));
	expect_no_encoder("mtr_enc_size0", types, false);

	info = full_encoder("mtr_enc_size_at", OBS_ENCODER_AUDIO, 0);
	reg_encoder(&info, full);
	expect_logged(0);
	expect_encoder("mtr_enc_size_at", OBS_ENCODER_AUDIO, 0);
	types++;

	snprintf(
		msg, sizeof(msg),
		"Tried to register obs_encoder_info with size %llu which is more than libobs currently supports (%llu)",
		(unsigned long long)(full + 1), (unsigned long long)full);
	info = full_encoder("mtr_enc_size_above", OBS_ENCODER_AUDIO, 0);
	reg_encoder(&info, full + 1);
	expect_logged(1, E, msg);
	expect_no_encoder("mtr_enc_size_above", types, true);

	/* Below the end of get_frame_size: an audio encoder lacks it, a video
	 * encoder registers. */
	size_t frame_size_end = offsetof(struct obs_encoder_info, get_frame_size) + sizeof(info.get_frame_size);
	info = full_encoder("mtr_enc_size_fs", OBS_ENCODER_AUDIO, 0);
	reg_encoder(&info, frame_size_end - 1);
	expect_logged(1, E, ENC_REQUIRED("get_frame_size", "mtr_enc_size_fs"));
	/* type_data and free_type_data are past the size: nothing freed. */
	expect_no_encoder("mtr_enc_size_fs", types, false);

	info = full_encoder("mtr_enc_size_fs_video", OBS_ENCODER_VIDEO, 0);
	reg_encoder(&info, frame_size_end - 1);
	expect_logged(0);
	assert_true(type_listed(obs_enum_encoder_types, "mtr_enc_size_fs_video"));
	assert_int_equal(obs_get_encoder_type("mtr_enc_size_fs_video"), OBS_ENCODER_VIDEO);
	/* Its type data was past the size, so get_name gets NULL. */
	assert_string_equal(obs_encoder_get_display_name("mtr_enc_size_fs_video"), "(no type data)");
	types++;

	/* The capabilities are read from the caller's structure whatever the
	 * size (characterized, not endorsed: a size that ends before caps
	 * still has its caps read, past the size). A texture encoder that
	 * scales is refused though its copy would have no caps. */
	info = full_encoder("mtr_enc_caps_cut", OBS_ENCODER_VIDEO, CAP_PASS | CAP_SCALE);
	reg_encoder(&info, caps_off);
	expect_logged(1, W,
		      "obs_register_encoder: Texture encoders cannot self-scale. Encoder id 'mtr_enc_caps_cut' not "
		      "registered.");
	expect_no_encoder("mtr_enc_caps_cut", types, true);

	/* A texture encoder cut before its texture callbacks lacks them, by
	 * the caller's caps. */
	info = full_encoder("mtr_enc_caps_cut2", OBS_ENCODER_VIDEO, CAP_PASS);
	reg_encoder(&info, caps_off);
	expect_logged(1, E,
		      "Neither 'encode_texture' nor 'encode_texture2' for 'mtr_enc_caps_cut2' found.  "
		      "obs_register_encoder failed.");
	expect_no_encoder("mtr_enc_caps_cut2", types, true);

	/* Cut before caps with encode: registered with no caps, though the
	 * caller's caps said otherwise. */
	info = full_encoder("mtr_enc_caps_cut3", OBS_ENCODER_VIDEO, CAP_SCALE | OBS_ENCODER_CAP_DYN_BITRATE);
	reg_encoder(&info, caps_off);
	expect_logged(0);
	expect_encoder("mtr_enc_caps_cut3", OBS_ENCODER_VIDEO, 0);
	types++;

	/* Cut right after encode_texture: the texture encoder registers. */
	size_t tex_end = offsetof(struct obs_encoder_info, encode_texture) + sizeof(info.encode_texture);
	info = full_encoder("mtr_enc_tex_cut", OBS_ENCODER_VIDEO, CAP_PASS);
	info.encode = NULL;
	reg_encoder(&info, tex_end);
	expect_logged(0);
	expect_encoder("mtr_enc_tex_cut", OBS_ENCODER_VIDEO, CAP_PASS);
	types++;
	info = full_encoder("mtr_enc_tex_cut2", OBS_ENCODER_VIDEO, CAP_PASS);
	info.encode_texture = NULL;
	reg_encoder(&info, tex_end);
	expect_logged(1, E,
		      "Neither 'encode_texture' nor 'encode_texture2' for 'mtr_enc_tex_cut2' found.  "
		      "obs_register_encoder failed.");
	expect_no_encoder("mtr_enc_tex_cut2", types, true);
}

/* --------------------------------------------------------------- services */

static void test_service(void **state)
{
	(void)state;
	REQUIRE_OBS();
	size_t types = type_count(obs_enum_service_types);
	const size_t full = sizeof(struct obs_service_info);
	char msg[256];

	struct obs_service_info info = full_service("mtr_svc");
	reg_service(&info, sizeof(info));
	expect_logged(0);
	expect_freed(false);
	expect_service("mtr_svc");
	types++;

#define SERVICE_MISSING(field, id, val)                         \
	do {                                                    \
		struct obs_service_info i_ = full_service(id);  \
		i_.field = NULL;                                \
		reg_service(&i_, sizeof(i_));                   \
		expect_logged(1, E, SVC_REQUIRED(val, id));     \
		expect_no_service(id, types, true);             \
	} while (false)

	SERVICE_MISSING(get_name, "mtr_svc_no_get_name", "get_name");
	SERVICE_MISSING(create, "mtr_svc_no_create", "create");
	SERVICE_MISSING(destroy, "mtr_svc_no_destroy", "destroy");
	SERVICE_MISSING(get_protocol, "mtr_svc_no_get_protocol", "get_protocol");

	/* The duplicate first. */
	info = full_service("mtr_svc");
	info.get_name = NULL;
	reg_service(&info, full + 8);
	expect_logged(1, W, "obs_register_service: Service id 'mtr_svc' already exists!  Duplicate library?");
	assert_int_equal(type_count(obs_enum_service_types), types);
	expect_freed(true);
	expect_service("mtr_svc");

	/* Sizes. */
	info = full_service("mtr_svc_size0");
	reg_service(&info, 0);
	expect_logged(1, E, SVC_REQUIRED("get_name", "mtr_svc_size0"));
	expect_no_service("mtr_svc_size0", types, false);

	snprintf(
		msg, sizeof(msg),
		"Tried to register obs_service_info with size %llu which is more than libobs currently supports (%llu)",
		(unsigned long long)(full + 1), (unsigned long long)full);
	info = full_service("mtr_svc_size_above");
	reg_service(&info, full + 1);
	expect_logged(1, E, msg);
	expect_no_service("mtr_svc_size_above", types, true);

	size_t protocol_end = offsetof(struct obs_service_info, get_protocol) + sizeof(info.get_protocol);
	info = full_service("mtr_svc_size_cut");
	reg_service(&info, protocol_end - 1);
	expect_logged(1, E, SVC_REQUIRED("get_protocol", "mtr_svc_size_cut"));
	expect_no_service("mtr_svc_size_cut", types, true);

	reg_service(&info, protocol_end);
	expect_logged(0);
	expect_service("mtr_svc_size_cut");
}

#define CASE(fn) cmocka_unit_test(fn)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(test_output_flag_combinations),
		CASE(test_output_other_flags),
		CASE(test_encoder_texture_scaling),
		CASE(test_encoder_callbacks),
		CASE(test_service_callbacks),
		CASE(test_protocol_listed),
		CASE(test_output_register),
		CASE(test_output_required),
		CASE(test_output_duplicate),
		CASE(test_output_size),
		CASE(test_output_protocols),
		CASE(test_encoder_register),
		CASE(test_encoder_required),
		CASE(test_encoder_texture_scaling_order),
		CASE(test_encoder_size),
		CASE(test_service),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
