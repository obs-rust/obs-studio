/* Regression test for obs-rust/obs-studio#2203: obs_register_source_s() and
 * REGISTER_OBS_DEF() (obs_register_output_s(), obs_register_encoder_s() and
 * obs_register_service_s()) in libobs/obs-module.c.h copied the first `size`
 * bytes of the caller's structure into a zeroed one. A size that ends
 * partway through a pointer field after the required callbacks left that
 * field partly copied in an accepted type, and OBS called it later: an
 * optional callback when it was needed, free_type_data at shutdown.
 *
 * Each type is registered through the public API with its required
 * callbacks, at sizes that end halfway into and one byte short of an
 * optional callback, of type_data and of free_type_data, exactly at the end
 * of free_type_data, and at sizeof. A field must be stored only when it lies
 * wholly within the size, as HAS_VAL() checks it, and NULL otherwise. The
 * stored type is inspected through the public API:
 * - the optional callback is triggered (obs_get_*_properties() for
 *   get_properties, obs_service_get_supported_audio_codecs() for services);
 * - type_data is read back from an instance (obs_*_get_type_data()). It is a
 *   byte pattern the test never dereferences, so a partly copied type_data
 *   is seen without crashing: it compares unequal to both the pattern and
 *   NULL;
 * - obs_shutdown() must call free_type_data only when both type_data and
 *   free_type_data are whole, and then once with the pattern.
 *
 * A service's type_data and free_type_data come before its required
 * get_protocol, so a size that ends inside them is refused; that path is
 * test_register_refused_type_data's.
 *
 * Each case runs in a forked child that starts and shuts down OBS itself, so
 * a jump through a partly copied pointer fails only its own case, and the
 * stage it reached is kept in shared memory. On 64-bit Linux a callback one
 * byte short is usually still whole (its top byte is 0), so the buggy code
 * calls it without crashing; halfway it jumps into unmapped memory. Windows
 * has no fork(), so the cases run in the test process.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. It skips only when no display is set at all; with one set, a failed
 * obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef _WIN32
#include <errno.h>
#include <signal.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif
#ifdef __linux__
#include <sys/prctl.h>
#endif

#include <obs.h>
#include <obs-module.h>

/* Never dereferenced: only read back and passed to free_type_data. */
#define TYPE_DATA ((void *)(uintptr_t)UINT64_C(0xA1B2C3D4E5F60718))

#define PTR sizeof(void *)

enum kind { KIND_SOURCE, KIND_OUTPUT, KIND_ENCODER, KIND_SERVICE };

/* The field the size ends in, or the whole structure. */
enum field { FIELD_OPTIONAL, FIELD_TYPE_DATA, FIELD_FREE, FIELD_SIZEOF };

enum cut { CUT_HALF, CUT_SHORT, CUT_END };

struct reg_case {
	enum kind kind;
	enum field field;
	enum cut cut;
	const char *id;
};

enum stage { STAGE_NONE, STAGE_STARTED, STAGE_REGISTERED, STAGE_OPTIONAL, STAGE_INSTANCE, STAGE_SHUT_DOWN };

struct result {
	enum stage stage;
	bool started;
	bool registered;
	bool created;
	int optional_calls;
	void *type_data;
	int frees;
	void *freed;
};

/* Room after the structure, so a size never reads past the caller's
 * buffer. */
union info {
	struct obs_source_info source;
	struct obs_output_info output;
	struct obs_encoder_info encoder;
	struct obs_service_info service;
	unsigned char bytes[2048];
};

static struct result *res;
static int dummy_data;

static const char *get_name(void *type_data)
{
	(void)type_data;
	return "register partial callbacks";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return &dummy_data;
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return &dummy_data;
}

static void *encoder_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return &dummy_data;
}

static void *service_create(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	(void)service;
	return &dummy_data;
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

static void encoded_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static bool encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static const char *get_protocol(void *data)
{
	(void)data;
	return "RTMP";
}

/* The optional callbacks the test triggers. */
static obs_properties_t *get_properties(void *data)
{
	(void)data;
	res->optional_calls++;
	return obs_properties_create();
}

static const char **get_supported_audio_codecs(void *data)
{
	(void)data;
	res->optional_calls++;
	return NULL;
}

static void free_type_data(void *type_data)
{
	res->frees++;
	res->freed = type_data;
}

struct layout {
	size_t optional;
	size_t type_data;
	size_t free_fn;
	size_t size;
};

#define LAYOUT(type, opt)                                         \
	(struct layout)                                           \
	{                                                         \
		offsetof(type, opt), offsetof(type, type_data),   \
			offsetof(type, free_type_data), sizeof(type) \
	}

static struct layout layout(enum kind kind)
{
	switch (kind) {
	case KIND_SOURCE:
		return LAYOUT(struct obs_source_info, get_properties);
	case KIND_OUTPUT:
		return LAYOUT(struct obs_output_info, get_properties);
	case KIND_ENCODER:
		return LAYOUT(struct obs_encoder_info, get_properties);
	case KIND_SERVICE:
		return LAYOUT(struct obs_service_info, get_supported_audio_codecs);
	}
	fail();
	return (struct layout){0};
}

static size_t size_of_case(const struct reg_case *c)
{
	struct layout l = layout(c->kind);
	size_t off = 0;
	switch (c->field) {
	case FIELD_OPTIONAL:
		off = l.optional;
		break;
	case FIELD_TYPE_DATA:
		off = l.type_data;
		break;
	case FIELD_FREE:
		off = l.free_fn;
		break;
	case FIELD_SIZEOF:
		return l.size;
	}
	switch (c->cut) {
	case CUT_HALF:
		return off + PTR / 2;
	case CUT_SHORT:
		return off + PTR - 1;
	case CUT_END:
		return off + PTR;
	}
	fail();
	return 0;
}

static bool whole(size_t off, size_t size)
{
	return off + PTR <= size;
}

/* The type with its required callbacks, an optional callback, type_data
 * and free_type_data, registered with `size`; the optional callback and an
 * instance's type_data are then looked at and OBS is shut down. */
static void reg(const struct reg_case *c, size_t size)
{
	union info info;
	memset(&info, 0, sizeof(info));

	res->started = obs_startup("en-US", NULL, NULL);
	if (!res->started)
		return;
	res->stage = STAGE_STARTED;

	obs_source_t *source = NULL;
	obs_output_t *output = NULL;
	obs_encoder_t *encoder = NULL;
	obs_service_t *service = NULL;
	obs_properties_t *props = NULL;

	switch (c->kind) {
	case KIND_SOURCE:
		info.source.id = c->id;
		info.source.type = OBS_SOURCE_TYPE_INPUT;
		info.source.get_name = get_name;
		info.source.create = source_create;
		info.source.destroy = destroy;
		info.source.get_properties = get_properties;
		info.source.type_data = TYPE_DATA;
		info.source.free_type_data = free_type_data;
		obs_register_source_s(&info.source, size);
		res->registered = obs_source_get_display_name(c->id) != NULL;
		break;
	case KIND_OUTPUT:
		info.output.id = c->id;
		/* An encoded output: obs_output_create() asks a raw one for
		 * the video, which needs obs_reset_video(). */
		info.output.flags = OBS_OUTPUT_ENCODED;
		info.output.get_name = get_name;
		info.output.create = output_create;
		info.output.destroy = destroy;
		info.output.start = output_start;
		info.output.stop = output_stop;
		info.output.encoded_packet = encoded_packet;
		info.output.get_properties = get_properties;
		info.output.type_data = TYPE_DATA;
		info.output.free_type_data = free_type_data;
		obs_register_output_s(&info.output, size);
		res->registered = obs_output_get_display_name(c->id) != NULL;
		break;
	case KIND_ENCODER:
		info.encoder.id = c->id;
		info.encoder.type = OBS_ENCODER_VIDEO;
		info.encoder.codec = "h264";
		info.encoder.get_name = get_name;
		info.encoder.create = encoder_create;
		info.encoder.destroy = destroy;
		info.encoder.encode = encode;
		info.encoder.get_properties = get_properties;
		info.encoder.type_data = TYPE_DATA;
		info.encoder.free_type_data = free_type_data;
		obs_register_encoder_s(&info.encoder, size);
		res->registered = obs_encoder_get_display_name(c->id) != NULL;
		break;
	case KIND_SERVICE:
		info.service.id = c->id;
		info.service.get_name = get_name;
		info.service.create = service_create;
		info.service.destroy = destroy;
		info.service.get_protocol = get_protocol;
		info.service.get_supported_audio_codecs = get_supported_audio_codecs;
		info.service.type_data = TYPE_DATA;
		info.service.free_type_data = free_type_data;
		obs_register_service_s(&info.service, size);
		res->registered = obs_service_get_display_name(c->id) != NULL;
		break;
	}
	if (res->registered)
		res->stage = STAGE_REGISTERED;

	/* The optional callback. */
	switch (c->kind) {
	case KIND_SOURCE:
		props = obs_get_source_properties(c->id);
		break;
	case KIND_OUTPUT:
		props = obs_get_output_properties(c->id);
		break;
	case KIND_ENCODER:
		props = obs_get_encoder_properties(c->id);
		break;
	case KIND_SERVICE:
		service = obs_service_create(c->id, "register partial callbacks", NULL, NULL);
		if (service)
			obs_service_get_supported_audio_codecs(service);
		break;
	}
	obs_properties_destroy(props);
	res->stage = STAGE_OPTIONAL;

	/* The instance's type_data. */
	switch (c->kind) {
	case KIND_SOURCE:
		source = obs_source_create(c->id, "register partial callbacks", NULL, NULL);
		res->created = source != NULL;
		res->type_data = obs_source_get_type_data(source);
		obs_source_release(source);
		break;
	case KIND_OUTPUT:
		output = obs_output_create(c->id, "register partial callbacks", NULL, NULL);
		res->created = output != NULL;
		res->type_data = obs_output_get_type_data(output);
		obs_output_release(output);
		break;
	case KIND_ENCODER:
		encoder = obs_video_encoder_create(c->id, "register partial callbacks", NULL, NULL);
		res->created = encoder != NULL;
		res->type_data = obs_encoder_get_type_data(encoder);
		obs_encoder_release(encoder);
		break;
	case KIND_SERVICE:
		res->created = service != NULL;
		res->type_data = obs_service_get_type_data(service);
		obs_service_release(service);
		break;
	}
	res->stage = STAGE_INSTANCE;

	obs_shutdown();
	res->stage = STAGE_SHUT_DOWN;
}

#ifndef _WIN32
static const char *stage_name(enum stage stage)
{
	switch (stage) {
	case STAGE_NONE:
		return "obs_startup";
	case STAGE_STARTED:
		return "the registration";
	case STAGE_REGISTERED:
		return "the optional callback";
	case STAGE_OPTIONAL:
		return "the instance";
	case STAGE_INSTANCE:
		return "obs_shutdown";
	case STAGE_SHUT_DOWN:
		return "the report";
	}
	return "?";
}
#endif

static void run_case(void **state)
{
	const struct reg_case *c = *state;
	size_t size = size_of_case(c);
	struct layout l = layout(c->kind);

#ifdef _WIN32
	struct result local;
	memset(&local, 0, sizeof(local));
	res = &local;
	reg(c, size);
#else
	res = mmap(NULL, sizeof(*res), PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANONYMOUS, -1, 0);
	assert_true(res != MAP_FAILED);
	memset(res, 0, sizeof(*res));
	fflush(stdout);
	fflush(stderr);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka traps SIGSEGV and would resume the test runner in the
		 * child; let a crash kill the child instead. */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGILL, SIG_DFL);
		signal(SIGABRT, SIG_DFL);
		signal(SIGALRM, SIG_DFL);
#ifdef __linux__
		/* an expected crash should not leave a core dump behind */
		prctl(PR_SET_DUMPABLE, 0);
#endif
		/* A jump into code that loops ends here. */
		alarm(60);
		reg(c, size);
		_exit(0);
	}

	int status = 0;
	while (waitpid(pid, &status, 0) < 0)
		assert_int_equal(errno, EINTR);
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "%s with size %zu: died with signal %d in %s\n", c->id, size, WTERMSIG(status),
			stage_name(res->stage));
		munmap(res, sizeof(*res));
		res = NULL;
		fail();
	}
	assert_true(WIFEXITED(status));
	assert_int_equal(WEXITSTATUS(status), 0);
#endif

	struct result r = *res;
#ifndef _WIN32
	munmap(res, sizeof(*res));
#endif
	res = NULL;

	if (!r.started) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			fail();
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}

	int optional_calls = whole(l.optional, size) ? 1 : 0;
	void *type_data = whole(l.type_data, size) ? TYPE_DATA : NULL;
	int frees = whole(l.type_data, size) && whole(l.free_fn, size) ? 1 : 0;

	if (r.optional_calls != optional_calls || r.type_data != type_data || r.frees != frees ||
	    (r.frees && r.freed != TYPE_DATA))
		fprintf(stderr,
			"%s with size %zu: optional callback called %d times (want %d), type_data %p (want %p), "
			"free_type_data called %d times (want %d), last with %p\n",
			c->id, size, r.optional_calls, optional_calls, r.type_data, type_data, r.frees, frees, r.freed);
	assert_true(r.registered);
	assert_int_equal(r.stage, STAGE_SHUT_DOWN);
	assert_true(r.created);
	assert_int_equal(r.optional_calls, optional_calls);
	assert_ptr_equal(r.type_data, type_data);
	assert_int_equal(r.frees, frees);
	if (frees)
		assert_ptr_equal(r.freed, TYPE_DATA);
}

#define CUTS(kind, field, name)                               \
	{kind, field, CUT_HALF, "rpc_" name "_half"}, {kind, field, CUT_SHORT, "rpc_" name "_short"}

static struct reg_case cases[] = {
	CUTS(KIND_SOURCE, FIELD_OPTIONAL, "source_in_get_properties"),
	CUTS(KIND_SOURCE, FIELD_TYPE_DATA, "source_in_type_data"),
	CUTS(KIND_SOURCE, FIELD_FREE, "source_in_free"),
	{KIND_SOURCE, FIELD_FREE, CUT_END, "rpc_source_at_free_end"},
	{KIND_SOURCE, FIELD_SIZEOF, CUT_END, "rpc_source_sizeof"},
	CUTS(KIND_OUTPUT, FIELD_OPTIONAL, "output_in_get_properties"),
	CUTS(KIND_OUTPUT, FIELD_TYPE_DATA, "output_in_type_data"),
	CUTS(KIND_OUTPUT, FIELD_FREE, "output_in_free"),
	{KIND_OUTPUT, FIELD_FREE, CUT_END, "rpc_output_at_free_end"},
	{KIND_OUTPUT, FIELD_SIZEOF, CUT_END, "rpc_output_sizeof"},
	CUTS(KIND_ENCODER, FIELD_OPTIONAL, "encoder_in_get_properties"),
	CUTS(KIND_ENCODER, FIELD_TYPE_DATA, "encoder_in_type_data"),
	CUTS(KIND_ENCODER, FIELD_FREE, "encoder_in_free"),
	{KIND_ENCODER, FIELD_FREE, CUT_END, "rpc_encoder_at_free_end"},
	{KIND_ENCODER, FIELD_SIZEOF, CUT_END, "rpc_encoder_sizeof"},
	CUTS(KIND_SERVICE, FIELD_OPTIONAL, "service_in_audio_codecs"),
	{KIND_SERVICE, FIELD_OPTIONAL, CUT_END, "rpc_service_at_audio_codecs_end"},
	{KIND_SERVICE, FIELD_SIZEOF, CUT_END, "rpc_service_sizeof"},
};

int main(void)
{
	struct CMUnitTest tests[sizeof(cases) / sizeof(cases[0])];
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		struct CMUnitTest t = cmocka_unit_test_prestate(run_case, &cases[i]);
		t.name = cases[i].id;
		tests[i] = t;
	}
	return cmocka_run_group_tests_name("test_register_partial_callbacks", tests, NULL, NULL);
}
