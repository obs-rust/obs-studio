#pragma once

/* Profile > Import (obs-rust/obs-studio#2295). Header-only, QtCore and
 * std::filesystem only, so it can be unit tested without the frontend. */

#include <QString>

#include <array>
#include <filesystem>
#include <string>
#include <utility>

/* The profile directory name for the folder chosen in Profile > Import: the
 * folder's own name. */
inline std::string ProfileImportDirectoryName(const QString &sourceDirectory)
{
	const std::filesystem::path sourcePath = std::filesystem::u8path(sourceDirectory.toStdString());
	return sourcePath.filename().u8string();
}

struct ProfileImportResult {
	enum class Status { Imported, CreateFailed, MissingFile, CopyFailed };

	Status status = Status::Imported;
	/* The file that was missing or failed to copy. */
	std::string file;
	/* The filesystem error, for CreateFailed and CopyFailed. */
	std::string error;
};

/* Creates the profile directory `directoryName` in `profilesDirectory` and
 * copies the profile files from `source` into it. basic.ini must exist; the
 * others are optional. */
inline ProfileImportResult ImportProfileFiles(const std::filesystem::path &source,
					      const std::filesystem::path &profilesDirectory,
					      const std::string &directoryName)
{
	ProfileImportResult result;

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
