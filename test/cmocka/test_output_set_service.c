/* Direct actual-library characterization of obs_output_set_service.
 * Private-layout fixtures need no core startup, allocation, or destruction:
 * this setter only checks flags/active state and writes borrowed links.
 * In particular, replacing an output's service leaves the old service's
 * reverse link intact: characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

struct capture {
	unsigned count;
	int level;
	char text[256];
};

static struct capture logs;
static log_handler_t previous_handler;
static void *previous_param;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct capture *capture = param;
	capture->count++;
	capture->level = level;
	vsnprintf(capture->text, sizeof(capture->text), format, args);
}

static int setup(void **state)
{
	(void)state;
	memset(&logs, 0, sizeof(logs));
	base_get_log_handler(&previous_handler, &previous_param);
	base_set_log_handler(capture_log, &logs);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	base_set_log_handler(previous_handler, previous_param);
	return 0;
}

struct links {
	struct obs_output outputs[2];
	struct obs_service services[2];
};

static void initialize(struct links *links)
{
	memset(links, 0, sizeof(*links));
	links->outputs[0].context.name = "target";
	links->outputs[0].info.flags = OBS_OUTPUT_SERVICE;
	links->outputs[1].context.name = "old output";
	links->outputs[1].info.flags = OBS_OUTPUT_SERVICE;
}

static void null_output(void **state)
{
	(void)state;
	struct obs_service service = {0}, before;
	memcpy(&before, &service, sizeof(before));
	obs_output_set_service(NULL, &service);
	assert_memory_equal(&service, &before, sizeof(service));
	assert_int_equal(logs.count, 1);
	assert_int_equal(logs.level, LOG_DEBUG);
	assert_string_equal(logs.text, "obs_output_set_service: Null 'output' parameter");
	obs_output_set_service(NULL, NULL);
	assert_int_equal(logs.count, 2);
}

static void refusal_gates(void **state)
{
	(void)state;
	/* Every combination pins the flag warning before later refusals. A
	 * NULL service is a refusal, never a request to clear an existing link. */
	for (unsigned flags = 0; flags < 2; flags++) {
		for (unsigned output_active = 0; output_active < 2; output_active++) {
			for (unsigned service_active = 0; service_active < 2; service_active++) {
				for (unsigned missing = 0; missing < 2; missing++) {
					if (flags && !output_active && !service_active && !missing)
						continue;
					struct links links, before;
					initialize(&links);
					links.outputs[0].info.flags = flags ? OBS_OUTPUT_SERVICE : OBS_OUTPUT_VIDEO;
					links.outputs[0].active = output_active != 0;
					links.outputs[0].service = &links.services[0];
					links.services[0].output = &links.outputs[0];
					links.outputs[1].service = &links.services[1];
					links.services[1].output = &links.outputs[1];
					links.services[1].active = service_active != 0;
					memcpy(&before, &links, sizeof(before));
					memset(&logs, 0, sizeof(logs));
					obs_output_set_service(&links.outputs[0], missing ? NULL : &links.services[1]);
					assert_memory_equal(&links, &before, sizeof(links));
					assert_int_equal(logs.count, flags ? 0 : 1);
					if (!flags) {
						assert_int_equal(logs.level, LOG_WARNING);
						assert_string_equal(
							logs.text,
							"Output 'target': Tried to use obs_output_set_service on a non-service output");
					}
				}
			}
		}
	}
}

static void assign_and_repeat(void **state)
{
	(void)state;
	struct links links, expected;
	initialize(&links);
	/* Unrelated output flags do not prevent service attachment. */
	links.outputs[0].info.flags |= OBS_OUTPUT_VIDEO | OBS_OUTPUT_ENCODED;
	memcpy(&expected, &links, sizeof(expected));
	expected.outputs[0].service = &links.services[0];
	expected.services[0].output = &links.outputs[0];
	obs_output_set_service(&links.outputs[0], &links.services[0]);
	assert_memory_equal(&links, &expected, sizeof(links));
	obs_output_set_service(&links.outputs[0], &links.services[0]);
	assert_memory_equal(&links, &expected, sizeof(links));
	assert_int_equal(logs.count, 0);
}

static void transfer_detaches_old_output(void **state)
{
	(void)state;
	struct links links, expected;
	initialize(&links);
	/* The old output's active state and service flag are not admission gates. */
	links.outputs[1].active = true;
	links.outputs[1].info.flags = 0;
	links.outputs[1].service = &links.services[0];
	links.services[0].output = &links.outputs[1];
	memcpy(&expected, &links, sizeof(expected));
	expected.outputs[1].service = NULL;
	expected.outputs[0].service = &links.services[0];
	expected.services[0].output = &links.outputs[0];
	obs_output_set_service(&links.outputs[0], &links.services[0]);
	assert_memory_equal(&links, &expected, sizeof(links));
	assert_int_equal(logs.count, 0);
}

static void replacement_keeps_former_reverse_link(void **state)
{
	(void)state;
	struct links links, expected;
	initialize(&links);
	links.outputs[0].service = &links.services[0];
	links.services[0].output = &links.outputs[0];
	links.outputs[1].service = &links.services[1];
	links.services[1].output = &links.outputs[1];
	memcpy(&expected, &links, sizeof(expected));
	expected.outputs[1].service = NULL;
	expected.outputs[0].service = &links.services[1];
	expected.services[1].output = &links.outputs[0];
	/* No refcount, ownership, or former-service cleanup is added here. */
	obs_output_set_service(&links.outputs[0], &links.services[1]);
	assert_memory_equal(&links, &expected, sizeof(links));
	assert_ptr_equal(links.services[0].output, &links.outputs[0]);
	assert_int_equal(logs.count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(null_output, setup, teardown),
		cmocka_unit_test_setup_teardown(refusal_gates, setup, teardown),
		cmocka_unit_test_setup_teardown(assign_and_repeat, setup, teardown),
		cmocka_unit_test_setup_teardown(transfer_detaches_old_output, setup, teardown),
		cmocka_unit_test_setup_teardown(replacement_keeps_former_reverse_link, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
