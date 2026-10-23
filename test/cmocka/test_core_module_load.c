/*
 * Core modules that cannot be loaded must not take the others down with them.
 *
 * obs_load_core_modules() looks for every module in the core list next to the
 * executable ("<exe dir>/../" OBS_PLUGIN_PATH "/<name>.so"). Each case builds
 * that layout in a temporary directory with a copy of this test binary and
 * stub modules, runs the copy as a child and reads back what it loaded:
 *
 * - all_present: every core module is there and loads (control).
 * - missing_dependency: one core module is installed but a shared library it
 *   needs is not (an obs-browser.so without libcef.so): the others still load,
 *   the broken one is recorded as a disabled module (failed to open), and the load
 *   does not fail as a whole (OBS refused to start: obs-rust/obs-studio#404).
 * - missing_file: one core module is not installed at all (left out by a
 *   package split): every other core module still loads.
 * - same_libobs: the child runs on the libobs this process runs on. Tier 2
 *   mode ON picks libobs-with-rust through LD_LIBRARY_PATH; a child that falls
 *   back to the C libobs from its RUNPATH tests the C build in both modes.
 *
 * - core_module_paths: the binary and data paths each core module gets.
 *
 * The cases after those ("module search paths") pin how modules are found:
 * the search paths, the module info list, the module file lookup and the data
 * path a plugin gets. Then ("module names and files") how a module is named
 * from its binary, where its config and manifest files are, and how the safe
 * and disabled module lists match names.
 *
 * The last cases ("module type lists") pin which module a type id belongs
 * to: a type the loading module registers is in its list only once libobs
 * has accepted it, and obs_module_add_*() copies an id only into a module
 * (obs-rust/obs-studio#1953).
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <errno.h>
#include <limits.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#include <obs.h>
#include <obs-module.h>
#include <obsconfig.h>
#include <obs-core-modules.h>
#include <util/dstr.h>
#include <util/platform.h>

#define SKIP_CODE 77

/* The name of CORE_STUB_PATH's module: its file name without ".so". */
#define OBS_STUB_NAME "core_module_stub"

/* The libobs.so this process mapped, as the loader opened it; false if none. */
static bool mapped_libobs(char *path, size_t size)
{
	FILE *maps = fopen("/proc/self/maps", "r");
	if (!maps)
		return false;

	char line[4096];
	bool found = false;
	while (!found && fgets(line, sizeof(line), maps)) {
		char *file = strchr(line, '/');
		if (!file)
			continue;
		file[strcspn(file, "\n")] = '\0';
		const char *base = strrchr(file, '/') + 1;
		if (strncmp(base, "libobs.so", 9) == 0) {
			snprintf(path, size, "%s", file);
			found = true;
		}
	}
	fclose(maps);
	return found;
}

/* ------------------------------------------------------------------ child */

static int run_child(void)
{
	if (!obs_startup("en-US", NULL, NULL)) {
		printf("startup-failed\n");
		return SKIP_CODE;
	}

	char libobs[4096];
	if (mapped_libobs(libobs, sizeof(libobs)))
		printf("libobs %s\n", libobs);

	bool ok = obs_load_core_modules();
	printf("result %d\n", ok ? 1 : 0);

	for (unsigned int i = 0; i < obs_core_modules_count; i++) {
		const char *name = obs_core_modules[i];
		printf("module %s loaded=%d disabled=%d\n", name, obs_get_module(name) ? 1 : 0,
		       obs_get_disabled_module(name) ? 1 : 0);
		obs_module_t *mod = obs_get_module(name);
		if (!mod)
			mod = obs_get_disabled_module(name);
		if (mod)
			printf("paths %s\t%s\t%s\t%s\n", name, obs_get_module_file_name(mod),
			       obs_get_module_binary_path(mod), obs_get_module_data_path(mod));
	}
	fflush(stdout);

	obs_shutdown();
	return 0;
}

/* ----------------------------------------------------------------- parent */

enum scenario { ALL_PRESENT, MISSING_DEPENDENCY, MISSING_FILE };

/* What the child reported for one core module: its file name, binary path
 * and data path. */
struct child_paths {
	bool seen;
	char file[256];
	char bin[1024];
	char data[1024];
};

struct child_result {
	bool skipped;
	int load_result;
	bool loaded[256];
	bool disabled[256];
	char libobs[4096];
	/* The scenario's root, kept after it is removed. */
	char root[1024];
	/* NULL, or one per core module. */
	struct child_paths *paths;
};

static bool copy_file(const char *from, const char *to, mode_t mode)
{
	int in = open(from, O_RDONLY);
	if (in < 0)
		return false;
	int out = open(to, O_WRONLY | O_CREAT | O_TRUNC, mode);
	if (out < 0) {
		close(in);
		return false;
	}

	char buf[65536];
	ssize_t n;
	bool ok = true;
	while ((n = read(in, buf, sizeof(buf))) > 0) {
		if (write(out, buf, (size_t)n) != n) {
			ok = false;
			break;
		}
	}
	if (n < 0)
		ok = false;
	close(in);
	close(out);
	return ok;
}

static int index_of(const char *name)
{
	for (unsigned int i = 0; i < obs_core_modules_count; i++)
		if (strcmp(obs_core_modules[i], name) == 0)
			return (int)i;
	return -1;
}

static void parse_child_output(FILE *f, struct child_result *res)
{
	char line[1024];
	while (fgets(line, sizeof(line), f)) {
		char name[512];
		int loaded, failed;
		if (strncmp(line, "startup-failed", 14) == 0) {
			res->skipped = true;
		} else if (strncmp(line, "libobs ", 7) == 0) {
			snprintf(res->libobs, sizeof(res->libobs), "%s", line + 7);
			res->libobs[strcspn(res->libobs, "\n")] = '\0';
		} else if (sscanf(line, "result %d", &res->load_result) == 1) {
			continue;
		} else if (strncmp(line, "paths ", 6) == 0) {
			line[strcspn(line, "\n")] = '\0';
			char *fields[4] = {line + 6, NULL, NULL, NULL};
			for (int k = 1; k < 4 && fields[k - 1]; k++) {
				char *tab = strchr(fields[k - 1], '\t');
				if (tab) {
					*tab = '\0';
					fields[k] = tab + 1;
				}
			}
			int i = index_of(fields[0]);
			if (i >= 0 && res->paths && fields[3]) {
				struct child_paths *p = &res->paths[i];
				p->seen = true;
				snprintf(p->file, sizeof(p->file), "%s", fields[1]);
				snprintf(p->bin, sizeof(p->bin), "%s", fields[2]);
				snprintf(p->data, sizeof(p->data), "%s", fields[3]);
			}
		} else if (sscanf(line, "module %511s loaded=%d disabled=%d", name, &loaded, &failed) == 3) {
			int i = index_of(name);
			if (i >= 0) {
				res->loaded[i] = loaded != 0;
				res->disabled[i] = failed != 0;
			}
		} else {
			/* The child's log, passed through. */
			fputs(line, stderr);
		}
	}
}

static void rm_rf(const char *path)
{
	char *argv[] = {"rm", "-rf", (char *)path, NULL};
	pid_t pid = fork();
	if (pid == 0) {
		execvp("rm", argv);
		_exit(127);
	}
	if (pid > 0)
		waitpid(pid, NULL, 0);
}

/* The broken module is the first one in the list, so that every other core
 * module comes after it. */
static void run_scenario(enum scenario scenario, struct child_result *res)
{
	struct child_paths *paths = res->paths;
	memset(res, 0, sizeof(*res));
	res->paths = paths;
	res->load_result = -1;

	const char *tmp = getenv("TMPDIR");
	struct dstr root = {0};
	dstr_printf(&root, "%s/obs-core-load-XXXXXX", tmp && *tmp ? tmp : "/tmp");
	assert_non_null(mkdtemp(root.array));
	snprintf(res->root, sizeof(res->root), "%s", root.array);

	struct dstr bin_dir = {0}, exe = {0}, mod_dir = {0};
	dstr_printf(&bin_dir, "%s/bin", root.array);
	dstr_printf(&exe, "%s/test_core_module_load", bin_dir.array);
	dstr_printf(&mod_dir, "%s/" OBS_PLUGIN_PATH, root.array);
	assert_int_equal(os_mkdirs(bin_dir.array), MKDIR_SUCCESS);
	assert_int_equal(os_mkdirs(mod_dir.array), MKDIR_SUCCESS);

	char self[4096];
	ssize_t len = readlink("/proc/self/exe", self, sizeof(self) - 1);
	assert_true(len > 0);
	self[len] = '\0';
	assert_true(copy_file(self, exe.array, 0755));

	for (unsigned int i = 0; i < obs_core_modules_count; i++) {
		const char *stub = CORE_STUB_PATH;
		if (i == 0 && scenario == MISSING_FILE)
			continue;
		if (i == 0 && scenario == MISSING_DEPENDENCY)
			stub = CORE_STUB_NEEDS_DEP_PATH;

		struct dstr dst = {0};
		dstr_printf(&dst, "%s/%s.so", mod_dir.array, obs_core_modules[i]);
		assert_true(copy_file(stub, dst.array, 0755));
		dstr_free(&dst);
	}

	/* The directory of the libobs this process runs on: Tier 2 mode ON
	 * selects libobs-with-rust through LD_LIBRARY_PATH. */
	char libobs_dir[4096];
	assert_true(mapped_libobs(libobs_dir, sizeof(libobs_dir)));
	*strrchr(libobs_dir, '/') = '\0';

	int fds[2];
	assert_int_equal(pipe(fds), 0);
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		dup2(fds[1], STDOUT_FILENO);
		close(fds[0]);
		close(fds[1]);
		/* The dependency must stay out of reach, as libcef.so was, and
		 * the child must run on the same libobs: LD_LIBRARY_PATH names
		 * that libobs's directory and nothing else. */
		setenv("LD_LIBRARY_PATH", libobs_dir, 1);
		char *argv[] = {exe.array, "--child", NULL};
		execv(exe.array, argv);
		_exit(127);
	}
	close(fds[1]);
	FILE *f = fdopen(fds[0], "r");
	parse_child_output(f, res);
	fclose(f);

	int status = 0;
	waitpid(pid, &status, 0);
	if (WIFEXITED(status) && WEXITSTATUS(status) == SKIP_CODE)
		res->skipped = true;
	else
		assert_true(WIFEXITED(status) && WEXITSTATUS(status) == 0);

	rm_rf(root.array);
	dstr_free(&mod_dir);
	dstr_free(&exe);
	dstr_free(&bin_dir);
	dstr_free(&root);
}

static void check_others_loaded(const struct child_result *res)
{
	for (unsigned int i = 1; i < obs_core_modules_count; i++) {
		if (!res->loaded[i])
			print_error("core module '%s' was not loaded\n", obs_core_modules[i]);
		assert_true(res->loaded[i]);
	}
}

/* ------------------------------------------- obs_add_module_path() type */

/*
 * obs_add_module_path() is the search path API from before module types. Its
 * paths carry no type, so obs_find_modules2() reports their modules as
 * OBS_MODULE_TYPE_UNDEFINED, and libobs's loader treats that type the way it
 * treated every module before types existed: the disabled list applies, and a
 * module named like a core module is not skipped as a legacy copy of it
 * (obs-rust/obs-studio#1900).
 *
 * The type used to be left uninitialized on the stack. stain_stack() first
 * fills the stack that obs_add_module_path() then runs on with
 * OBS_MODULE_TYPE_CORE, so an unset type reads as CORE rather than as
 * whatever happens to be there, often 0 (UNDEFINED) by luck.
 */

#define STAIN_WORDS 8192

static __attribute__((noinline)) void stain_stack(void)
{
	volatile uint32_t words[STAIN_WORDS];
	for (size_t i = 0; i < STAIN_WORDS; i++)
		words[i] = OBS_MODULE_TYPE_CORE;
}

struct typed_found {
	size_t count;
	enum obs_runtime_module_type type[4];
};

static void collect_typed(void *param, const struct obs_module_info2 *info)
{
	struct typed_found *found = param;
	if (found->count < 4)
		found->type[found->count] = info->type;
	found->count++;
}

struct type_fixture {
	struct dstr root;
	bool started;
};

static int type_setup(void **state)
{
	struct type_fixture *f = calloc(1, sizeof(*f));
	assert_non_null(f);

	const char *tmp = getenv("TMPDIR");
	if (tmp && *tmp) {
		dstr_printf(&f->root, "%s/obs-module-path-type-XXXXXX", tmp);
	} else {
		char self[4096];
		ssize_t len = readlink("/proc/self/exe", self, sizeof(self) - 1);
		assert_true(len > 0);
		self[len] = '\0';
		*strrchr(self, '/') = '\0';
		dstr_printf(&f->root, "%s/obs-module-path-type-XXXXXX", self);
	}
	assert_non_null(mkdtemp(f->root.array));
	*state = f;
	return 0;
}

static int type_teardown(void **state)
{
	struct type_fixture *f = *state;
	if (f->started)
		obs_shutdown();
	rm_rf(f->root.array);
	dstr_free(&f->root);
	free(f);
	return 0;
}

static void module_path_type(void **state)
{
	struct type_fixture *f = *state;

	struct dstr bin = {0}, data = {0}, file = {0};
	dstr_printf(&bin, "%s/bin", f->root.array);
	dstr_printf(&data, "%s/data/%%module%%", f->root.array);
	assert_int_equal(os_mkdirs(bin.array), MKDIR_SUCCESS);

	/* One module on the disabled list, one named like a core module. */
	const char *disabled = "module-path-type-disabled";
	const char *core_named = obs_core_modules[0];
	dstr_printf(&file, "%s/%s.so", bin.array, disabled);
	assert_true(copy_file(CORE_STUB_PATH, file.array, 0755));
	dstr_printf(&file, "%s/%s.so", bin.array, core_named);
	assert_true(copy_file(CORE_STUB_PATH, file.array, 0755));
	dstr_free(&file);

	f->started = obs_startup("en-US", NULL, NULL);
	if (!f->started) {
		dstr_free(&data);
		dstr_free(&bin);
		skip();
	}
	obs_add_disabled_module(disabled);

	stain_stack();
	obs_add_module_path(bin.array, data.array);

	struct typed_found found = {0};
	obs_find_modules2(collect_typed, &found);

	/* Load what was found as a caller of obs_find_modules2() would with
	 * libobs's loader: from the same path, with the type reported. */
	struct obs_runtime_module_info info = {
		.path_info = {.binary = bin.array, .data = data.array},
		.type = found.type[0],
	};
	struct obs_module_failure_info failures = {0};
	obs_load_plugins(&info, &failures);
	obs_module_failure_info_free(&failures);
	dstr_free(&data);
	dstr_free(&bin);

	obs_module_t *core_named_mod = obs_get_module(core_named);
	print_message("types %d %d; disabled loaded=%d listed=%d; core-named loaded=%d core=%d legacy=%d\n",
		      found.type[0], found.type[1], obs_get_module(disabled) != NULL,
		      obs_get_disabled_module(disabled) != NULL, core_named_mod != NULL,
		      obs_is_core_module(core_named_mod), obs_is_legacy_module(core_named_mod));

	assert_int_equal(found.count, 2);
	for (size_t i = 0; i < 2; i++)
		assert_int_equal(found.type[i], OBS_MODULE_TYPE_UNDEFINED);

	/* The disabled list applies, as it did before module types. */
	assert_null(obs_get_module(disabled));
	assert_non_null(obs_get_disabled_module(disabled));

	/* Not skipped as a legacy copy of a core module, and neither core nor
	 * legacy once loaded. */
	assert_non_null(core_named_mod);
	assert_false(obs_is_core_module(core_named_mod));
	assert_false(obs_is_legacy_module(core_named_mod));
}

static void all_present(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_result res = {0};
	run_scenario(ALL_PRESENT, &res);
	if (res.skipped)
		skip();

	assert_int_equal(res.load_result, 1);
	assert_true(res.loaded[0]);
	check_others_loaded(&res);
}

static void missing_dependency(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_result res = {0};
	run_scenario(MISSING_DEPENDENCY, &res);
	if (res.skipped)
		skip();

	/* OBS must still start: the frontend aborts when this is false. */
	assert_int_equal(res.load_result, 1);
	assert_false(res.loaded[0]);
	/* Listed as a module that could not be loaded, as plugins are. */
	assert_true(res.disabled[0]);
	check_others_loaded(&res);
}

static void missing_file(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_result res = {0};
	run_scenario(MISSING_FILE, &res);
	if (res.skipped)
		skip();

	assert_int_equal(res.load_result, 1);
	assert_false(res.loaded[0]);
	check_others_loaded(&res);
}

static void same_libobs(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_result res = {0};
	run_scenario(ALL_PRESENT, &res);
	if (res.skipped)
		skip();

	char parent[4096];
	assert_true(mapped_libobs(parent, sizeof(parent)));
	assert_true(res.libobs[0] != '\0');

	char parent_real[PATH_MAX], child_real[PATH_MAX];
	assert_non_null(realpath(parent, parent_real));
	assert_non_null(realpath(res.libobs, child_real));
	if (strcmp(parent_real, child_real) != 0)
		print_error("child loaded %s, this process runs on %s\n", child_real, parent_real);
	assert_string_equal(child_real, parent_real);
}

/* Every core module, loaded or not, gets "<exe dir>/../" OBS_PLUGIN_PATH
 * "/<name>.so" as its binary path, its file name after the last '/', and
 * "<exe dir>/../" OBS_DATA_PATH "/obs-modules/core/<name>" as its data path,
 * whether or not that exists (obs-rust/obs-studio#1876). The first one, whose
 * dependency is missing, is a disabled module with the same paths. */
static void core_module_paths(void **state)
{
	UNUSED_PARAMETER(state);
	struct child_paths *paths = calloc(256, sizeof(*paths));
	assert_non_null(paths);
	struct child_result res = {.paths = paths};
	run_scenario(MISSING_DEPENDENCY, &res);
	if (res.skipped) {
		free(paths);
		skip();
	}

	assert_true(res.disabled[0]);
	for (unsigned int i = 0; i < obs_core_modules_count; i++) {
		const char *name = obs_core_modules[i];
		struct dstr bin = {0}, data = {0}, file = {0};
		dstr_printf(&bin, "%s/bin/../" OBS_PLUGIN_PATH "/%s.so", res.root, name);
		dstr_printf(&data, "%s/bin/../" OBS_DATA_PATH "/obs-modules/core/%s", res.root, name);
		dstr_printf(&file, "%s.so", name);
		if (!paths[i].seen)
			print_error("core module '%s' reported no paths\n", name);
		assert_true(paths[i].seen);
		assert_string_equal(paths[i].file, file.array);
		assert_string_equal(paths[i].bin, bin.array);
		assert_string_equal(paths[i].data, data.array);
		dstr_free(&bin);
		dstr_free(&data);
		dstr_free(&file);
	}
	free(paths);
}

/* --------------------------------------------------- module search paths */

/*
 * The module search paths (obs_add_module_path, obs_find_modules2,
 * obs_find_modules), the module info list they report, the module file lookup
 * (obs_find_module_file, obs_get_module_data_path) and the data path that
 * obs_load_plugins gives a plugin (obs-rust/obs-studio#1876). Each case runs
 * on a libobs of its own in this process, with a layout of empty files and
 * directories under a new root in TMPDIR, else next to this binary. Paths are
 * compared with that root written as "R".
 *
 * Characterized, not endorsed:
 * - A data path is reported whether or not it exists. For a name that starts
 *   with "lib" (any case) whose data directory does not exist, the data path
 *   is the one without "lib", whether or not that one exists.
 * - A module directory whose name has a '.' is looked up under the name up to
 *   its last '.', so it is not found.
 * - A directory passes for a module binary.
 * - An empty data path, or "/", reports no module at all.
 * - Only one trailing '/' is taken off a data path.
 * - A path added twice reports its modules twice.
 * - obs_find_module_file() with a NULL or empty file returns the data path
 *   itself when it exists; with an empty or NULL data path the file is looked
 *   up relative to the working directory; a file that starts with '/' is
 *   appended after one more ("R/data//f").
 */

#define MAX_FOUND 32

struct found_module {
	char bin[1024];
	char data[1024];
	char name[256];
	bool bin_null;
	bool data_null;
	bool name_null;
};

struct found_list {
	const char *root;
	size_t count;
	struct found_module mods[MAX_FOUND];
};

struct paths_fixture {
	struct dstr root;
	struct dstr buf[4];
	size_t next_buf;
	bool started;
	struct found_list list;
};

/* "R" for the root, then the rest of the path. */
static void copy_rel(char *out, size_t size, const char *root, const char *path, bool *was_null)
{
	*was_null = !path;
	if (!path) {
		out[0] = '\0';
		return;
	}
	size_t n = strlen(root);
	if (n && strncmp(path, root, n) == 0)
		snprintf(out, size, "R%s", path + n);
	else
		snprintf(out, size, "%s", path);
}

static struct found_module *next_found(struct found_list *list)
{
	if (list->count >= MAX_FOUND) {
		list->count++;
		return NULL;
	}
	return &list->mods[list->count++];
}

static void collect_module(void *param, const struct obs_module_info2 *info)
{
	struct found_list *list = param;
	struct found_module *m = next_found(list);
	if (!m)
		return;
	copy_rel(m->bin, sizeof(m->bin), list->root, info->bin_path, &m->bin_null);
	copy_rel(m->data, sizeof(m->data), list->root, info->data_path, &m->data_null);
	copy_rel(m->name, sizeof(m->name), "", info->name, &m->name_null);
}

static void collect_module_v1(void *param, const struct obs_module_info *info)
{
	struct found_list *list = param;
	struct found_module *m = next_found(list);
	if (!m)
		return;
	copy_rel(m->bin, sizeof(m->bin), list->root, info->bin_path, &m->bin_null);
	copy_rel(m->data, sizeof(m->data), list->root, info->data_path, &m->data_null);
	m->name_null = true;
}

static void expect_module(const struct found_list *list, size_t i, const char *bin, const char *data, const char *name)
{
	assert_true(i < list->count);
	const struct found_module *m = &list->mods[i];
	assert_false(m->bin_null);
	assert_string_equal(m->bin, bin);
	assert_false(m->data_null);
	assert_string_equal(m->data, data);
	if (name) {
		assert_false(m->name_null);
		assert_string_equal(m->name, name);
	}
}

static struct found_list *find_all(struct paths_fixture *f)
{
	memset(&f->list, 0, sizeof(f->list));
	f->list.root = f->root.array;
	obs_find_modules2(collect_module, &f->list);
	return &f->list;
}

/* The root and rel, valid for the next three calls. */
static const char *at(struct paths_fixture *f, const char *rel)
{
	struct dstr *out = &f->buf[f->next_buf++ % 4];
	dstr_printf(out, "%s%s", f->root.array, rel);
	return out->array;
}

/* rel ending in '/' makes a directory, else an empty file; parents too. */
static void make(struct paths_fixture *f, const char *rel)
{
	struct dstr path = {0};
	dstr_printf(&path, "%s%s", f->root.array, rel);
	char *slash = strrchr(path.array, '/');
	if (slash[1] == '\0') {
		assert_int_not_equal(os_mkdirs(path.array), MKDIR_ERROR);
	} else {
		*slash = '\0';
		assert_int_not_equal(os_mkdirs(path.array), MKDIR_ERROR);
		*slash = '/';
		FILE *file = fopen(path.array, "wb");
		assert_non_null(file);
		fclose(file);
	}
	dstr_free(&path);
}

static int paths_setup(void **state)
{
	struct paths_fixture *f = calloc(1, sizeof(*f));
	assert_non_null(f);

	const char *tmp = getenv("TMPDIR");
	if (tmp && *tmp) {
		dstr_printf(&f->root, "%s/obs-module-paths-XXXXXX", tmp);
	} else {
		char self[4096];
		ssize_t len = readlink("/proc/self/exe", self, sizeof(self) - 1);
		assert_true(len > 0);
		self[len] = '\0';
		*strrchr(self, '/') = '\0';
		dstr_printf(&f->root, "%s/obs-module-paths-XXXXXX", self);
	}
	assert_non_null(mkdtemp(f->root.array));

	f->started = obs_startup("en-US", NULL, NULL);
	*state = f;
	return 0;
}

static int paths_teardown(void **state)
{
	struct paths_fixture *f = *state;
	if (f->started)
		obs_shutdown();
	rm_rf(f->root.array);
	dstr_free(&f->root);
	for (size_t i = 0; i < 4; i++)
		dstr_free(&f->buf[i]);
	free(f);
	return 0;
}

static struct paths_fixture *paths_begin(void **state)
{
	struct paths_fixture *f = *state;
	if (!f->started)
		skip();
	return f;
}

/* A binary directory without %module%: every *.so file in it, in glob order.
 * Directories (dir.so/ too) and other files are not modules. */
static void search_flat_directory(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/plugins/a.so");
	make(f, "/plugins/d.so");
	make(f, "/plugins/libb.so");
	make(f, "/plugins/libe.so");
	make(f, "/plugins/libf.so");
	make(f, "/plugins/c.txt");
	make(f, "/plugins/sub/");
	make(f, "/plugins/dir.so/");
	make(f, "/data/a/");
	make(f, "/data/b/");
	make(f, "/data/libf/");

	obs_add_module_path(at(f, "/plugins"), at(f, "/data/%module%"));
	struct found_list *list = find_all(f);

	assert_int_equal(list->count, 5);
	expect_module(list, 0, "R/plugins/a.so", "R/data/a", "a");
	/* Reported though data/d does not exist. */
	expect_module(list, 1, "R/plugins/d.so", "R/data/d", "d");
	/* data/libb does not exist: data/b. */
	expect_module(list, 2, "R/plugins/libb.so", "R/data/b", "libb");
	/* Neither exists: still the one without "lib". */
	expect_module(list, 3, "R/plugins/libe.so", "R/data/e", "libe");
	/* data/libf exists: kept. */
	expect_module(list, 4, "R/plugins/libf.so", "R/data/libf", "libf");
}

/* One '/' is added to a binary directory without one; one trailing '/' is
 * taken off a data path, not two. */
static void search_trailing_separators(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/plugins/a.so");
	make(f, "/p2/x.so");
	make(f, "/p3/LIBy.so");

	obs_add_module_path(at(f, "/plugins/"), at(f, "/data/%module%/"));
	obs_add_module_path(at(f, "/p2"), at(f, "/d2/%module%//"));
	obs_add_module_path(at(f, "/p3"), at(f, "/d3/%module%/"));
	struct found_list *list = find_all(f);

	assert_int_equal(list->count, 3);
	expect_module(list, 0, "R/plugins/a.so", "R/data/a", "a");
	expect_module(list, 1, "R/p2/x.so", "R/d2/x/", "x");
	/* "lib" in any case. */
	expect_module(list, 2, "R/p3/LIBy.so", "R/d3/y", "LIBy");
}

/* %module% in the binary directory: every directory where the path up to
 * %module% ends, by name; <name>.so first, then lib<name>.so. */
static void search_module_directories(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/mods/x/bin/64bit/x.so");
	make(f, "/mods/x/data/");
	make(f, "/mods/y/bin/64bit/liby.so");
	make(f, "/mods/z/bin/64bit/");
	make(f, "/mods/w.so");
	make(f, "/mods/p.q/bin/64bit/p.so");
	make(f, "/mods/p.q/bin/64bit/p.q.so");
	make(f, "/mods/libl/bin/64bit/libl.so");
	make(f, "/mods/v/bin/64bit/v.so/");

	obs_add_module_path(at(f, "/mods/%module%/bin/64bit"), at(f, "/mods/%module%/data"));
	struct found_list *list = find_all(f);

	/* p.q is looked up as "p" (mods/p/...), w.so is a file, z has no binary. */
	assert_int_equal(list->count, 4);
	/* mods/libl/data does not exist: mods/l/data. */
	expect_module(list, 0, "R/mods/libl/bin/64bit/libl.so", "R/mods/l/data", "libl");
	/* A directory named v.so passes for the binary. */
	expect_module(list, 1, "R/mods/v/bin/64bit/v.so", "R/mods/v/data", "v");
	expect_module(list, 2, "R/mods/x/bin/64bit/x.so", "R/mods/x/data", "x");
	expect_module(list, 3, "R/mods/y/bin/64bit/liby.so", "R/mods/y/data", "y");
}

/* Every %module% of the data path, and of the binary directory after the
 * first, is the name; a data path without %module% is the same for every
 * module. */
static void search_repeated_placeholder(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/plugins/a.so");
	make(f, "/mods/x/x/x.so");

	obs_add_module_path(at(f, "/plugins"), at(f, "/data/%module%/%module%"));
	obs_add_module_path(at(f, "/plugins"), at(f, "/shared"));
	obs_add_module_path(at(f, "/mods/%module%/%module%"), at(f, "/md/%module%"));
	struct found_list *list = find_all(f);

	assert_int_equal(list->count, 3);
	expect_module(list, 0, "R/plugins/a.so", "R/data/a/a", "a");
	expect_module(list, 1, "R/plugins/a.so", "R/shared", "a");
	expect_module(list, 2, "R/mods/x/x/x.so", "R/md/x", "x");
}

/* A binary directory that does not exist reports nothing; NULL paths are not
 * added; an empty data path or "/" reports no module. */
static void search_missing_and_empty(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/plugins/a.so");
	make(f, "/plugins/libb.so");

	obs_add_module_path(at(f, "/nowhere"), at(f, "/data/%module%"));
	obs_add_module_path(at(f, "/mods/%module%/bin"), at(f, "/data/%module%"));
	obs_add_module_path(NULL, at(f, "/data/%module%"));
	obs_add_module_path(at(f, "/plugins"), NULL);
	obs_add_module_path(at(f, "/plugins"), "");
	obs_add_module_path(at(f, "/plugins"), "/");
	struct found_list *list = find_all(f);
	assert_int_equal(list->count, 0);

	/* A path added after those is searched. */
	obs_add_module_path(at(f, "/plugins"), at(f, "/data/%module%"));
	list = find_all(f);
	assert_int_equal(list->count, 2);
	expect_module(list, 0, "R/plugins/a.so", "R/data/a", "a");
	expect_module(list, 1, "R/plugins/libb.so", "R/data/b", "libb");
}

/* A path added twice is searched twice; obs_find_modules() reports the same
 * binary and data paths. */
static void search_duplicate_paths(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/plugins/a.so");

	obs_add_module_path(at(f, "/plugins"), at(f, "/data/%module%"));
	obs_add_module_path(at(f, "/plugins"), at(f, "/data/%module%"));
	struct found_list *list = find_all(f);
	assert_int_equal(list->count, 2);
	expect_module(list, 0, "R/plugins/a.so", "R/data/a", "a");
	expect_module(list, 1, "R/plugins/a.so", "R/data/a", "a");

	memset(list, 0, sizeof(*list));
	list->root = f->root.array;
	obs_find_modules(collect_module_v1, list);
	assert_int_equal(list->count, 2);
	expect_module(list, 0, "R/plugins/a.so", "R/data/a", NULL);
	expect_module(list, 1, "R/plugins/a.so", "R/data/a", NULL);
}

static void expect_file(struct paths_fixture *f, obs_module_t *mod, const char *file, const char *expected)
{
	char *found = obs_find_module_file(mod, file);
	if (!expected) {
		if (found)
			print_error("obs_find_module_file(%s) = %s\n", file ? file : "NULL", found);
		assert_null(found);
		return;
	}
	assert_non_null(found);
	char rel[1024];
	bool was_null;
	copy_rel(rel, sizeof(rel), f->root.array, found, &was_null);
	assert_string_equal(rel, expected);
	bfree(found);
}

/* obs_find_module_file(): the data path, one '/', the file; NULL when that
 * does not exist. */
static void find_module_file(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/data/stub/locale/en-US.ini");
	make(f, "/data/stub/sub/");

	obs_module_t *mod = NULL;
	assert_int_equal(obs_open_module(&mod, CORE_STUB_PATH, at(f, "/data/stub")), MODULE_SUCCESS);
	assert_string_equal(obs_get_module_data_path(mod), at(f, "/data/stub"));
	assert_null(obs_get_module_data_path(NULL));

	expect_file(f, mod, "locale/en-US.ini", "R/data/stub/locale/en-US.ini");
	expect_file(f, mod, "locale/xx-XX.ini", NULL);
	expect_file(f, mod, "sub", "R/data/stub/sub");
	expect_file(f, mod, "sub/", "R/data/stub/sub/");
	expect_file(f, mod, NULL, "R/data/stub");
	expect_file(f, mod, "", "R/data/stub");
	expect_file(f, mod, "/locale/en-US.ini", "R/data/stub//locale/en-US.ini");
	expect_file(f, NULL, "locale/en-US.ini", NULL);
	expect_file(f, NULL, NULL, NULL);

	/* A trailing '/' on the data path is not doubled. */
	obs_module_t *slash = NULL;
	assert_int_equal(obs_open_module(&slash, CORE_STUB_PATH, at(f, "/data/stub/")), MODULE_SUCCESS);
	assert_string_equal(obs_get_module_data_path(slash), at(f, "/data/stub/"));
	expect_file(f, slash, "locale/en-US.ini", "R/data/stub/locale/en-US.ini");
	expect_file(f, slash, NULL, "R/data/stub/");

	/* A data path that does not exist. */
	obs_module_t *missing = NULL;
	assert_int_equal(obs_open_module(&missing, CORE_STUB_PATH, at(f, "/nodata")), MODULE_SUCCESS);
	expect_file(f, missing, "locale/en-US.ini", NULL);
	expect_file(f, missing, NULL, NULL);
}

/* An empty or NULL data path looks the file up relative to the working
 * directory. */
static void find_module_file_relative(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/rel.txt");

	obs_module_t *empty = NULL, *null = NULL;
	assert_int_equal(obs_open_module(&empty, CORE_STUB_PATH, ""), MODULE_SUCCESS);
	assert_int_equal(obs_open_module(&null, CORE_STUB_PATH, NULL), MODULE_SUCCESS);
	assert_string_equal(obs_get_module_data_path(empty), "");
	assert_null(obs_get_module_data_path(null));

	char cwd[4096];
	assert_non_null(getcwd(cwd, sizeof(cwd)));
	assert_int_equal(chdir(f->root.array), 0);
	char *found = obs_find_module_file(empty, "rel.txt");
	char *found_null = obs_find_module_file(null, "rel.txt");
	char *none = obs_find_module_file(empty, "none.txt");
	char *dir = obs_find_module_file(empty, NULL);
	assert_int_equal(chdir(cwd), 0);

	assert_non_null(found);
	assert_string_equal(found, "rel.txt");
	assert_non_null(found_null);
	assert_string_equal(found_null, "rel.txt");
	assert_null(none);
	assert_null(dir);
	bfree(found);
	bfree(found_null);
}

/* obs_load_plugins() searches its one path as obs_find_modules2() does and
 * gives each plugin the data path it found. */
static void load_plugins_data_path(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/plugs/pathstub/bin/64bit/");
	make(f, "/plugs/pathstub/data/locale/");
	make(f, "/plugs/libpathlib/bin/64bit/");
	make(f, "/plugs/pathlib/data/");
	assert_true(copy_file(CORE_STUB_PATH, at(f, "/plugs/pathstub/bin/64bit/pathstub.so"), 0755));
	assert_true(copy_file(CORE_STUB_PATH, at(f, "/plugs/libpathlib/bin/64bit/libpathlib.so"), 0755));

	struct dstr bin = {0}, data = {0};
	dstr_copy(&bin, at(f, "/plugs/%module%/bin/64bit"));
	dstr_copy(&data, at(f, "/plugs/%module%/data"));
	struct obs_runtime_module_info info = {
		.path_info = {.binary = bin.array, .data = data.array},
		.type = OBS_MODULE_TYPE_PLUGIN,
	};
	struct obs_module_failure_info failures = {0};
	obs_load_plugins(&info, &failures);
	assert_int_equal(failures.count, 0);
	obs_module_failure_info_free(&failures);

	obs_module_t *mod = obs_get_module("pathstub");
	assert_non_null(mod);
	assert_string_equal(obs_get_module_data_path(mod), at(f, "/plugs/pathstub/data"));
	expect_file(f, mod, "locale", "R/plugs/pathstub/data/locale");

	obs_module_t *lib = obs_get_module("libpathlib");
	assert_non_null(lib);
	assert_string_equal(obs_get_module_data_path(lib), at(f, "/plugs/pathlib/data"));

	dstr_free(&bin);
	dstr_free(&data);
}

/* ------------------------------------------------ module names and files */

/*
 * How a module is named from its binary (obs_open_module,
 * obs_create_disabled_module: obs_get_module_file_name, the name that
 * obs_get_module and obs_get_disabled_module look up), where its config file
 * is (obs_module_get_config_path) and its manifest (obs_get_module_id), and
 * how the safe and disabled module lists match names (obs_add_safe_module,
 * obs_add_disabled_module, obs_load_plugins) (obs-rust/obs-studio#1876). The
 * name shows in the config path, which is "<config>/<name>/<file>".
 *
 * Characterized, not endorsed:
 * - The name is the file name without its last three bytes (the length of
 *   ".so"), whatever they are: "noext" is "no", "x.dll" is "x.".
 * - A file name of three bytes, or ".so", has a NULL name: its config path is
 *   "<config>//<file>". (A name lookup then dereferences it, so none is made.)
 * - A module without a config path, or an empty one, has a relative config
 *   path; only one '/' is checked at the end of the config path.
 * - A module without a data path, or an empty one, reads "manifest.json" in
 *   the working directory.
 * - The safe list and the disabled list match whole names, case-sensitively.
 *   The disabled list does not apply to core modules.
 */

/* Shuts the libobs of the fixture down and starts a new one. */
static void restart(struct paths_fixture *f, const char *config)
{
	if (f->started)
		obs_shutdown();
	f->started = obs_startup("en-US", config, NULL);
	assert_true(f->started);
}

static void write_text(struct paths_fixture *f, const char *rel, const char *text)
{
	make(f, rel);
	FILE *file = fopen(at(f, rel), "wb");
	assert_non_null(file);
	assert_int_equal(fputs(text, file) >= 0, 1);
	fclose(file);
}

static obs_module_t *disabled_module(const char *path, const char *data)
{
	obs_module_t *mod = NULL;
	assert_true(obs_create_disabled_module(&mod, path, data, OBS_MODULE_DISABLED));
	assert_non_null(mod);
	return mod;
}

static void expect_config(struct paths_fixture *f, obs_module_t *mod, const char *file, const char *expected)
{
	char *path = obs_module_get_config_path(mod, file);
	assert_non_null(path);
	char rel[1024];
	bool was_null;
	copy_rel(rel, sizeof(rel), f->root.array, path, &was_null);
	assert_string_equal(rel, expected);
	bfree(path);
}

/* The file name and name of a module with the binary path, the name as its
 * config path under "R/cfg" shows it. */
static void expect_names(struct paths_fixture *f, const char *path, const char *file, const char *config)
{
	obs_module_t *mod = disabled_module(path, at(f, "/nodata"));
	assert_string_equal(obs_get_module_binary_path(mod), path);
	const char *bin = obs_get_module_binary_path(mod);
	const char *name = obs_get_module_file_name(mod);
	assert_string_equal(name, file);
	/* The file name points into the binary path. */
	assert_ptr_equal(name, bin + strlen(bin) - strlen(file));
	expect_config(f, mod, "f", config);
}

/* The file name is after the last '/'; the name is the file name without its
 * last three bytes. */
static void module_names(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	restart(f, at(f, "/cfg"));

	expect_names(f, at(f, "/n/stub.so"), "stub.so", "R/cfg/stub/f");
	expect_names(f, at(f, "/n/a.b.so"), "a.b.so", "R/cfg/a.b/f");
	expect_names(f, at(f, "/n/libz.so"), "libz.so", "R/cfg/libz/f");
	expect_names(f, at(f, "/n/LIBZ.SO"), "LIBZ.SO", "R/cfg/LIBZ/f");
	expect_names(f, at(f, "/n/noext"), "noext", "R/cfg/no/f");
	expect_names(f, at(f, "/n/x.dll"), "x.dll", "R/cfg/x./f");
	expect_names(f, at(f, "/n/d.so/x.so"), "x.so", "R/cfg/x/f");
	expect_names(f, at(f, "/n/a\\b.so"), "a\\b.so", "R/cfg/a\\b/f");
	expect_names(f, at(f, "/n/sp ace.so"), "sp ace.so", "R/cfg/sp ace/f");
	expect_names(f, at(f, "//n//two.so"), "two.so", "R/cfg/two/f");
	expect_names(f, "plain.so", "plain.so", "R/cfg/plain/f");
	expect_names(f, "abcd", "abcd", "R/cfg/a/f");

	/* Looked up by name, the newest first. */
	obs_module_t *stub = obs_get_disabled_module("stub");
	assert_non_null(stub);
	assert_string_equal(obs_get_module_file_name(stub), "stub.so");
	assert_non_null(obs_get_disabled_module("a.b"));
	assert_non_null(obs_get_disabled_module("x."));
	assert_null(obs_get_disabled_module("stub.so"));
	assert_null(obs_get_disabled_module("x.dll"));
	assert_null(obs_get_disabled_module("Stub"));
	assert_null(obs_get_module("stub"));

	/* A loaded module is named the same way. */
	obs_module_t *mod = NULL;
	assert_int_equal(obs_open_module(&mod, CORE_STUB_PATH, at(f, "/nodata")), MODULE_SUCCESS);
	const char *slash = strrchr(CORE_STUB_PATH, '/');
	assert_string_equal(obs_get_module_file_name(mod), slash ? slash + 1 : CORE_STUB_PATH);
	assert_string_equal(obs_get_module_binary_path(mod), CORE_STUB_PATH);
	assert_ptr_equal(obs_get_module(OBS_STUB_NAME), mod);
	assert_null(obs_get_disabled_module(OBS_STUB_NAME));

	/* Last: names that are NULL. */
	expect_names(f, at(f, "/n/abc"), "abc", "R/cfg//f");
	expect_names(f, at(f, "/n/.so"), ".so", "R/cfg//f");
}

/* obs_module_get_config_path(): the config path, one '/' when it has none at
 * the end, the name, '/', the file. */
static void module_config_path(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	const char *stub = at(f, "/n/stub.so");
	struct dstr bin = {0};
	dstr_copy(&bin, stub);

	restart(f, at(f, "/cfg"));
	obs_module_t *mod = disabled_module(bin.array, at(f, "/nodata"));
	expect_config(f, mod, "f.json", "R/cfg/stub/f.json");
	expect_config(f, mod, "a/b.ini", "R/cfg/stub/a/b.ini");
	expect_config(f, mod, "/abs", "R/cfg/stub//abs");
	expect_config(f, mod, "", "R/cfg/stub/");
	expect_config(f, mod, NULL, "R/cfg/stub/");

	restart(f, at(f, "/cfg/"));
	mod = disabled_module(bin.array, at(f, "/nodata"));
	expect_config(f, mod, "f.json", "R/cfg/stub/f.json");

	restart(f, at(f, "/cfg//"));
	mod = disabled_module(bin.array, at(f, "/nodata"));
	expect_config(f, mod, "f.json", "R/cfg//stub/f.json");

	restart(f, "/");
	mod = disabled_module(bin.array, at(f, "/nodata"));
	expect_config(f, mod, "f.json", "/stub/f.json");

	restart(f, "rel");
	mod = disabled_module(bin.array, at(f, "/nodata"));
	expect_config(f, mod, "f.json", "rel/stub/f.json");

	restart(f, "");
	mod = disabled_module(bin.array, at(f, "/nodata"));
	expect_config(f, mod, "f.json", "stub/f.json");
	expect_config(f, mod, NULL, "stub/");

	restart(f, NULL);
	mod = disabled_module(bin.array, at(f, "/nodata"));
	expect_config(f, mod, "f.json", "stub/f.json");

	/* A loaded module too. */
	restart(f, at(f, "/cfg"));
	mod = NULL;
	assert_int_equal(obs_open_module(&mod, CORE_STUB_PATH, at(f, "/nodata")), MODULE_SUCCESS);
	expect_config(f, mod, "f.json", "R/cfg/" OBS_STUB_NAME "/f.json");

	/* No module: NULL, as obs_find_module_file() returns. */
	assert_null(obs_module_get_config_path(NULL, "f.json"));
	assert_null(obs_module_get_config_path(NULL, NULL));

	dstr_free(&bin);
}

#define MANIFEST "{\"id\": \"%s\", \"version\": \"1.2\", \"display_name\": \"Shown\"}"

static void write_manifest(struct paths_fixture *f, const char *dir, const char *id)
{
	struct dstr rel = {0}, text = {0};
	dstr_printf(&rel, "%s/manifest.json", dir);
	dstr_printf(&text, MANIFEST, id);
	write_text(f, rel.array, text.array);
	dstr_free(&rel);
	dstr_free(&text);
}

static void expect_id(obs_module_t *mod, const char *id)
{
	if (!id) {
		assert_null(obs_get_module_id(mod));
		return;
	}
	assert_non_null(obs_get_module_id(mod));
	assert_string_equal(obs_get_module_id(mod), id);
}

/* The manifest is "manifest.json" in the data path, with one '/' between
 * them. */
static void module_manifest(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	write_manifest(f, "/m1", "m1-id");
	write_manifest(f, "/m1/sub", "sub-id");
	make(f, "/m2/");
	struct dstr bin = {0};
	dstr_copy(&bin, at(f, "/n/stub.so"));
	const char *stub = bin.array;

	obs_module_t *mod = disabled_module(stub, at(f, "/m1"));
	expect_id(mod, "m1-id");
	assert_string_equal(obs_get_module_version(mod), "1.2");
	assert_string_equal(obs_get_module_name(mod), "Shown");
	expect_id(disabled_module(stub, at(f, "/m1/")), "m1-id");
	expect_id(disabled_module(stub, at(f, "/m1/sub")), "sub-id");
	expect_id(disabled_module(stub, at(f, "/m2")), NULL);
	expect_id(disabled_module(stub, at(f, "/none")), NULL);
	expect_id(disabled_module(stub, at(f, "/m1/manifest.json")), NULL);

	/* No data path: the working directory's manifest. */
	char cwd[4096];
	assert_non_null(getcwd(cwd, sizeof(cwd)));
	assert_int_equal(chdir(at(f, "/m1")), 0);
	obs_module_t *empty = disabled_module(stub, "");
	obs_module_t *null = disabled_module(stub, NULL);
	assert_int_equal(chdir(at(f, "/m2")), 0);
	obs_module_t *empty_none = disabled_module(stub, "");
	assert_int_equal(chdir(cwd), 0);
	expect_id(empty, "m1-id");
	expect_id(null, "m1-id");
	expect_id(empty_none, NULL);

	/* A loaded module reads it the same way. */
	mod = NULL;
	assert_int_equal(obs_open_module(&mod, CORE_STUB_PATH, at(f, "/m1/sub")), MODULE_SUCCESS);
	expect_id(mod, "sub-id");
	dstr_free(&bin);
}

/* Lays out plugins a, b and bb, loads them with obs_load_plugins() as the
 * type, and checks which are loaded ('l') and which disabled ('d'). */
static void expect_loaded(struct paths_fixture *f, enum obs_runtime_module_type type, const char *expected)
{
	struct dstr bin = {0}, data = {0};
	dstr_copy(&bin, at(f, "/pl"));
	dstr_copy(&data, at(f, "/pd/%module%"));
	struct obs_runtime_module_info info = {
		.path_info = {.binary = bin.array, .data = data.array},
		.type = type,
	};
	struct obs_module_failure_info failures = {0};
	obs_load_plugins(&info, &failures);
	assert_int_equal(failures.count, 0);
	obs_module_failure_info_free(&failures);

	const char *names[] = {"a", "b", "bb"};
	char got[4] = {0};
	for (size_t i = 0; i < 3; i++) {
		bool loaded = obs_get_module(names[i]) != NULL;
		bool off = obs_get_disabled_module(names[i]) != NULL;
		got[i] = loaded && !off ? 'l' : !loaded && off ? 'd' : '?';
	}
	assert_string_equal(got, expected);
	dstr_free(&bin);
	dstr_free(&data);
}

/* The safe list, when it is not empty, holds the only modules that load; the
 * disabled list holds modules that do not. Whole names, case-sensitive. */
static void safe_and_disabled_modules(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	make(f, "/pd/");
	make(f, "/pl/");
	assert_true(copy_file(CORE_STUB_PATH, at(f, "/pl/a.so"), 0755));
	assert_true(copy_file(CORE_STUB_PATH, at(f, "/pl/b.so"), 0755));
	assert_true(copy_file(CORE_STUB_PATH, at(f, "/pl/bb.so"), 0755));

	restart(f, NULL);
	expect_loaded(f, OBS_MODULE_TYPE_PLUGIN, "lll");

	restart(f, NULL);
	obs_add_safe_module("a");
	obs_add_safe_module("bb");
	expect_loaded(f, OBS_MODULE_TYPE_PLUGIN, "ldl");

	restart(f, NULL);
	obs_add_safe_module("A");
	obs_add_safe_module("b ");
	obs_add_safe_module("");
	expect_loaded(f, OBS_MODULE_TYPE_PLUGIN, "ddd");

	restart(f, NULL);
	obs_add_disabled_module("b");
	expect_loaded(f, OBS_MODULE_TYPE_PLUGIN, "ldl");

	restart(f, NULL);
	obs_add_disabled_module("B");
	obs_add_disabled_module("bbb");
	obs_add_disabled_module("a.so");
	obs_add_disabled_module("");
	expect_loaded(f, OBS_MODULE_TYPE_PLUGIN, "lll");

	restart(f, NULL);
	obs_add_safe_module("b");
	obs_add_safe_module("bb");
	obs_add_disabled_module("bb");
	obs_add_disabled_module("bb");
	expect_loaded(f, OBS_MODULE_TYPE_PLUGIN, "dld");

	/* NULL names are not added. */
	restart(f, NULL);
	obs_add_safe_module(NULL);
	obs_add_disabled_module(NULL);
	expect_loaded(f, OBS_MODULE_TYPE_PLUGIN, "lll");

	/* Core modules: the safe list applies, the disabled list does not. */
	restart(f, NULL);
	obs_add_disabled_module("a");
	expect_loaded(f, OBS_MODULE_TYPE_CORE, "lll");

	restart(f, NULL);
	obs_add_safe_module("a");
	expect_loaded(f, OBS_MODULE_TYPE_CORE, "ldd");
}

/* ------------------------------------------------------- module locales */

/*
 * obs_module_load_locale(module, default_locale, locale): the text of
 * "locale/<default_locale>.ini" in the module's data path
 * (obs_find_module_file), with "/locale/<locale>.ini" added on top unless
 * locale is default_locale, ignoring case (obs-rust/obs-studio#1876). NULL
 * when a parameter is NULL or the default file is missing.
 *
 * Characterized, not endorsed:
 * - The second name starts with '/' and the first does not, so after a data
 *   path the second has two: "<data>//locale/<locale>.ini". It still
 *   resolves.
 * - With an empty data path the first name is relative to the working
 *   directory and the second is absolute: "/locale/<locale>.ini".
 * - A locale that differs from the default only in case is never loaded,
 *   even when its own file exists.
 * - A missing locale file keeps the default text.
 */

#define LOCALE_EN "A=\"en A\"\nB=\"en B\"\n"
#define LOCALE_DE "A=\"de A\"\n"
#define LOCALE_EN_CASE "A=\"EN-us A\"\nB=\"EN-us B\"\n"

/* The text of key in lookup, or NULL when it has none. */
static const char *text_of(lookup_t *lookup, const char *key)
{
	const char *out = NULL;
	return text_lookup_getstr(lookup, key, &out) ? out : NULL;
}

/* The lookup has A and B as given (NULL: not there), or is NULL when a is
 * NULL. */
static void expect_locale(obs_module_t *mod, const char *def, const char *locale, const char *a, const char *b)
{
	lookup_t *lookup = obs_module_load_locale(mod, def, locale);
	if (!a) {
		assert_null(lookup);
		return;
	}
	assert_non_null(lookup);
	assert_non_null(text_of(lookup, "A"));
	assert_string_equal(text_of(lookup, "A"), a);
	if (b) {
		assert_non_null(text_of(lookup, "B"));
		assert_string_equal(text_of(lookup, "B"), b);
	} else {
		assert_null(text_of(lookup, "B"));
	}
	text_lookup_destroy(lookup);
}

static void expect_found(struct paths_fixture *f, obs_module_t *mod, const char *file, const char *expected)
{
	char *found = obs_find_module_file(mod, file);
	assert_non_null(found);
	char rel[1024];
	bool was_null;
	copy_rel(rel, sizeof(rel), f->root.array, found, &was_null);
	assert_string_equal(rel, expected);
	bfree(found);
}

static void module_locale(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	write_text(f, "/loc/locale/en-US.ini", LOCALE_EN);
	write_text(f, "/loc/locale/de-DE.ini", LOCALE_DE);
	write_text(f, "/loc/locale/EN-us.ini", LOCALE_EN_CASE);
	write_text(f, "/nodef/locale/de-DE.ini", LOCALE_DE);
	struct dstr bin = {0};
	dstr_copy(&bin, at(f, "/n/stub.so"));
	obs_module_t *mod = disabled_module(bin.array, at(f, "/loc"));

	/* The default only. */
	expect_locale(mod, "en-US", "en-US", "en A", "en B");
	/* Another locale on top of the default. */
	expect_locale(mod, "en-US", "de-DE", "de A", "en B");
	/* The default in another case: not loaded, though its file exists. */
	expect_locale(mod, "en-US", "EN-us", "en A", "en B");
	expect_locale(mod, "EN-us", "en-US", "EN-us A", "EN-us B");
	/* A missing locale keeps the default. */
	expect_locale(mod, "en-US", "fr-FR", "en A", "en B");
	expect_locale(mod, "en-US", "", "en A", "en B");
	/* A missing default: NULL, though the locale exists. */
	expect_locale(mod, "xx-XX", "de-DE", NULL, NULL);
	expect_locale(mod, "", "de-DE", NULL, NULL);
	expect_locale(disabled_module(bin.array, at(f, "/nodef")), "en-US", "de-DE", NULL, NULL);
	expect_locale(disabled_module(bin.array, at(f, "/none")), "en-US", "en-US", NULL, NULL);
	/* The default file is a second locale's file as well. */
	expect_locale(mod, "de-DE", "en-US", "en A", "en B");
	expect_locale(mod, "de-DE", "de-DE", "de A", NULL);

	/* NULL parameters. */
	expect_locale(NULL, "en-US", "en-US", NULL, NULL);
	expect_locale(mod, NULL, "en-US", NULL, NULL);
	expect_locale(mod, "en-US", NULL, NULL, NULL);
	expect_locale(mod, NULL, NULL, NULL, NULL);

	/* The two names as obs_find_module_file() resolves them. */
	expect_found(f, mod, "locale/de-DE.ini", "R/loc/locale/de-DE.ini");
	expect_found(f, mod, "/locale/de-DE.ini", "R/loc//locale/de-DE.ini");

	/* A data path with a trailing '/': the same text. */
	obs_module_t *slash = disabled_module(bin.array, at(f, "/loc/"));
	expect_locale(slash, "en-US", "de-DE", "de A", "en B");
	expect_found(f, slash, "/locale/de-DE.ini", "R/loc//locale/de-DE.ini");

	/* An empty data path: the default from the working directory, the
	 * locale from the root, so the working directory's de-DE is not read.
	 * (Only where the root has no such file.) */
	if (!os_file_exists("/locale/de-DE.ini")) {
		obs_module_t *empty = disabled_module(bin.array, "");
		char cwd[4096];
		assert_non_null(getcwd(cwd, sizeof(cwd)));
		assert_int_equal(chdir(at(f, "/loc")), 0);
		lookup_t *lookup = obs_module_load_locale(empty, "en-US", "de-DE");
		assert_int_equal(chdir(cwd), 0);
		assert_non_null(lookup);
		assert_string_equal(text_of(lookup, "A"), "en A");
		text_lookup_destroy(lookup);
	}

	dstr_free(&bin);
}

/* ------------------------------------------------------ module type lists
 *
 * CORE_STUB_REGISTERS_PATH registers, from its obs_module_load(), one source,
 * output, encoder and service type that libobs accepts and types that fail a
 * check: a composite audio source, a source without get_name, a video source
 * without get_width, an output without start, an encoder without encode and a
 * service without get_protocol. A module names a type only if it registered
 * it: obs_*_get_module() of a rejected id is NULL and its load state is
 * missing.
 */

#define REGSTUB_NAME "core_module_stub_registers"

static obs_module_t *load_registering_stub(struct paths_fixture *f)
{
	obs_module_t *mod = NULL;
	assert_int_equal(obs_open_module(&mod, CORE_STUB_REGISTERS_PATH, at(f, "/nodata")), MODULE_SUCCESS);
	assert_non_null(mod);
	assert_true(obs_init_module(mod));
	assert_ptr_equal(obs_get_module(REGSTUB_NAME), mod);
	return mod;
}

static bool source_type_listed(const char *id)
{
	const char *t;
	for (size_t i = 0; obs_enum_source_types(i, &t); i++) {
		if (strcmp(t, id) == 0)
			return true;
	}
	return false;
}

/* Accepted types: registered, and named by the loading module. */
static void module_lists_accepted_types(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	obs_module_t *mod = load_registering_stub(f);

	assert_true(source_type_listed("regstub_source"));
	assert_ptr_equal(obs_source_get_module("regstub_source"), mod);
	assert_int_equal(obs_source_load_state("regstub_source"), OBS_MODULE_ENABLED);
	assert_ptr_equal(obs_output_get_module("regstub_output"), mod);
	assert_int_equal(obs_output_load_state("regstub_output"), OBS_MODULE_ENABLED);
	assert_ptr_equal(obs_encoder_get_module("regstub_encoder"), mod);
	assert_int_equal(obs_encoder_load_state("regstub_encoder"), OBS_MODULE_ENABLED);
	assert_ptr_equal(obs_service_get_module("regstub_service"), mod);
	assert_int_equal(obs_service_load_state("regstub_service"), OBS_MODULE_ENABLED);
}

static void expect_unowned_source(const char *id)
{
	assert_false(source_type_listed(id));
	assert_null(obs_source_get_module(id));
	assert_int_equal(obs_source_load_state(id), OBS_MODULE_MISSING);
}

/* A source type that fails a check after the duplicate and size checks (the
 * composite and required callback rules) is not registered, and the module
 * that tried does not name it. */
static void module_omits_rejected_sources(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	load_registering_stub(f);

	expect_unowned_source("regstub_composite_audio");
	expect_unowned_source("regstub_no_name");
	expect_unowned_source("regstub_no_width");
}

/* Outputs, encoders and services: a rejected type is not named either. */
static void module_omits_rejected_types(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	load_registering_stub(f);

	assert_null(obs_output_get_display_name("regstub_output_no_start"));
	assert_null(obs_output_get_module("regstub_output_no_start"));
	assert_int_equal(obs_output_load_state("regstub_output_no_start"), OBS_MODULE_MISSING);
	assert_null(obs_encoder_get_display_name("regstub_encoder_no_encode"));
	assert_null(obs_encoder_get_module("regstub_encoder_no_encode"));
	assert_int_equal(obs_encoder_load_state("regstub_encoder_no_encode"), OBS_MODULE_MISSING);
	assert_null(obs_service_get_display_name("regstub_service_no_protocol"));
	assert_null(obs_service_get_module("regstub_service_no_protocol"));
	assert_int_equal(obs_service_load_state("regstub_service_no_protocol"), OBS_MODULE_MISSING);
}

static const char *reg_name(void *type_data)
{
	(void)type_data;
	return "registered later";
}

/* The id a module failed to register, registered later outside any module:
 * a type of no module, not one of the module that was rejected. */
static void rejected_id_registered_later(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	load_registering_stub(f);

	struct obs_source_info info = {0};
	info.id = "regstub_no_name";
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.get_name = reg_name;
	obs_register_source(&info);

	assert_true(source_type_listed("regstub_no_name"));
	assert_null(obs_source_get_module("regstub_no_name"));
	assert_int_equal(obs_source_load_state("regstub_no_name"), OBS_MODULE_MISSING);
}

/* obs_module_add_*() with a module: the id is copied into the module's list
 * (the plugin manager names a disabled module's types this way). Without
 * one: nothing is allocated. */
static void module_add_types(void **state)
{
	struct paths_fixture *f = paths_begin(state);
	obs_module_t *mod = disabled_module(at(f, "/n/added.so"), at(f, "/nodata"));

	char id[] = "regstub_added";
	obs_module_add_source(mod, id);
	obs_module_add_output(mod, id);
	obs_module_add_encoder(mod, id);
	obs_module_add_service(mod, id);
	/* A copy: the caller's string can change. */
	id[0] = 'X';
	assert_ptr_equal(obs_source_get_module("regstub_added"), mod);
	assert_int_equal(obs_source_load_state("regstub_added"), OBS_MODULE_DISABLED);
	assert_ptr_equal(obs_output_get_module("regstub_added"), mod);
	assert_ptr_equal(obs_encoder_get_module("regstub_added"), mod);
	assert_ptr_equal(obs_service_get_module("regstub_added"), mod);
	assert_null(obs_source_get_module("Xegstub_added"));

	/* The allocations each call leaves behind. */
	long left[4], allocs = bnum_allocs();
	obs_module_add_source(NULL, "regstub_null");
	left[0] = bnum_allocs() - allocs;
	obs_module_add_output(NULL, "regstub_null");
	left[1] = bnum_allocs() - allocs - left[0];
	obs_module_add_encoder(NULL, "regstub_null");
	left[2] = bnum_allocs() - allocs - left[0] - left[1];
	obs_module_add_service(NULL, "regstub_null");
	left[3] = bnum_allocs() - allocs - left[0] - left[1] - left[2];
	printf("allocations left by obs_module_add_*(NULL): source %ld, output %ld, encoder %ld, service %ld\n",
	       left[0], left[1], left[2], left[3]);
	for (size_t i = 0; i < 4; i++)
		assert_int_equal(left[i], 0);
	assert_null(obs_source_get_module("regstub_null"));
}

int main(int argc, char **argv)
{
	if (argc > 1 && strcmp(argv[1], "--child") == 0)
		return run_child();

	if (obs_core_modules_count < 2 || obs_core_modules_count > 256) {
		printf("need 2..256 core modules, have %u\n", obs_core_modules_count);
		return SKIP_CODE;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(module_path_type, type_setup, type_teardown),
		cmocka_unit_test(all_present),
		cmocka_unit_test(missing_dependency),
		cmocka_unit_test(missing_file),
		cmocka_unit_test(same_libobs),
		cmocka_unit_test(core_module_paths),
		cmocka_unit_test_setup_teardown(search_flat_directory, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(search_trailing_separators, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(search_module_directories, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(search_repeated_placeholder, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(search_missing_and_empty, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(search_duplicate_paths, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(find_module_file, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(find_module_file_relative, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(load_plugins_data_path, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_names, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_config_path, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_manifest, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(safe_and_disabled_modules, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_locale, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_lists_accepted_types, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_omits_rejected_sources, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_omits_rejected_types, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(rejected_id_registered_later, paths_setup, paths_teardown),
		cmocka_unit_test_setup_teardown(module_add_types, paths_setup, paths_teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
