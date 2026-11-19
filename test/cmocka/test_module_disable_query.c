/* C characterization for #5166. Record the real wrapper's dependency calls,
 * then check the public API against this build's generated core-module list.
 * The Rust swap must relink the unchanged dispatch fixture to the replacement. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdlib.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"
#include "obs-core-modules.h"

static const char *expected_name;
static bool membership;
static bool permission;
static unsigned calls;

static bool recorded_membership(const char *name)
{
	assert_int_equal(calls++, 0);
	assert_ptr_equal(name, expected_name);
	return membership;
}

static bool recorded_policy(bool is_core)
{
	assert_int_equal(calls++, 1);
	assert_int_equal(is_core, membership);
	return permission;
}

#define is_core_module recorded_membership
#define obs_module_load_rules_allow_disable recorded_policy
#define obs_get_module_allow_disable characterized_allow_disable
#include "libobs/obs-module-disable-query.c.h"
#undef obs_get_module_allow_disable
#undef obs_module_load_rules_allow_disable
#undef is_core_module

static void membership_dispatch(void **state)
{
	(void)state;
	const char name[] = "dispatch-token";
	expected_name = name;
	/* Vary the policy result independently: the wrapper must forward it,
	 * rather than repeat the existing Rust policy or normalize the name. */
	for (unsigned i = 0; i < 4; i++) {
		membership = (i & 1) != 0;
		permission = (i & 2) != 0;
		calls = 0;
		assert_int_equal(characterized_allow_disable(name), permission);
		assert_int_equal(calls, 2);
	}
}

static void generated_core_names(void **state)
{
	(void)state;
	/* A zero-length core list is valid. Do not invent a platform core name.
	 * NULL names are outside the matcher's valid string domain. */
	for (unsigned i = 0; i < obs_core_modules_count; i++) {
		const char *name = obs_core_modules[i];
		assert_non_null(name);
		long allocs = bnum_allocs();
		assert_false(obs_get_module_allow_disable(name));
		assert_int_equal(bnum_allocs(), allocs);

		size_t length = strlen(name);
		char *changed = malloc(length + 1);
		assert_non_null(changed);
		memcpy(changed, name, length + 1);
		/* Toggle ASCII case without depending on the process locale. */
		for (size_t j = 0; j < length; j++) {
			if (changed[j] >= 'a' && changed[j] <= 'z')
				changed[j] = (char)(changed[j] - 'a' + 'A');
			else if (changed[j] >= 'A' && changed[j] <= 'Z')
				changed[j] = (char)(changed[j] - 'A' + 'a');
		}
		/* A differently cased entry could itself be in a future build list. */
		bool listed = false;
		for (unsigned j = 0; j < obs_core_modules_count; j++)
			listed |= strcmp(changed, obs_core_modules[j]) == 0;
		assert_int_equal(obs_get_module_allow_disable(changed), !listed);
		free(changed);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(membership_dispatch),
					   cmocka_unit_test(generated_core_names)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
