#include "plugins/obs-x264/build/obs-x264-test.pch.h"

#define CHECK(condition)                                                                                     \
	do {                                                                                                 \
		if (!(condition)) {                                                                          \
			fprintf(stderr, "%s:%d: error: check failed: %s\n", __FILE__, __LINE__, #condition); \
			exit(1);                                                                             \
		}                                                                                            \
	} while (0)

static void test_obs_parse_options(void)
{
	struct obs_options options;

	options = obs_parse_options(NULL);
	CHECK(options.count == 0);
	CHECK(options.ignored_word_count == 0);
	obs_free_options(options);

	options = obs_parse_options("");
	CHECK(options.count == 0);
	CHECK(options.ignored_word_count == 0);
	obs_free_options(options);

	options = obs_parse_options("ref=3");
	CHECK(options.count == 1);
	CHECK(strcmp(options.options[0].name, "ref") == 0);
	CHECK(strcmp(options.options[0].value, "3") == 0);
	CHECK(options.ignored_word_count == 0);
	obs_free_options(options);

	options = obs_parse_options("ref=3 bframes=8");
	CHECK(options.count == 2);
	CHECK(strcmp(options.options[0].name, "ref") == 0);
	CHECK(strcmp(options.options[0].value, "3") == 0);
	CHECK(strcmp(options.options[1].name, "bframes") == 0);
	CHECK(strcmp(options.options[1].value, "8") == 0);
	CHECK(options.ignored_word_count == 0);
	obs_free_options(options);

	// Invalid options are ignored.
	options = obs_parse_options("ref=3 option_with_no_equal_sign bframes=8 1234");
	CHECK(options.count == 2);
	CHECK(strcmp(options.options[0].name, "ref") == 0);
	CHECK(strcmp(options.options[0].value, "3") == 0);
	CHECK(strcmp(options.options[1].name, "bframes") == 0);
	CHECK(strcmp(options.options[1].value, "8") == 0);
	CHECK(options.ignored_word_count == 2);
	CHECK(strcmp(options.ignored_words[0], "option_with_no_equal_sign") == 0);
	CHECK(strcmp(options.ignored_words[1], "1234") == 0);
	obs_free_options(options);

	// Extra whitespace is ignored between and around options.
	options = obs_parse_options("  ref=3   bframes=8  ");
	CHECK(options.count == 2);
	CHECK(strcmp(options.options[0].name, "ref") == 0);
	CHECK(strcmp(options.options[0].value, "3") == 0);
	CHECK(strcmp(options.options[1].name, "bframes") == 0);
	CHECK(strcmp(options.options[1].value, "8") == 0);
	CHECK(options.ignored_word_count == 0);
	obs_free_options(options);
}

int main(void)
{
	test_obs_parse_options();
	return 0;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef CHECK
