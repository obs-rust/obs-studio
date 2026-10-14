#pragma once

#include <filesystem>
#include <functional>
#include <utility>
#include <vector>

enum class ConfigMigrationResult { NoSource, Existing, Imported };

// The caller holds the frontend migration lock. Overrides pair an original
// settings path with its relative destination inside the independent copy.
inline ConfigMigrationResult MigrateAppConfig(
	const std::filesystem::path &, const std::filesystem::path &,
	const std::vector<std::pair<std::filesystem::path, std::filesystem::path>> &,
	const std::function<void(const std::filesystem::path &)> &)
{
	return ConfigMigrationResult::Existing;
}
