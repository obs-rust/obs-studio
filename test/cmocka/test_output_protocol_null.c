/* The output protocol lookups with a NULL protocol.
 *
 * obs_is_output_protocol_registered(NULL) ran strcmp() on NULL and crashed
 * once any service output had registered a protocol, and
 * obs_enum_output_types_with_protocol(NULL, ...) crashed the same way, as it
 * asks obs_is_output_protocol_registered() first. obs_service_get_protocol()
 * returns NULL for a NULL service, and a service plugin can return NULL from
 * get_protocol, so callers can pass it on. A NULL protocol is not a registered
 * protocol: the check returns false and the enumeration calls nothing.
 * obs_enum_output_protocols() with a NULL out pointer returns false instead
 * of writing through it.
 *
 * Each NULL call runs in a forked child so a crash is reported as a failure
 * instead of aborting the whole test binary. obs_startup() is required to
 * register an output type; it needs a display on Linux (e.g. xvfb-run), and
 * the tests skip when it cannot start.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#ifndef _WIN32
#include <signal.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif
#ifdef __linux__
#include <sys/prctl.h>
#endif

#include <obs.h>
#include <obs-module.h>

static bool obs_up;

static const char *dummy_get_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "Protocol Test Output";
}

static void *dummy_create(obs_data_t *settings, obs_output_t *output)
{
	UNUSED_PARAMETER(settings);
	UNUSED_PARAMETER(output);
	return NULL;
}

static void dummy_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool dummy_start(void *data)
{
	UNUSED_PARAMETER(data);
	return false;
}

static void dummy_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(ts);
}

static void dummy_encoded_packet(void *data, struct encoder_packet *packet)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(packet);
}

static const struct obs_output_info service_output = {
	.id = "test_protocol_null_output",
	.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_SERVICE,
	.protocols = "TESTPROTO",
	.encoded_video_codecs = "h264",
	.encoded_audio_codecs = "aac",
	.get_name = dummy_get_name,
	.create = dummy_create,
	.destroy = dummy_destroy,
	.start = dummy_start,
	.stop = dummy_stop,
	.encoded_packet = dummy_encoded_packet,
};

static bool count_cb(void *data, const char *id)
{
	UNUSED_PARAMETER(id);
	int *count = data;
	(*count)++;
	return true;
}

enum null_call {
	CALL_IS_REGISTERED,
	CALL_ENUM_TYPES,
	CALL_ENUM_PROTOCOLS,
};

/* Returns 0 when the call gave the "none" answer, 1 otherwise. */
static int run_null_call(enum null_call call)
{
	switch (call) {
	case CALL_IS_REGISTERED:
		return obs_is_output_protocol_registered(NULL) ? 1 : 0;
	case CALL_ENUM_TYPES: {
		int count = 0;
		obs_enum_output_types_with_protocol(NULL, &count, count_cb);
		return count != 0;
	}
	case CALL_ENUM_PROTOCOLS:
		return obs_enum_output_protocols(0, NULL) ? 1 : 0;
	}
	return 1;
}

static void expect_null_safe(enum null_call call, const char *name)
{
	if (!obs_up)
		skip();
#ifdef _WIN32
	(void)name;
	assert_int_equal(run_null_call(call), 0);
#else
	fflush(stdout);
	fflush(stderr);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka traps SIGSEGV and would resume the test runner in the
		 * child; let a crash kill the child instead. */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGABRT, SIG_DFL);
#ifdef __linux__
		/* an expected crash should not leave a core dump behind */
		prctl(PR_SET_DUMPABLE, 0);
#endif
		_exit(run_null_call(call));
	}

	int status = 0;
	assert_int_equal(waitpid(pid, &status, 0), pid);
	if (WIFSIGNALED(status)) {
		fprintf(stderr, "%s with NULL crashed with signal %d\n", name, WTERMSIG(status));
		fail();
	}
	if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
		fprintf(stderr, "%s with NULL did not give the none answer\n", name);
		fail();
	}
#endif
}

/* Controls: a registered protocol is still found and enumerated. */
static void registered_protocol_still_found(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_true(obs_is_output_protocol_registered("TESTPROTO"));
	assert_false(obs_is_output_protocol_registered("NOPROTO"));

	int count = 0;
	obs_enum_output_types_with_protocol("TESTPROTO", &count, count_cb);
	assert_int_equal(count, 1);

	char *protocol = NULL;
	bool found = false;
	for (size_t i = 0; obs_enum_output_protocols(i, &protocol); i++) {
		if (strcmp(protocol, "TESTPROTO") == 0)
			found = true;
	}
	assert_true(found);
}

static void is_registered_null(void **state)
{
	UNUSED_PARAMETER(state);
	expect_null_safe(CALL_IS_REGISTERED, "obs_is_output_protocol_registered");
}

static void enum_types_with_protocol_null(void **state)
{
	UNUSED_PARAMETER(state);
	expect_null_safe(CALL_ENUM_TYPES, "obs_enum_output_types_with_protocol");
}

static void enum_protocols_null_out(void **state)
{
	UNUSED_PARAMETER(state);
	expect_null_safe(CALL_ENUM_PROTOCOLS, "obs_enum_output_protocols");
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		obs_register_output(&service_output);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(registered_protocol_still_found),
		cmocka_unit_test(is_registered_null),
		cmocka_unit_test(enum_types_with_protocol_null),
		cmocka_unit_test(enum_protocols_null_out),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
