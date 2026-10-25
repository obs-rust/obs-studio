/* Profile > Import with a folder path that ends in a separator
 * (obs-rust/obs-studio#2295, obsproject/obs-studio#13862).
 *
 * The profile directory name was the selected path's filename(), which is
 * empty for "/x/MyProfile/". The macOS folder dialog returns such a path,
 * and a path typed into Qt's own dialog can end in a separator too. The
 * import then copied the profile's files into the profiles folder itself,
 * not into a folder of their own, and no profile showed up; a second import
 * failed with "File exists".
 *
 * Builds the helpers on_actionImportProfile_triggered() uses
 * (frontend/utility/profile-import.hpp):
 * - the directory name for paths with and without trailing separators, a
 *   Windows path, and a root, which has no name and must be refused;
 * - an import from a path with a trailing separator into a temporary
 *   profiles folder: the files must land in profiles/MyProfile/;
 * - an import with no directory name must copy nothing into the profiles
 *   folder itself.
 *
 * Two profiles with the same [General] Name (obs-rust/obs-studio#2382). The
 * profile list is keyed by name, and the second profile was dropped from it
 * without a word: still on disk, but impossible to select, rename or remove.
 * - importing a profile named like an existing one must give it a name of
 *   its own, and both must be listed under distinct names;
 * - two folders with the same name (made on disk) must both be listed, the
 *   second under a name of its own, whatever order they are found in;
 * - profiles with unique names keep them.
 *
 * QtCore only; no display. */
#include "frontend/utility/profile-import.hpp"

#include <QString>

#include <filesystem>
#include <fstream>
#include <set>
#include <string>
#include <vector>
#include <system_error>

#include <stdarg.h>
#include <stdio.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

namespace fs = std::filesystem;

namespace {

void check_name(const char *selected, const char *expected)
{
	const std::string name = ProfileImportDirectoryName(QString::fromUtf8(selected));
	if (name != expected) {
		fprintf(stderr, "ProfileImportDirectoryName(\"%s\") = \"%s\", expected \"%s\"\n", selected,
			name.c_str(), expected);
		fail();
	}
}

void test_name_trailing_slash(void **)
{
	check_name("/x/MyProfile/", "MyProfile");
}

void test_name_no_trailing_slash(void **)
{
	check_name("/x/MyProfile", "MyProfile");
}

void test_name_space_and_double_slash(void **)
{
	check_name("/x/My Profile//", "My Profile");
}

void test_name_windows_trailing_slash(void **)
{
	check_name("C:/x/P/", "P");
#ifdef _WIN32
	check_name("C:\\x\\P\\", "P");
	check_name("C:/", "");
#endif
}

void test_name_root_refused(void **)
{
	check_name("/", "");
	check_name("/x/..", "");
}

struct TempDir {
	fs::path path;

	TempDir()
	{
		std::error_code ec;
		const fs::path base = fs::temp_directory_path(ec);
		assert_false(static_cast<bool>(ec));
		for (int i = 0; i < 1000; i++) {
			fs::path candidate = base / ("obs-test-profile-import-" + std::to_string(i));
			if (fs::create_directory(candidate, ec)) {
				path = candidate;
				return;
			}
		}
		fail();
	}

	~TempDir()
	{
		std::error_code ec;
		fs::remove_all(path, ec);
	}
};

void write_file(const fs::path &file, const char *contents)
{
	std::ofstream out(file, std::ios::binary);
	out << contents;
	assert_true(out.good());
}

/* A profile to import in <tmp>/source/MyProfile, and an empty profiles
 * folder in <tmp>/profiles. */
void make_source(const TempDir &tmp)
{
	const fs::path source = tmp.path / "source" / "MyProfile";
	assert_true(fs::create_directories(source));
	assert_true(fs::create_directory(tmp.path / "profiles"));
	write_file(source / "basic.ini", "[General]\nName=MyProfile\n");
	write_file(source / "service.json", "{}");
}

void test_import_trailing_slash(void **)
{
	TempDir tmp;
	make_source(tmp);

	const QString selected = QString::fromStdString((tmp.path / "source" / "MyProfile").u8string() + "/");
	const std::string name = ProfileImportDirectoryName(selected);
	const ProfileImportResult result =
		ImportProfileFiles(fs::u8path(selected.toStdString()), tmp.path / "profiles", name);

	assert_int_equal((int)result.status, (int)ProfileImportResult::Status::Imported);
	assert_true(fs::exists(tmp.path / "profiles" / "MyProfile" / "basic.ini"));
	assert_true(fs::exists(tmp.path / "profiles" / "MyProfile" / "service.json"));
	assert_false(fs::exists(tmp.path / "profiles" / "basic.ini"));
	assert_false(fs::exists(tmp.path / "profiles" / "service.json"));
}

void test_import_without_name_refused(void **)
{
	TempDir tmp;
	make_source(tmp);

	const ProfileImportResult result =
		ImportProfileFiles(tmp.path / "source" / "MyProfile", tmp.path / "profiles", std::string());

	assert_int_not_equal((int)result.status, (int)ProfileImportResult::Status::Imported);
	assert_true(fs::is_empty(tmp.path / "profiles"));
}

ProfileListEntry entry(const char *name, const char *directoryName)
{
	return {name, directoryName, fs::u8path(directoryName), {}, {}};
}

const ProfileListEntry *find_directory(const std::vector<ProfileListEntry> &list, const char *directoryName)
{
	for (const ProfileListEntry &profile : list) {
		if (profile.directoryName == directoryName) {
			return &profile;
		}
	}
	fprintf(stderr, "profile folder '%s' is not listed\n", directoryName);
	return nullptr;
}

void check_distinct_names(const std::vector<ProfileListEntry> &list)
{
	std::set<std::string> names;
	for (const ProfileListEntry &profile : list) {
		if (!names.insert(profile.name).second) {
			fprintf(stderr, "two profiles are listed as '%s'\n", profile.name.c_str());
			fail();
		}
	}
}

/* An existing profile "Shared" in profiles/Existing, and a profile in
 * source/Other also named "Shared". The import must give it another name,
 * and both must be listed. */
void test_import_name_clash(void **)
{
	TempDir tmp;
	const fs::path source = tmp.path / "source" / "Other";
	assert_true(fs::create_directories(source));
	assert_true(fs::create_directories(tmp.path / "profiles" / "Existing"));
	write_file(tmp.path / "profiles" / "Existing" / "basic.ini", "[General]\nName=Shared\n");
	write_file(source / "basic.ini", "[General]\nName=Shared\n");

	const std::vector<ProfileListEntry> before = ProfileListEntries({entry("Shared", "Existing")});
	assert_int_equal((int)before.size(), 1);

	const ProfileImportResult result = ImportProfileFiles(source, tmp.path / "profiles", "Other");
	assert_int_equal((int)result.status, (int)ProfileImportResult::Status::Imported);

	const std::string name = ImportedProfileName("Shared", [&before](const std::string &candidate) {
		for (const ProfileListEntry &profile : before) {
			if (profile.name == candidate) {
				return true;
			}
		}
		return false;
	});
	if (name == "Shared" || name.empty()) {
		fprintf(stderr, "imported profile named '%s', clashing with the existing one\n", name.c_str());
		fail();
	}

	const std::vector<ProfileListEntry> after =
		ProfileListEntries({entry("Shared", "Existing"), entry(name.c_str(), "Other")});
	assert_int_equal((int)after.size(), 2);
	check_distinct_names(after);

	const ProfileListEntry *existing = find_directory(after, "Existing");
	const ProfileListEntry *imported = find_directory(after, "Other");
	assert_non_null(existing);
	assert_non_null(imported);
	assert_string_equal(existing->name.c_str(), "Shared");
	assert_string_equal(imported->name.c_str(), name.c_str());
	assert_true(imported->renamedFrom.empty());
}

/* An imported profile with no [General] Name is listed under its folder
 * name, which can be another profile's name too. */
void test_import_folder_name_clash(void **)
{
	const std::string name =
		ImportedProfileName("Shared", [](const std::string &candidate) { return candidate == "Shared"; });
	if (name == "Shared" || name.empty()) {
		fprintf(stderr, "imported profile named '%s', clashing with the existing one\n", name.c_str());
		fail();
	}
}

void check_same_name_folders(const std::vector<ProfileListEntry> &found)
{
	const std::vector<ProfileListEntry> list = ProfileListEntries(found);
	assert_int_equal((int)list.size(), 2);
	check_distinct_names(list);

	const ProfileListEntry *first = find_directory(list, "A");
	const ProfileListEntry *second = find_directory(list, "B");
	assert_non_null(first);
	assert_non_null(second);
	assert_string_equal(first->name.c_str(), "Shared");
	assert_true(first->renamedFrom.empty());
	assert_string_not_equal(second->name.c_str(), "Shared");
	assert_string_equal(second->renamedFrom.c_str(), "Shared");
	assert_string_equal(second->sameNameAs.c_str(), "A");
	assert_true(second->path == fs::u8path("B"));
}

/* Two folders, A and B, both named "Shared" on disk. Both are listed; A, the
 * first by folder name, keeps the name, in either order found. */
void test_list_same_name_folders(void **)
{
	check_same_name_folders({entry("Shared", "A"), entry("Shared", "B")});
	check_same_name_folders({entry("Shared", "B"), entry("Shared", "A")});
}

/* The name given to the second must not be one a third profile has. */
void test_list_same_name_avoids_other_names(void **)
{
	const std::vector<ProfileListEntry> found = {entry("Shared", "A"), entry("Shared", "B"), entry("Shared2", "C")};
	const std::vector<ProfileListEntry> list = ProfileListEntries(found);
	assert_int_equal((int)list.size(), 3);
	check_distinct_names(list);

	const ProfileListEntry *third = find_directory(list, "C");
	assert_non_null(third);
	assert_string_equal(third->name.c_str(), "Shared2");
	assert_true(third->renamedFrom.empty());
}

void test_unique_names_unchanged(void **)
{
	const std::vector<ProfileListEntry> list = ProfileListEntries({entry("Two", "b"), entry("One", "a")});
	assert_int_equal((int)list.size(), 2);

	const ProfileListEntry *one = find_directory(list, "a");
	const ProfileListEntry *two = find_directory(list, "b");
	assert_non_null(one);
	assert_non_null(two);
	assert_string_equal(one->name.c_str(), "One");
	assert_string_equal(two->name.c_str(), "Two");
	assert_true(one->renamedFrom.empty());
	assert_true(two->renamedFrom.empty());

	const std::string name =
		ImportedProfileName("Three", [](const std::string &candidate) { return candidate == "One"; });
	assert_string_equal(name.c_str(), "Three");
}

} // namespace

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_name_trailing_slash),
		cmocka_unit_test(test_name_no_trailing_slash),
		cmocka_unit_test(test_name_space_and_double_slash),
		cmocka_unit_test(test_name_windows_trailing_slash),
		cmocka_unit_test(test_name_root_refused),
		cmocka_unit_test(test_import_trailing_slash),
		cmocka_unit_test(test_import_without_name_refused),
		cmocka_unit_test(test_import_name_clash),
		cmocka_unit_test(test_import_folder_name_clash),
		cmocka_unit_test(test_list_same_name_folders),
		cmocka_unit_test(test_list_same_name_avoids_other_names),
		cmocka_unit_test(test_unique_names_unchanged),
	};

	return cmocka_run_group_tests_name("profile_import", tests, NULL, NULL);
}
