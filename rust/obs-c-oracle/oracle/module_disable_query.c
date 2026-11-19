/* Original C module disable-permission query (#5183) and its retained
 * membership helper, as the C reference builds them. Both the original and
 * the Rust export call the membership boundary under its real name,
 * obs_in_core_module_list(), defined here for the test binaries that link
 * the obs-core shims without libobs: it records each call and answers by
 * the generated matcher's contract (case-sensitive strcmp() over a fixed
 * list) or with a forced answer. Its default answers by the list, so a test
 * binary that only links it still gets a real matcher. The policy is the
 * existing Rust obs_module_load_rules_allow_disable() for both. The real
 * generated list is covered by the unchanged test_module_disable_query
 * against both actual libraries. NULL names are outside the matcher's
 * domain and excluded. */
#include "libobs/build/libobs.pch.h"
#include <string.h>

#if defined(_MSC_VER)
#define MODULE_DISABLE_QUERY_TLS __declspec(thread)
#else
#define MODULE_DISABLE_QUERY_TLS _Thread_local
#endif

static const char *const module_disable_query_list[] = {"obs-ffmpeg", "image-source", "obs-x264",
							"Mixed-Case", "mIXED-cASE",   "rtmp-services"};

/* 0: answer by the list; 1: forced non-member; 2: forced member. */
static MODULE_DISABLE_QUERY_TLS int module_disable_query_mode;
static MODULE_DISABLE_QUERY_TLS unsigned module_disable_query_calls;
static MODULE_DISABLE_QUERY_TLS const char *module_disable_query_name;

bool obs_in_core_module_list(const char *name)
{
	module_disable_query_calls++;
	module_disable_query_name = name;
	if (module_disable_query_mode)
		return module_disable_query_mode == 2;
	for (size_t i = 0; i < sizeof(module_disable_query_list) / sizeof(module_disable_query_list[0]); i++) {
		if (strcmp(module_disable_query_list[i], name) == 0)
			return true;
	}
	return false;
}

bool oracle_obs_get_module_allow_disable(const char *name);
#define obs_get_module_allow_disable oracle_obs_get_module_allow_disable
#include "libobs/obs-module-disable-query-support.c.h"
#include "libobs/obs-module-disable-query.c.h"
#undef obs_get_module_allow_disable

typedef bool (*module_disable_query_fn)(const char *name);

/* Runs the original C, then the port, on the same borrowed name. Writes
 * the original result to out[0] and the membership answer to out[1]; true
 * when each made exactly one membership call with the caller's pointer,
 * the name bytes are unchanged and the port's result equals the original. */
bool oracle_module_disable_query_check(int mode, const char *name, module_disable_query_fn port, bool *out);
bool oracle_module_disable_query_check(int mode, const char *name, module_disable_query_fn port, bool *out)
{
	size_t size = strlen(name) + 1;
	char *before = malloc(size);
	if (!before)
		return false;
	memcpy(before, name, size);
	module_disable_query_mode = mode;
	module_disable_query_calls = 0;
	module_disable_query_name = NULL;
	out[0] = oracle_obs_get_module_allow_disable(name);
	bool valid = module_disable_query_calls == 1 && module_disable_query_name == name;
	module_disable_query_calls = 0;
	module_disable_query_name = NULL;
	out[1] = obs_in_core_module_list(name);
	module_disable_query_calls = 0;
	module_disable_query_name = NULL;
	bool result = port(name);
	valid = valid && module_disable_query_calls == 1 && module_disable_query_name == name && result == out[0] &&
		memcmp(before, name, size) == 0;
	module_disable_query_mode = 0;
	module_disable_query_calls = 0;
	module_disable_query_name = NULL;
	free(before);
	return valid;
}
