/* Direct characterization of the hidden C loader; unchanged across the swap.
 * Public caller coverage remains in test_core_module_load. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#define obs_module_load_metadata characterized_module_load_metadata
#include "libobs/obs-module-load-metadata.c.h"
#undef obs_module_load_metadata

static const char *fixture;
static void check_manifest(void **state)
{
	(void)state;
	struct dstr path = {0};
	dstr_copy(&path, fixture);
	dstr_cat(&path, "/manifest.json");
	const char *documents[] = {
		"{\"display_name\":\"Shown\",\"id\":\"id\",\"version\":\"1.2\",\"os_arch\":\"arch\","
		"\"name\":\"name\",\"description\":\"short\",\"long_description\":\"long\","
		"\"urls\":{\"repository\":\"repo\",\"website\":\"web\",\"support\":\"help\"},"
		"\"has_banner\":true,\"has_icon\":true}",
		"{}", "not json", "[]", "{\"urls\":false,\"id\":12,\"has_icon\":\"yes\"}"};
	for (size_t i = 0; i < sizeof(documents) / sizeof(*documents); ++i) {
		assert_true(os_quick_write_utf8_file(path.array, documents[i], strlen(documents[i]), false));
		struct obs_module module = {.data_path = (char *)fixture};
		assert_int_equal(characterized_module_load_metadata(&module), MODULE_SUCCESS);
		struct obs_module_metadata *m = module.metadata;
		/* Characterized, not endorsed: invalid JSON still allocates empty metadata. */
		assert_non_null(m);
		const char *actual[] = {m->display_name, m->id, m->version, m->os_arch, m->name,
				       m->description, m->long_description, m->repository_url, m->website_url,
				       m->support_url};
		const char *expected[] = {"Shown", "id", "1.2", "arch", "name", "short", "long", "repo", "web", "help"};
		for (size_t j = 0; j < 10; ++j)
			assert_string_equal(actual[j], i == 0 ? expected[j] : "");
		assert_int_equal(m->has_banner, i == 0);
		assert_int_equal(m->has_icon, i == 0);
		free_module_metadata(m);
		bfree(m);
	}
	assert_int_equal(remove(path.array), 0);
	struct obs_module absent = {.data_path = (char *)fixture, .metadata = (void *)1};
	assert_int_equal(characterized_module_load_metadata(&absent), MODULE_SUCCESS);
	assert_null(absent.metadata);
	dstr_free(&path);
}

int main(int argc, char **argv)
{
	if (argc != 2)
		return 2;
	fixture = argv[1];
	const struct CMUnitTest tests[] = {cmocka_unit_test(check_manifest)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
