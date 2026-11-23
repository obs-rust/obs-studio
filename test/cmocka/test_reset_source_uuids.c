/* C-only characterization before extracting obs_reset_source_uuids
 * (Refs #5498).
 *
 * obs_reset_source_uuids gives every source in the core's uuid table a new
 * uuid:
 *
 * - every source changes uuid: inputs, scenes and private sources alike;
 *   each new uuid is a well formed UUID string and no two sources share
 *   one;
 * - a lookup by the new uuid finds the same source, a lookup by an old uuid
 *   finds nothing;
 * - names, the by-name lookup and the set of sources are unchanged, and
 *   obs_enum_all_sources still visits them in creation order;
 * - with no source it does nothing, and a second reset changes every uuid
 *   again.
 *
 * Every case runs obs_startup() without obs_reset_video(); no display is
 * needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <ctype.h>
#include <stdbool.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>

#define INPUT_ID "test_reset_source_uuids_input"
#define MAX_SEEN 16

/* Created in this order by populate(). */
enum {
	IN_A,
	PRIV_IN,
	SCENE,
	IN_B,
	PRIV_SCENE,
	N_SOURCES,
};

static const char *const names[N_SOURCES] = {"input a", "private input", "scene", "input b", "private scene"};

struct fixture {
	obs_source_t *sources[N_SOURCES];
	obs_scene_t *scenes[2];
	char uuids[N_SOURCES][UUID_STR_LENGTH + 1];
};

struct visit {
	obs_source_t *seen[MAX_SEEN];
	size_t count;
};

static bool record_source(void *param, obs_source_t *source)
{
	struct visit *v = param;
	assert_true(v->count < MAX_SEEN);
	v->seen[v->count++] = source;
	return true;
}

static const char *input_name(void *data)
{
	(void)data;
	return "reset source uuids input";
}

static void *input_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void input_destroy(void *data)
{
	(void)data;
}

static int startup(void **state)
{
	if (!obs_startup("en-US", NULL, NULL))
		return -1;

	struct obs_source_info input = {
		.id = INPUT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = input_name,
		.create = input_create,
		.destroy = input_destroy,
	};
	obs_register_source(&input);

	*state = NULL;
	return 0;
}

static int shutdown_core(void **state)
{
	struct fixture *f = *state;
	if (f) {
		for (size_t i = 0; i < N_SOURCES; i++)
			obs_source_release(f->sources[i]);
		bfree(f);
	}
	obs_shutdown();
	return 0;
}

static void save_uuids(struct fixture *f)
{
	for (size_t i = 0; i < N_SOURCES; i++) {
		const char *uuid = obs_source_get_uuid(f->sources[i]);
		assert_non_null(uuid);
		assert_int_equal(strlen(uuid), UUID_STR_LENGTH);
		memcpy(f->uuids[i], uuid, UUID_STR_LENGTH + 1);
	}
}

static struct fixture *populate(void **state)
{
	struct fixture *f = bzalloc(sizeof(*f));
	*state = f;

	f->sources[IN_A] = obs_source_create(INPUT_ID, names[IN_A], NULL, NULL);
	f->sources[PRIV_IN] = obs_source_create_private(INPUT_ID, names[PRIV_IN], NULL);
	f->scenes[0] = obs_scene_create(names[SCENE]);
	f->sources[SCENE] = obs_source_get_ref(obs_scene_get_source(f->scenes[0]));
	obs_scene_release(f->scenes[0]);
	f->sources[IN_B] = obs_source_create(INPUT_ID, names[IN_B], NULL, NULL);
	f->scenes[1] = obs_scene_create_private(names[PRIV_SCENE]);
	f->sources[PRIV_SCENE] = obs_source_get_ref(obs_scene_get_source(f->scenes[1]));
	obs_scene_release(f->scenes[1]);

	for (size_t i = 0; i < N_SOURCES; i++)
		assert_non_null(f->sources[i]);

	save_uuids(f);
	return f;
}

static bool well_formed_uuid(const char *uuid)
{
	if (strlen(uuid) != UUID_STR_LENGTH)
		return false;
	for (size_t i = 0; i < UUID_STR_LENGTH; i++) {
		if (i == 8 || i == 13 || i == 18 || i == 23) {
			if (uuid[i] != '-')
				return false;
		} else if (!isxdigit((unsigned char)uuid[i])) {
			return false;
		}
	}
	return true;
}

static void assert_all_new_and_unique(const struct fixture *f)
{
	for (size_t i = 0; i < N_SOURCES; i++) {
		const char *uuid = obs_source_get_uuid(f->sources[i]);
		assert_non_null(uuid);
		assert_true(well_formed_uuid(uuid));
		assert_string_not_equal(uuid, f->uuids[i]);
		for (size_t j = 0; j < N_SOURCES; j++) {
			assert_string_not_equal(uuid, f->uuids[j]);
			if (j != i)
				assert_string_not_equal(uuid, obs_source_get_uuid(f->sources[j]));
		}
	}
}

static void every_source_gets_a_new_unique_uuid(void **state)
{
	struct fixture *f = populate(state);

	obs_reset_source_uuids();

	assert_all_new_and_unique(f);
}

static void lookup_by_new_uuid_finds_the_source_old_finds_nothing(void **state)
{
	struct fixture *f = populate(state);

	obs_reset_source_uuids();

	for (size_t i = 0; i < N_SOURCES; i++) {
		obs_source_t *found = obs_get_source_by_uuid(obs_source_get_uuid(f->sources[i]));
		assert_ptr_equal(found, f->sources[i]);
		obs_source_release(found);

		assert_null(obs_get_source_by_uuid(f->uuids[i]));
	}
}

static void names_and_order_are_kept(void **state)
{
	struct visit before = {0};
	obs_enum_all_sources(record_source, &before);
	assert_int_equal(before.count, 0);

	struct fixture *f = populate(state);
	obs_enum_all_sources(record_source, &before);
	assert_int_equal(before.count, N_SOURCES);
	for (size_t i = 0; i < N_SOURCES; i++)
		assert_ptr_equal(before.seen[i], f->sources[i]);

	obs_reset_source_uuids();

	struct visit after = {0};
	obs_enum_all_sources(record_source, &after);
	assert_int_equal(after.count, N_SOURCES);
	for (size_t i = 0; i < N_SOURCES; i++) {
		assert_ptr_equal(after.seen[i], f->sources[i]);
		assert_string_equal(obs_source_get_name(f->sources[i]), names[i]);
	}

	/* The public sources are still found by name. */
	const size_t public_ids[] = {IN_A, SCENE, IN_B};
	for (size_t i = 0; i < sizeof(public_ids) / sizeof(public_ids[0]); i++) {
		obs_source_t *found = obs_get_source_by_name(names[public_ids[i]]);
		assert_ptr_equal(found, f->sources[public_ids[i]]);
		obs_source_release(found);
	}
}

static void no_sources_is_a_no_op(void **state)
{
	(void)state;
	obs_reset_source_uuids();

	struct visit v = {0};
	obs_enum_all_sources(record_source, &v);
	assert_int_equal(v.count, 0);
}

static void second_reset_changes_uuids_again(void **state)
{
	struct fixture *f = populate(state);

	obs_reset_source_uuids();
	save_uuids(f);
	obs_reset_source_uuids();

	assert_all_new_and_unique(f);
	for (size_t i = 0; i < N_SOURCES; i++) {
		obs_source_t *found = obs_get_source_by_uuid(obs_source_get_uuid(f->sources[i]));
		assert_ptr_equal(found, f->sources[i]);
		obs_source_release(found);
		assert_null(obs_get_source_by_uuid(f->uuids[i]));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(every_source_gets_a_new_unique_uuid, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(lookup_by_new_uuid_finds_the_source_old_finds_nothing, startup,
						shutdown_core),
		cmocka_unit_test_setup_teardown(names_and_order_are_kept, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(no_sources_is_a_no_op, startup, shutdown_core),
		cmocka_unit_test_setup_teardown(second_reset_changes_uuids_again, startup, shutdown_core),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
