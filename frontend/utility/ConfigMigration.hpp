#pragma once

#include <filesystem>
#include <functional>
#include <set>
#include <stdexcept>
#include <utility>
#include <vector>

enum class ConfigMigrationResult { NoSource, Existing, Imported };

inline void CopyConfigSnapshot(const std::filesystem::path &source, const std::filesystem::path &destination,
			       std::set<std::filesystem::path> &active)
{
	namespace fs = std::filesystem;
	const auto canonical = fs::canonical(source);
	const auto target = fs::weakly_canonical(destination);
	const auto relative = target.lexically_relative(canonical);
	if (!relative.empty() && *relative.begin() != "..") {
		throw std::runtime_error("Configuration copy destination is inside its source");
	}
	if (fs::is_directory(source)) {
		if (!active.insert(canonical).second) {
			throw std::runtime_error("Configuration contains a directory symlink cycle");
		}
		fs::create_directories(destination);
		for (const auto &entry : fs::directory_iterator(source)) {
			CopyConfigSnapshot(entry.path(), destination / entry.path().filename(), active);
		}
		active.erase(canonical);
	} else if (fs::is_regular_file(source)) {
		fs::create_directories(destination.parent_path());
		fs::copy_file(source, destination, fs::copy_options::overwrite_existing);
	} else {
		throw std::runtime_error("Configuration contains a file that cannot be safely copied");
	}
}

// The caller holds the frontend migration lock. Overrides pair an original
// settings path with its relative destination inside the independent copy.
inline ConfigMigrationResult
MigrateAppConfig(const std::filesystem::path &source, const std::filesystem::path &destination,
		 const std::vector<std::pair<std::filesystem::path, std::filesystem::path>> &overrides,
		 const std::function<void(const std::filesystem::path &)> &finalize)
{
	namespace fs = std::filesystem;
	if (fs::symlink_status(destination).type() != fs::file_type::not_found) {
		return ConfigMigrationResult::Existing;
	}
	if (!fs::exists(source)) {
		return ConfigMigrationResult::NoSource;
	}
	const auto staging = destination.parent_path() / (destination.filename().string() + ".migration");
	fs::create_directories(destination.parent_path());
	if (!fs::create_directory(staging)) {
		throw std::runtime_error("An unfinished configuration migration already exists");
	}
	struct Cleanup {
		fs::path path;
		~Cleanup()
		{
			std::error_code error;
			fs::remove_all(path, error);
		}
	} cleanup{staging};
	std::set<fs::path> active;
	CopyConfigSnapshot(source, staging, active);
	for (const auto &[original, relative] : overrides) {
		if (relative.empty() || relative.is_absolute()) {
			throw std::runtime_error("Configuration override must be a relative settings path");
		}
		for (const auto &part : relative) {
			if (part == "..") {
				throw std::runtime_error("Configuration override escapes the settings directory");
			}
		}
		fs::remove_all(staging / relative);
		if (fs::exists(original)) {
			CopyConfigSnapshot(original, staging / relative, active);
		}
	}
	finalize(staging);
	fs::rename(staging, destination);
	return ConfigMigrationResult::Imported;
}
