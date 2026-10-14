#include <utility/ConfigMigration.hpp>

#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>

namespace fs = std::filesystem;

static void write(const fs::path &path, const std::string &text)
{
	fs::create_directories(path.parent_path());
	std::ofstream(path) << text;
}

static std::string read(const fs::path &path)
{
	std::ifstream stream(path);
	return std::string(std::istreambuf_iterator<char>(stream), {});
}

static void require(bool condition, const char *message)
{
	if (!condition)
		throw std::runtime_error(message);
}

int main(int argc, char **argv)
{
	if (argc != 3)
		return 2;
	const fs::path root(argv[1]);
	const std::string mode(argv[2]);
	const auto source = root / "obs-studio";
	const auto destination = root / "obs-studio-rust";
	try {
		if (mode != "absent") {
			write(source / "global.ini", "upstream global");
			write(source / "user.ini", "upstream user");
			write(source / "basic/scenes/original.json", "upstream scene");
		}
		if (mode == "existing")
			write(destination / "global.ini", "existing Rust");
		const auto custom = root / "custom/obs-studio/basic/scenes";
		if (mode == "custom")
			write(custom / "chosen.json", "custom scene");
		std::vector<std::pair<fs::path, fs::path>> overrides;
		if (mode == "custom" || mode == "empty-custom")
			overrides.emplace_back(custom, "basic/scenes");
		bool finalized = false;
		bool failed = false;
		ConfigMigrationResult result = ConfigMigrationResult::NoSource;
		try {
			result = MigrateAppConfig(source, destination, overrides, [&](const fs::path &copy) {
				finalized = true;
				require(read(copy / "global.ini") == "upstream global", "global settings missing");
				if (mode == "failure")
					throw std::runtime_error("injected finalization failure");
				write(copy / "global.ini", "isolated Locations");
			});
		} catch (const std::runtime_error &) {
			failed = true;
		}
		if (mode == "failure") {
			require(failed && finalized, "finalization failure was not propagated");
			require(!fs::exists(destination), "failed migration exposed partial config");
		} else if (mode == "absent") {
			require(result == ConfigMigrationResult::NoSource && !finalized, "absent source imported");
			require(!fs::exists(destination), "absent source created config");
		} else if (mode == "existing") {
			require(result == ConfigMigrationResult::Existing && !finalized, "existing config imported");
			require(read(destination / "global.ini") == "existing Rust", "existing Rust overwritten");
		} else {
			require(result == ConfigMigrationResult::Imported && finalized, "first run did not import");
			require(read(destination / "user.ini") == "upstream user", "user settings missing");
			if (mode == "custom") {
				require(read(destination / "basic/scenes/chosen.json") == "custom scene", "custom root ignored");
				require(!fs::exists(destination / "basic/scenes/original.json"), "unselected scenes retained");
			} else if (mode == "empty-custom") {
				require(!fs::exists(destination / "basic/scenes"), "empty custom root mixed with defaults");
			} else {
				require(read(destination / "basic/scenes/original.json") == "upstream scene", "scene missing");
			}
			write(destination / "user.ini", "Rust changed");
		}
		if (mode != "absent") {
			require(read(source / "global.ini") == "upstream global", "upstream global changed");
			require(read(source / "user.ini") == "upstream user", "upstream user changed");
		}
	} catch (const std::exception &error) {
		std::cerr << error.what() << '\n';
		return 1;
	}
}
