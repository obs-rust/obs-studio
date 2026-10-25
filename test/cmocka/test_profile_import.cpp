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
 * QtCore only; no display. */
#include "frontend/utility/profile-import.hpp"

#include <QString>

#include <filesystem>
#include <fstream>
#include <string>
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

} // namespace

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_name_trailing_slash),         cmocka_unit_test(test_name_no_trailing_slash),
		cmocka_unit_test(test_name_space_and_double_slash), cmocka_unit_test(test_name_windows_trailing_slash),
		cmocka_unit_test(test_name_root_refused),           cmocka_unit_test(test_import_trailing_slash),
		cmocka_unit_test(test_import_without_name_refused),
	};

	return cmocka_run_group_tests_name("profile_import", tests, NULL, NULL);
}
