/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

/* Windows hooks for obs-main (#865): log lines go to an attached debugger,
 * OBS_DEBUGBREAK_ON_ERROR builds break into it on errors, and startup
 * handles wide paths, the run-once mutex, Wine and the app identity.
 * No include guard: this file defines nothing off Windows, where obs-main
 * uses its portable defaults. */
#ifdef _WIN32
#define OBS_MAIN_HOST_WINDOWS 1

#include <shellapi.h>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <QGuiApplication>
#include <QMessageBox>
#include <QPushButton>

#include <fstream>
#include <mutex>
#include <string>

/* Portable mode exists on Windows; its marker files sit two levels up. */
#define ALLOW_PORTABLE_MODE 1

/* frontend/utility/obs-main-host-windows.cpp.hpp */
#define OBS_MAIN_HOST_EARLY_INIT 1
bool obs_main_host_early_init();
void obs_main_host_show_help(const std::string &help);
void obs_main_host_shutdown();
void delete_oldest_file(bool has_prefix, const char *location);

static inline void obs_main_host_log_debugger(const char *str)
{
	if (!IsDebuggerPresent()) {
		return;
	}

	int wNum = MultiByteToWideChar(CP_UTF8, 0, str, -1, NULL, 0);
	if (wNum > 1) {
		static std::wstring wide_buf;
		static std::mutex wide_mutex;

		std::lock_guard<std::mutex> lock(wide_mutex);
		wide_buf.reserve(static_cast<size_t>(wNum + 1));
		wide_buf.resize(static_cast<size_t>(wNum - 1));
		MultiByteToWideChar(CP_UTF8, 0, str, -1, &wide_buf[0], wNum);
		wide_buf.push_back('\n');

		OutputDebugStringW(wide_buf.c_str());
	}
}

static inline void obs_main_host_log_break(int log_level)
{
#ifdef OBS_DEBUGBREAK_ON_ERROR
	if (log_level <= LOG_ERROR && IsDebuggerPresent()) {
		__debugbreak();
	}
#else
	(void)log_level;
#endif
}

static inline void obs_main_host_localtime(struct tm *out, const time_t *now)
{
	localtime_s(out, now);
}

static inline void obs_main_host_open_log(std::fstream &logFile, const char *path)
{
	BPtr<wchar_t> wpath;
	os_utf8_to_wcs_ptr(path, 0, &wpath);
	logFile.open(wpath, std::ios_base::in | std::ios_base::out | std::ios_base::trunc);
}

#define OBS_MAIN_HOST_PRE_APP 1
static inline bool obs_main_host_pre_app()
{
	QGuiApplication::setHighDpiScaleFactorRoundingPolicy(Qt::HighDpiScaleFactorRoundingPolicy::PassThrough);
	return true;
}

static inline RunOnceMutex obs_main_host_check_running(bool &already_running)
{
	return CheckIfAlreadyRunning(already_running);
}

#define OBS_MAIN_HOST_REFUSE_LAUNCH 1
static inline bool obs_main_host_refuse_launch()
{
	if (!IsRunningOnWine()) {
		return false;
	}

	QMessageBox mb(QMessageBox::Question, QTStr("Wine.Title"), QTStr("Wine.Text"));
	mb.setTextFormat(Qt::RichText);
	mb.addButton(QTStr("AlreadyRunning.LaunchAnyway"), QMessageBox::AcceptRole);
	QPushButton *closeButton = mb.addButton(QMessageBox::Close);
	mb.setDefaultButton(closeButton);

	mb.exec();
	return mb.clickedButton() == closeButton;
}

#define OBS_MAIN_HOST_VERIFY_STARTUP 1
static inline void obs_main_host_verify_startup()
{
#ifdef OBS_RUST_VERSION
	// Read back the process identity after the persistent logger is ready.
	if (!VerifyAppIdentity()) {
		throw "Windows application identity changed during startup";
	}
#endif
}
#endif
