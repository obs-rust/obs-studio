/* Characterization of obs_enum_outputs(), obs_enum_encoders() and
 * obs_enum_services() before their Rust swap (#5448). The lifetime rules
 * (releasing the current entry, dropping the snapshot references, skipping a
 * dying encoder) are pinned by test_obs_enum_outputs_removal; this pins the
 * rest of what a caller can observe, for each of the three kinds:
 *  - an empty list never calls the callback;
 *  - every entry is visited once, newest first, with the caller's param;
 *  - returning false stops the walk after that entry;
 *  - the callback may enumerate the same kind again (the list lock is
 *    re-entrant for the walking thread) and sees the same entries;
 *  - an object created by the callback is not visited by the walk in progress,
 *    which works on the entries it found when it started. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-encoder.h>
#include <obs-output.h>
#include <obs-service.h>
#include <util/base.h>
#include <util/bmem.h>

#define OBJECT_COUNT 3
#define MAX_VISITS 16

enum kind { KIND_OUTPUT, KIND_ENCODER, KIND_SERVICE };

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)lvl;
	(void)msg;
	(void)args;
	(void)p;
}

static const char *test_get_name(void *type_data)
{
	(void)type_data;
	return "enum contexts test object";
}

static void *test_create_output(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	(void)output;
	return bmalloc(1);
}

static void *test_create_encoder(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	(void)encoder;
	return bmalloc(1);
}

static void *test_create_service(obs_data_t *settings, obs_service_t *service)
{
	(void)settings;
	(void)service;
	return bmalloc(1);
}

static void test_destroy(void *data)
{
	bfree(data);
}

static bool test_output_start(void *data)
{
	(void)data;
	return true;
}

static void test_output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void test_output_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

static bool test_encoder_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet,
				bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received_packet = false;
	return true;
}

/* obs_service_create() refuses a service without a protocol. */
static const char *test_service_get_protocol(void *data)
{
	(void)data;
	return "RTMP";
}

static const struct obs_output_info test_output_info = {
	.id = "enum_contexts_test_output",
	.flags = OBS_OUTPUT_ENCODED,
	.get_name = test_get_name,
	.create = test_create_output,
	.destroy = test_destroy,
	.start = test_output_start,
	.stop = test_output_stop,
	.encoded_packet = test_output_packet,
};

static const struct obs_encoder_info test_encoder_info = {
	.id = "enum_contexts_test_encoder",
	.type = OBS_ENCODER_VIDEO,
	.codec = "h264",
	.get_name = test_get_name,
	.create = test_create_encoder,
	.destroy = test_destroy,
	.encode = test_encoder_encode,
};

static const struct obs_service_info test_service_info = {
	.id = "enum_contexts_test_service",
	.get_name = test_get_name,
	.create = test_create_service,
	.destroy = test_destroy,
	.get_protocol = test_service_get_protocol,
};

struct walk {
	enum kind kind;
	void *visited[MAX_VISITS];
	void *params[MAX_VISITS];
	size_t visits;
	/* Zero-based visit after which the callback returns false; SIZE_MAX
	 * never stops. */
	size_t stop_after;
	/* On the first visit: enumerate the same kind again into nested. */
	struct walk *nested;
	/* On the first visit: create one more object of the kind into created. */
	bool create_in_callback;
	void *created;
};

static void *create_object(enum kind kind, const char *name)
{
	switch (kind) {
	case KIND_OUTPUT:
		return obs_output_create(test_output_info.id, name, NULL, NULL);
	case KIND_ENCODER:
		return obs_video_encoder_create(test_encoder_info.id, name, NULL, NULL);
	case KIND_SERVICE:
		return obs_service_create(test_service_info.id, name, NULL, NULL);
	}
	return NULL;
}

static void release_object(enum kind kind, void *object)
{
	switch (kind) {
	case KIND_OUTPUT:
		obs_output_release(object);
		break;
	case KIND_ENCODER:
		obs_encoder_release(object);
		break;
	case KIND_SERVICE:
		obs_service_release(object);
		break;
	}
}

static void enumerate(struct walk *w);

static bool visit(void *param, void *object)
{
	struct walk *w = param;
	size_t index = w->visits++;

	assert_true(index < MAX_VISITS);
	w->visited[index] = object;
	w->params[index] = param;

	if (index == 0 && w->nested)
		enumerate(w->nested);
	if (index == 0 && w->create_in_callback)
		w->created = create_object(w->kind, "enum contexts created in callback");

	return index != w->stop_after;
}

static bool visit_output(void *param, obs_output_t *output)
{
	return visit(param, output);
}

static bool visit_encoder(void *param, obs_encoder_t *encoder)
{
	return visit(param, encoder);
}

static bool visit_service(void *param, obs_service_t *service)
{
	return visit(param, service);
}

static void enumerate(struct walk *w)
{
	switch (w->kind) {
	case KIND_OUTPUT:
		obs_enum_outputs(visit_output, w);
		break;
	case KIND_ENCODER:
		obs_enum_encoders(visit_encoder, w);
		break;
	case KIND_SERVICE:
		obs_enum_services(visit_service, w);
		break;
	}
}

static void init_walk(struct walk *w, enum kind kind)
{
	memset(w, 0, sizeof(*w));
	w->kind = kind;
	w->stop_after = SIZE_MAX;
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
	obs_register_output(&test_output_info);
	obs_register_encoder(&test_encoder_info);
	obs_register_service(&test_service_info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

static void create_objects(enum kind kind, void *objects[OBJECT_COUNT])
{
	for (size_t i = 0; i < OBJECT_COUNT; i++) {
		char name[48];
		snprintf(name, sizeof(name), "enum contexts object %zu", i);
		objects[i] = create_object(kind, name);
		assert_non_null(objects[i]);
	}
}

static void release_objects(enum kind kind, void *objects[OBJECT_COUNT])
{
	for (size_t i = 0; i < OBJECT_COUNT; i++)
		release_object(kind, objects[i]);
}

/* The newest object comes first: index OBJECT_COUNT - 1 - visit. */
static void assert_newest_first(const struct walk *w, void *objects[OBJECT_COUNT], size_t count)
{
	assert_int_equal(w->visits, count);
	for (size_t i = 0; i < count; i++) {
		assert_ptr_equal(w->visited[i], objects[OBJECT_COUNT - 1 - i]);
		assert_ptr_equal(w->params[i], w);
	}
}

static void check_empty(enum kind kind)
{
	struct walk w;
	init_walk(&w, kind);
	enumerate(&w);
	assert_int_equal(w.visits, 0);
}

static void check_order(enum kind kind)
{
	void *objects[OBJECT_COUNT];
	struct walk w;

	create_objects(kind, objects);
	init_walk(&w, kind);
	enumerate(&w);
	assert_newest_first(&w, objects, OBJECT_COUNT);

	/* A second walk sees the same list: enumeration changes nothing. */
	init_walk(&w, kind);
	enumerate(&w);
	assert_newest_first(&w, objects, OBJECT_COUNT);

	release_objects(kind, objects);
	check_empty(kind);
}

static void check_stop(enum kind kind)
{
	void *objects[OBJECT_COUNT];
	struct walk w;

	create_objects(kind, objects);

	init_walk(&w, kind);
	w.stop_after = 0;
	enumerate(&w);
	assert_newest_first(&w, objects, 1);

	init_walk(&w, kind);
	w.stop_after = 1;
	enumerate(&w);
	assert_newest_first(&w, objects, 2);

	release_objects(kind, objects);
}

static void check_nested(enum kind kind)
{
	void *objects[OBJECT_COUNT];
	struct walk outer, inner;

	create_objects(kind, objects);
	init_walk(&outer, kind);
	init_walk(&inner, kind);
	outer.nested = &inner;
	enumerate(&outer);

	assert_newest_first(&outer, objects, OBJECT_COUNT);
	assert_newest_first(&inner, objects, OBJECT_COUNT);

	release_objects(kind, objects);
}

static void check_created_in_callback(enum kind kind)
{
	void *objects[OBJECT_COUNT];
	struct walk w;

	create_objects(kind, objects);
	init_walk(&w, kind);
	w.create_in_callback = true;
	enumerate(&w);

	assert_non_null(w.created);
	assert_newest_first(&w, objects, OBJECT_COUNT);

	/* The next walk finds it, at the head of the list. */
	struct walk after;
	init_walk(&after, kind);
	enumerate(&after);
	assert_int_equal(after.visits, OBJECT_COUNT + 1);
	assert_ptr_equal(after.visited[0], w.created);

	release_object(kind, w.created);
	release_objects(kind, objects);
}

#define KIND_TESTS(name, kind)                                \
	static void test_##name##_empty(void **state)         \
	{                                                     \
		(void)state;                                  \
		check_empty(kind);                            \
	}                                                     \
	static void test_##name##_order(void **state)         \
	{                                                     \
		(void)state;                                  \
		check_order(kind);                            \
	}                                                     \
	static void test_##name##_stop(void **state)          \
	{                                                     \
		(void)state;                                  \
		check_stop(kind);                             \
	}                                                     \
	static void test_##name##_nested(void **state)        \
	{                                                     \
		(void)state;                                  \
		check_nested(kind);                           \
	}                                                     \
	static void test_##name##_created_in_callback(void **state) \
	{                                                     \
		(void)state;                                  \
		check_created_in_callback(kind);              \
	}

KIND_TESTS(outputs, KIND_OUTPUT)
KIND_TESTS(encoders, KIND_ENCODER)
KIND_TESTS(services, KIND_SERVICE)

#define KIND_CASES(name)                                                                                     \
	cmocka_unit_test_setup_teardown(test_##name##_empty, setup, teardown),                               \
		cmocka_unit_test_setup_teardown(test_##name##_order, setup, teardown),                       \
		cmocka_unit_test_setup_teardown(test_##name##_stop, setup, teardown),                        \
		cmocka_unit_test_setup_teardown(test_##name##_nested, setup, teardown),                      \
		cmocka_unit_test_setup_teardown(test_##name##_created_in_callback, setup, teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		KIND_CASES(outputs),
		KIND_CASES(encoders),
		KIND_CASES(services),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
