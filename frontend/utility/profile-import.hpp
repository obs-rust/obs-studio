#pragma once

/* Profile > Import (obs-rust/obs-studio#2295) and the names profiles are
 * listed under (obs-rust/obs-studio#2382). Header-only, QtCore and
 * std::filesystem only, so it can be unit tested without the frontend. */

#include <QDir>
#include <QString>

#include <algorithm>
#include <array>
#include <filesystem>
#include <map>
#include <set>
#include <string>
#include <utility>
#include <vector>

/* The profile directory name for the folder chosen in Profile > Import: the
 * folder's own name. A folder dialog can return the path with a trailing
 * separator (the macOS one does), and filename() of that is empty, so clean
 * the path first (obsproject/obs-studio#13862). Empty when the path has no
 * folder name, such as a root ("/", "C:/"): there is nothing to import into,
 * and an empty name would put the files in the profiles folder itself. */
inline std::string ProfileImportDirectoryName(const QString &sourceDirectory)
{
	const std::filesystem::path sourcePath =
		std::filesystem::u8path(QDir::cleanPath(sourceDirectory).toStdString());
	std::string name = sourcePath.filename().u8string();
	if (name == "." || name == "..") {
		return {};
	}
	return name;
}

struct ProfileImportResult {
	enum class Status { Imported, NoDirectoryName, CreateFailed, MissingFile, CopyFailed };

	Status status = Status::Imported;
	/* The file that was missing or failed to copy. */
	std::string file;
	/* The filesystem error, for CreateFailed and CopyFailed. */
	std::string error;
};

/* Creates the profile directory `directoryName` in `profilesDirectory` and
 * copies the profile files from `source` into it. basic.ini must exist; the
 * others are optional. An empty `directoryName` is refused (NoDirectoryName)
 * and nothing is created or copied. */
inline ProfileImportResult ImportProfileFiles(const std::filesystem::path &source,
					      const std::filesystem::path &profilesDirectory,
					      const std::string &directoryName)
{
	ProfileImportResult result;

	if (directoryName.empty()) {
		result.status = ProfileImportResult::Status::NoDirectoryName;
		return result;
	}

	const std::filesystem::path destination = profilesDirectory / std::filesystem::u8path(directoryName);

	try {
		std::filesystem::create_directory(destination);
	} catch (const std::filesystem::filesystem_error &error) {
		result.status = ProfileImportResult::Status::CreateFailed;
		result.error = error.what();
		return result;
	}

	const std::array<std::pair<std::string, bool>, 4> profileFiles{{
		{"basic.ini", true},
		{"service.json", false},
		{"streamEncoder.json", false},
		{"recordEncoder.json", false},
	}};

	for (auto &[file, isMandatory] : profileFiles) {
		const std::filesystem::path sourceFile = source / std::filesystem::u8path(file);

		if (!std::filesystem::exists(sourceFile)) {
			if (isMandatory) {
				result.status = ProfileImportResult::Status::MissingFile;
				result.file = file;
				return result;
			}
			continue;
		}

		const std::filesystem::path destinationFile = destination / std::filesystem::u8path(file);

		try {
			std::filesystem::copy(sourceFile, destinationFile);
		} catch (const std::filesystem::filesystem_error &error) {
			result.status = ProfileImportResult::Status::CopyFailed;
			result.file = file;
			result.error = error.what();
			return result;
		}
	}

	return result;
}

/* A profile found in the profiles folder. `name` is the name it is listed
 * under: the [General] Name in its basic.ini, or its folder name when that
 * has none. */
struct ProfileListEntry {
	std::string name;
	std::string directoryName;
	std::filesystem::path path;
	/* Set when the profile is listed under another name than its own,
	 * because a profile in folder `sameNameAs` has `renamedFrom` too. */
	std::string renamedFrom;
	std::string sameNameAs;
};

/* `name`, or, when isUsed(name), the first of name + "2", name + "3", ...
 * that is not used: the rule CreateProfile() uses to give a new profile a
 * folder of its own (GetClosestUnusedFileName()). */
template<typename IsUsed> inline std::string UniqueProfileName(const std::string &name, IsUsed &&isUsed)
{
	if (!isUsed(name)) {
		return name;
	}

	std::string candidate;
	int index = 1;

	do {
		candidate = name + std::to_string(++index);
	} while (isUsed(candidate));

	return candidate;
}

/* The profiles to list, from those found on disk. The list is keyed by
 * name, so when profiles share a name, the first by folder name keeps it,
 * and each other one is listed under a unique name (UniqueProfileName()),
 * with renamedFrom and sameNameAs set. Names nothing clashes with are kept
 * (obs-rust/obs-studio#2382). */
inline std::vector<ProfileListEntry> ProfileListEntries(std::vector<ProfileListEntry> found)
{
	std::sort(found.begin(), found.end(), [](const ProfileListEntry &lhs, const ProfileListEntry &rhs) {
		return lhs.directoryName < rhs.directoryName;
	});

	std::set<std::string> foundNames;
	for (const ProfileListEntry &entry : found) {
		foundNames.insert(entry.name);
	}

	std::map<std::string, std::string> listedNames;

	for (ProfileListEntry &entry : found) {
		const auto listed = listedNames.find(entry.name);

		if (listed != listedNames.end()) {
			entry.renamedFrom = entry.name;
			entry.sameNameAs = listed->second;
			entry.name = UniqueProfileName(entry.name, [&](const std::string &candidate) {
				return foundNames.count(candidate) > 0 || listedNames.count(candidate) > 0;
			});
		}

		listedNames.emplace(entry.name, entry.directoryName);
	}

	return found;
}

/* The name to list an imported profile under. `name` is its [General] Name,
 * or its folder name when that has none; isUsed(n) is true when a profile
 * is listed under n already. A name that is taken is made unique
 * (UniqueProfileName()), so the imported profile is not hidden behind the
 * existing one (obs-rust/obs-studio#2382). */
template<typename IsUsed> inline std::string ImportedProfileName(const std::string &name, IsUsed &&isUsed)
{
	return UniqueProfileName(name, isUsed);
}
