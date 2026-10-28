/******************************************************************************
    Copyright (C) 2025 by Patrick Heyer <PatTheMav@users.noreply.github.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "frontend/build/frontend-utility.pch.hpp"

using json = nlohmann::json;
using CrashLogUpdateResult = OBS::CrashHandler::CrashLogUpdateResult;

namespace {

constexpr std::string_view crashSentinelPath = "obs-studio/.sentinel";
constexpr std::string_view crashSentinelPrefix = "run_";

#ifndef NDEBUG
constexpr bool isSentinelEnabled = false;
#else
constexpr bool isSentinelEnabled = true;
#endif

} // namespace

namespace OBS {

static_assert(!crashSentinelPath.empty(), "Crash sentinel path name cannot be empty");
static_assert(!crashSentinelPrefix.empty(), "Crash sentinel filename prefix cannot be empty");

CrashHandler::CrashHandler(QUuid appLaunchUUID) : appLaunchUUID_(appLaunchUUID)
{
	if (!isSentinelEnabled) {
		return;
	}

	setupSentinel();
	checkCrashState();
}

CrashHandler::~CrashHandler()
{
	if (isActiveCrashHandler_) {
		applicationShutdownHandler();
	}
}

CrashHandler::CrashHandler(CrashHandler &&other) noexcept
{
	crashLogUploadThread_ = std::exchange(other.crashLogUploadThread_, nullptr);
	isActiveCrashHandler_ = true;
	other.isActiveCrashHandler_ = false;
}

CrashHandler &CrashHandler::operator=(CrashHandler &&other) noexcept
{
	std::swap(crashLogUploadThread_, other.crashLogUploadThread_);
	isActiveCrashHandler_ = true;
	other.isActiveCrashHandler_ = false;
	return *this;
}

bool CrashHandler::hasUncleanShutdown()
{
	return isSentinelEnabled && isSentinelPresent_;
}

bool CrashHandler::hasNewCrashLog()
{
	CrashLogUpdateResult result = updateLocalCrashLogState();

	if (result == CrashLogUpdateResult::NotAvailable) {
		return false;
	}

	bool hasNewCrashLog = (result == CrashLogUpdateResult::Updated);
	bool hasNoLogUrl = lastCrashLogURL_.empty();

	return (hasNewCrashLog || hasNoLogUrl);
}

CrashLogUpdateResult CrashHandler::updateLocalCrashLogState()
{
	updateCrashLogFromConfig();

	std::filesystem::path lastLocalCrashLogFile = findLastCrashLog();

	if (lastLocalCrashLogFile.empty() && lastCrashLogFile_.empty()) {
		return CrashLogUpdateResult::NotAvailable;
	}

	if (lastLocalCrashLogFile != lastCrashLogFile_) {
		lastCrashLogFile_ = std::move(lastLocalCrashLogFile);
		lastCrashLogFileName_ = lastCrashLogFile_.filename().u8string();
		lastCrashLogURL_.clear();

		return CrashLogUpdateResult::Updated;
	} else {
		return CrashLogUpdateResult::NotUpdated;
	}
}

void CrashHandler::uploadLastCrashLog()
{
	bool newCrashDetected = hasNewCrashLog();

	if (newCrashDetected) {
		uploadCrashLogToServer();
	} else {
		handleExistingCrashLogUpload();
	}
}

void CrashHandler::checkCrashState()
{
	bool currentSentinelFound = false;

	std::filesystem::path sentinel_dir = crashSentinelFile_.parent_path();

	if (!ensure_crash_sentinel_dir(sentinel_dir)) {
		return;
	}

	for (const auto &entry : std::filesystem::directory_iterator(sentinel_dir)) {
		if (entry.is_directory()) {
			continue;
		}

		std::string entryFileName = entry.path().filename().u8string();

		if (entryFileName.rfind(crashSentinelPrefix.data(), 0) != 0) {
			continue;
		}

		if (entry.path().filename() == crashSentinelFile_.filename()) {
			currentSentinelFound = true;
			continue;
		}

		isSentinelPresent_ = true;
	}

	if (!currentSentinelFound) {
		std::fstream filestream;
		filestream.open(crashSentinelFile_, std::ios::out);
		filestream.close();
	}
}

void CrashHandler::setupSentinel()
{
	BPtr crashSentinelPathString = GetAppConfigPathPtr(crashSentinelPath.data());
	std::string appLaunchUUIDString = appLaunchUUID_.toString(QUuid::WithoutBraces).toStdString();

	std::string crashSentinelFilePath = crashSentinelPathString.Get();
	crashSentinelFilePath.reserve(crashSentinelFilePath.size() + crashSentinelPrefix.size() +
				      appLaunchUUIDString.size() + 1);
	crashSentinelFilePath.append("/").append(crashSentinelPrefix).append(appLaunchUUIDString);

	crashSentinelFile_ = std::filesystem::u8path(crashSentinelFilePath);

	isActiveCrashHandler_ = true;
}

void CrashHandler::updateCrashLogFromConfig()
{
	config_t *appConfig = App()->GetAppConfig();

	if (!appConfig) {
		return;
	}

	const char *last_crash_log_file = config_get_string(appConfig, "CrashHandler", "last_crash_log_file");
	const char *last_crash_log_url = config_get_string(appConfig, "CrashHandler", "last_crash_log_url");

	std::string lastCrashLogFilePath = last_crash_log_file ? last_crash_log_file : "";
	int64_t lastCrashLogUploadTimestamp = config_get_int(appConfig, "CrashHandler", "last_crash_log_time");
	std::string lastCrashLogUploadURL = last_crash_log_url ? last_crash_log_url : "";

	OBS::Clock::duration durationSinceEpoch = std::chrono::seconds(lastCrashLogUploadTimestamp);
	OBS::TimePoint lastCrashLogUploadTime = OBS::TimePoint(durationSinceEpoch);

	lastCrashLogFile_ = std::filesystem::u8path(lastCrashLogFilePath);
	lastCrashLogFileName_ = lastCrashLogFile_.filename().u8string();
	lastCrashLogURL_ = lastCrashLogUploadURL;
	lastCrashUploadTime_ = lastCrashLogUploadTime;
}

void CrashHandler::saveCrashLogToConfig()
{
	config_t *appConfig = App()->GetAppConfig();

	if (!appConfig) {
		return;
	}

	std::time_t uploadTimePoint_c = OBS::Clock::to_time_t(lastCrashUploadTime_);

	config_set_string(appConfig, "CrashHandler", "last_crash_log_file", lastCrashLogFile_.u8string().c_str());
	config_set_int(appConfig, "CrashHandler", "last_crash_log_time", uploadTimePoint_c);
	config_set_string(appConfig, "CrashHandler", "last_crash_log_url", lastCrashLogURL_.c_str());
	config_save_safe(appConfig, "tmp", nullptr);
}

/* obs-rust runs no log upload service (#1731): the crash log stays on the
 * user's computer. Hand its path to the dialog; OBSApp opens its folder. */
void CrashHandler::uploadCrashLogToServer()
{
	if (lastCrashLogFile_.empty() || !std::filesystem::exists(lastCrashLogFile_)) {
		blog(LOG_WARNING, "Most recent crash log file was empty or unavailable for reading");
		emit crashLogUploadFailed(QTStr("LogUploadDialog.Errors.NoLogFile"));
		return;
	}

	emit crashLogUploadFinished(QString::fromStdString(lastCrashLogFile_.u8string()));
}

void CrashHandler::handleExistingCrashLogUpload()
{
	uploadCrashLogToServer();
}

void CrashHandler::crashLogUploadResultHandler(const std::string &uploadResult, const std::string &error)
{
	if (uploadResult.empty()) {
		emit crashLogUploadFailed(QString::fromStdString(error));
		return;
	}

	json uploadResultData = json::parse(uploadResult);

	try {
		std::string crashLogUrl = uploadResultData["url"];

		lastCrashLogURL_ = crashLogUrl;

		saveCrashLogToConfig();

		const QString crashLogUrlString = QString::fromStdString(crashLogUrl);

		emit crashLogUploadFinished(crashLogUrlString);
	} catch (const json::exception &error) {
		blog(LOG_ERROR, "JSON error while parsing crash upload response:\n%s", error.what());

		const QString jsonErrorMessage = QTStr("CrashHandling.Errors.UploadJSONError");
		emit crashLogUploadFailed(jsonErrorMessage);
	}
}

void CrashHandler::applicationShutdownHandler() noexcept
{
	if (!isSentinelEnabled) {
		return;
	}

	if (crashSentinelFile_.empty()) {
		blog(LOG_ERROR, "No crash sentinel location set for crash handler");
		return;
	}

	const std::filesystem::path sentinel_dir = crashSentinelFile_.parent_path();

	if (!std::filesystem::exists(sentinel_dir)) {
		blog(LOG_ERROR, "Crash sentinel location '%s' does not exist", sentinel_dir.u8string().c_str());
		return;
	}

	for (const auto &entry : std::filesystem::directory_iterator(sentinel_dir)) {
		if (entry.is_directory()) {
			continue;
		}

		std::string entryFileName = entry.path().filename().u8string();

		if (entryFileName.rfind(crashSentinelPrefix.data(), 0) != 0) {
			continue;
		}

		try {
			std::filesystem::remove(entry.path());
		} catch (const std::filesystem::filesystem_error &error) {
			blog(LOG_ERROR, "Failed to delete crash sentinel file:\n%s", error.what());
		}
	}

	isActiveCrashHandler_ = false;
}
} // namespace OBS
