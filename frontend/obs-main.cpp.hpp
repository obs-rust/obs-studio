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

#include "frontend/build/frontend-app-obs-main.pch.hpp"

using namespace std;

static log_handler_t def_log_handler;

extern string currentLogFile;
extern string lastLogFile;

bool portable_mode = false;
bool steam = false;
bool safe_mode = false;
bool disable_3p_plugins = false;
bool disable_shutdown_check = false;
static bool unclean_shutdown = false;
bool multi = false;
static bool log_verbose = false;
static bool unfiltered_log = false;
bool opt_start_streaming = false;
bool opt_start_recording = false;
bool opt_studio_mode = false;
bool opt_start_replaybuffer = false;
bool opt_start_virtualcam = false;
bool opt_minimize_tray = false;
bool opt_allow_opengl = false;
bool opt_always_on_top = false;
bool opt_disable_updater = false;
bool opt_disable_missing_files_check = false;
string opt_starting_collection;
string opt_starting_profile;
string opt_starting_scene;

bool restart = false;
bool restart_safe = false;
static QStringList arguments;

#ifndef OBS_MAIN_HOST_WINDOWS
/* Portable startup defaults; the host headers in the PCH override them (#865). */
static inline void obs_main_host_localtime(struct tm *out, const time_t *now)
{
	localtime_r(now, out);
}

static inline void obs_main_host_open_log(fstream &logFile, const char *path)
{
	logFile.open(path, ios_base::in | ios_base::out | ios_base::trunc);
}

static inline int obs_main_host_check_running(bool &already_running)
{
	CheckIfAlreadyRunning(already_running);
	return 0;
}
#endif
#ifndef OBS_MAIN_HOST_EARLY_INIT
static inline bool obs_main_host_early_init()
{
	return true;
}

static inline void obs_main_host_show_help(const string &help)
{
	std::cout << help << "--version, -V: Get current version.\n";
}

static inline void obs_main_host_shutdown() {}
#endif
#ifndef OBS_MAIN_HOST_PRE_APP
static inline bool obs_main_host_pre_app()
{
	return true;
}
#endif
#ifndef OBS_MAIN_HOST_REFUSE_LAUNCH
static inline bool obs_main_host_refuse_launch()
{
	return false;
}
#endif
#ifndef OBS_MAIN_HOST_VERIFY_STARTUP
static inline void obs_main_host_verify_startup() {}
#endif
#ifndef OBS_MAIN_HOST_PERMISSIONS
static inline void obs_main_host_permissions() {}
#endif

string CurrentTimeString()
{
	using namespace std::chrono;

	struct tm tstruct;
	char buf[80];

	auto tp = system_clock::now();
	auto now = system_clock::to_time_t(tp);
	obs_main_host_localtime(&tstruct, &now);

	size_t written = strftime(buf, sizeof(buf), "%T", &tstruct);
	if (ratio_less<system_clock::period, seconds::period>::value && written && (sizeof(buf) - written) > 5) {
		auto tp_secs = time_point_cast<seconds>(tp);
		auto millis = duration_cast<milliseconds>(tp - tp_secs).count();

		snprintf(buf + written, sizeof(buf) - written, ".%03u", static_cast<unsigned>(millis));
	}

	return buf;
}

static void LogString(fstream &logFile, const char *timeString, char *str, int log_level)
{
	static mutex logfile_mutex;
	string msg;
	msg += timeString;
	msg += str;

	logfile_mutex.lock();
	logFile << msg << endl;
	logfile_mutex.unlock();

	QMetaObject::invokeMethod(App(), &OBSApp::addLogLine, Qt::QueuedConnection, log_level, QString(msg.c_str()));
}

static inline void LogStringChunk(fstream &logFile, char *str, int log_level)
{
	char *nextLine = str;
	string timeString = CurrentTimeString();
	timeString += ": ";

	while (*nextLine) {
		char *lineEnd = strchr(str, '\n');
		if (!lineEnd) {
			break;
		}

		if (lineEnd != str && lineEnd[-1] == '\r') {
			lineEnd[-1] = 0;
		} else {
			lineEnd[0] = 0;
		}

		LogString(logFile, timeString.c_str(), str, log_level);
		lineEnd++;
		str = lineEnd;
	}

	LogString(logFile, timeString.c_str(), str, log_level);
}

#define MAX_REPEATED_LINES 30
#define MAX_CHAR_VARIATION (255 * 3)

static inline int sum_chars(const char *str)
{
	int val = 0;
	for (; *str != 0; str++) {
		val += *str;
	}

	return val;
}

static inline bool too_many_repeated_entries(fstream &logFile, const char *msg, const char *output_str)
{
	static mutex log_mutex;
	static const char *last_msg_ptr = nullptr;
	static int last_char_sum = 0;
	static int rep_count = 0;

	int new_sum = sum_chars(output_str);

	lock_guard<mutex> guard(log_mutex);

	if (unfiltered_log) {
		return false;
	}

	if (last_msg_ptr == msg) {
		int diff = std::abs(new_sum - last_char_sum);
		if (diff < MAX_CHAR_VARIATION) {
			return (rep_count++ >= MAX_REPEATED_LINES);
		}
	}

	if (rep_count > MAX_REPEATED_LINES) {
		logFile << CurrentTimeString() << ": Last log entry repeated for "
			<< to_string(rep_count - MAX_REPEATED_LINES) << " more lines" << endl;
	}

	last_msg_ptr = msg;
	last_char_sum = new_sum;
	rep_count = 0;

	return false;
}

#ifndef OBS_MAIN_HOST_WINDOWS
/* Elsewhere log lines are echoed to the console (#865). */
#define OBS_MAIN_LOG_ECHO 1
static inline void obs_main_host_log_debugger(const char *) {}
static inline void obs_main_host_log_break(int) {}
#else
#define OBS_MAIN_LOG_ECHO 0
#endif

#ifdef _DEBUG
#define OBS_MAIN_LOG_ECHO_ALL OBS_MAIN_LOG_ECHO
#else
#define OBS_MAIN_LOG_ECHO_ALL 0
#endif

static void do_log(int log_level, const char *msg, va_list args, void *param)
{
	fstream &logFile = *static_cast<fstream *>(param);
	char str[8192];

	va_list args2;
	va_copy(args2, args);

	vsnprintf(str, sizeof(str), msg, args);

	obs_main_host_log_debugger(str);

	if (OBS_MAIN_LOG_ECHO_ALL) {
		def_log_handler(log_level, msg, args2, nullptr);
	}

	if (log_level <= LOG_INFO || log_verbose) {
		GpuStatus::OnLogLine(str);
		if (OBS_MAIN_LOG_ECHO && !OBS_MAIN_LOG_ECHO_ALL) {
			def_log_handler(log_level, msg, args2, nullptr);
		}
		if (!too_many_repeated_entries(logFile, msg, str)) {
			LogStringChunk(logFile, str, log_level);
		}
	}

	obs_main_host_log_break(log_level);

	va_end(args2);
}

static bool get_token(lexer *lex, string &str, base_token_type type)
{
	base_token token;
	if (!lexer_getbasetoken(lex, &token, IGNORE_WHITESPACE)) {
		return false;
	}
	if (token.type != type) {
		return false;
	}

	str.assign(token.text.array, token.text.len);
	return true;
}

static bool expect_token(lexer *lex, const char *str, base_token_type type)
{
	base_token token;
	if (!lexer_getbasetoken(lex, &token, IGNORE_WHITESPACE)) {
		return false;
	}
	if (token.type != type) {
		return false;
	}

	return strref_cmp(&token.text, str) == 0;
}

static uint64_t convert_log_name(bool has_prefix, const char *name)
{
	BaseLexer lex;
	string year, month, day, hour, minute, second;

	lexer_start(lex, name);

	if (has_prefix) {
		string temp;
		if (!get_token(lex, temp, BASETOKEN_ALPHA)) {
			return 0;
		}
	}

	if (!get_token(lex, year, BASETOKEN_DIGIT)) {
		return 0;
	}
	if (!expect_token(lex, "-", BASETOKEN_OTHER)) {
		return 0;
	}
	if (!get_token(lex, month, BASETOKEN_DIGIT)) {
		return 0;
	}
	if (!expect_token(lex, "-", BASETOKEN_OTHER)) {
		return 0;
	}
	if (!get_token(lex, day, BASETOKEN_DIGIT)) {
		return 0;
	}
	if (!get_token(lex, hour, BASETOKEN_DIGIT)) {
		return 0;
	}
	if (!expect_token(lex, "-", BASETOKEN_OTHER)) {
		return 0;
	}
	if (!get_token(lex, minute, BASETOKEN_DIGIT)) {
		return 0;
	}
	if (!expect_token(lex, "-", BASETOKEN_OTHER)) {
		return 0;
	}
	if (!get_token(lex, second, BASETOKEN_DIGIT)) {
		return 0;
	}

	stringstream timestring;
	timestring << year << month << day << hour << minute << second;
	return std::stoull(timestring.str());
}

void delete_oldest_file(bool has_prefix, const char *location)
{
	BPtr<char> logDir(GetAppConfigPathPtr(location));
	string oldestLog;
	uint64_t oldest_ts = (uint64_t)-1;
	struct os_dirent *entry;

	unsigned int maxLogs = (unsigned int)config_get_uint(App()->GetAppConfig(), "General", "MaxLogs");

	os_dir_t *dir = os_opendir(logDir);
	if (dir) {
		unsigned int count = 0;

		while ((entry = os_readdir(dir)) != NULL) {
			if (entry->directory || *entry->d_name == '.') {
				continue;
			}

			uint64_t ts = convert_log_name(has_prefix, entry->d_name);

			if (ts) {
				if (ts < oldest_ts) {
					oldestLog = entry->d_name;
					oldest_ts = ts;
				}

				count++;
			}
		}

		os_closedir(dir);

		if (count > maxLogs) {
			stringstream delPath;

			delPath << logDir << "/" << oldestLog;
			os_unlink(delPath.str().c_str());
		}
	}
}

static void get_last_log(bool has_prefix, const char *subdir_to_use, std::string &last)
{
	BPtr<char> logDir(GetAppConfigPathPtr(subdir_to_use));
	struct os_dirent *entry;
	os_dir_t *dir = os_opendir(logDir);
	uint64_t highest_ts = 0;

	if (dir) {
		while ((entry = os_readdir(dir)) != NULL) {
			if (entry->directory || *entry->d_name == '.') {
				continue;
			}

			uint64_t ts = convert_log_name(has_prefix, entry->d_name);

			if (ts > highest_ts) {
				last = entry->d_name;
				highest_ts = ts;
			}
		}

		os_closedir(dir);
	}
}

static void create_log_file(fstream &logFile)
{
	stringstream dst;

	get_last_log(false, "obs-studio/logs", lastLogFile);

	currentLogFile = GenerateTimeDateFilename("txt");
	dst << "obs-studio/logs/" << currentLogFile.c_str();

	BPtr<char> path(GetAppConfigPathPtr(dst.str().c_str()));

	obs_main_host_open_log(logFile, path);

	if (logFile.is_open()) {
		delete_oldest_file(false, "obs-studio/logs");
		base_set_log_handler(do_log, &logFile);
	} else {
		blog(LOG_ERROR, "Failed to open log file");
	}
}

static auto ProfilerNameStoreRelease = [](profiler_name_store_t *store) {
	profiler_name_store_free(store);
};

using ProfilerNameStore = std::unique_ptr<profiler_name_store_t, decltype(ProfilerNameStoreRelease)>;

ProfilerNameStore CreateNameStore()
{
	return ProfilerNameStore{profiler_name_store_create(), ProfilerNameStoreRelease};
}

static auto SnapshotRelease = [](profiler_snapshot_t *snap) {
	profile_snapshot_free(snap);
};

using ProfilerSnapshot = std::unique_ptr<profiler_snapshot_t, decltype(SnapshotRelease)>;

ProfilerSnapshot GetSnapshot()
{
	return ProfilerSnapshot{profile_snapshot_create(), SnapshotRelease};
}

static void SaveProfilerData(const ProfilerSnapshot &snap)
{
	if (currentLogFile.empty()) {
		return;
	}

	auto pos = currentLogFile.rfind('.');
	if (pos == currentLogFile.npos) {
		return;
	}

#define LITERAL_SIZE(x) x, (sizeof(x) - 1)
	ostringstream dst;
	dst.write(LITERAL_SIZE("obs-studio/profiler_data/"));
	dst.write(currentLogFile.c_str(), pos);
	dst.write(LITERAL_SIZE(".csv.gz"));
#undef LITERAL_SIZE

	BPtr<char> path = GetAppConfigPathPtr(dst.str().c_str());
	if (!profiler_snapshot_dump_csv_gz(snap.get(), path)) {
		blog(LOG_WARNING, "Could not save profiler data to '%s'", static_cast<const char *>(path));
	}
}

static auto ProfilerFree = [](void *) {
	profiler_stop();

	auto snap = GetSnapshot();

	profiler_print(snap.get());
	profiler_print_time_between_calls(snap.get());

	SaveProfilerData(snap);

	profiler_free();
};

QAccessibleInterface *accessibleFactory(const QString &classname, QObject *object)
{
	if (classname == QLatin1String("VolumeSlider") && object && object->isWidgetType()) {
		return new VolumeAccessibleInterface(static_cast<QWidget *>(object));
	}

	return nullptr;
}

static const char *run_program_init = "run_program_init";
static int run_program(fstream &logFile, int argc, char *argv[])
{
	int ret = -1;

	auto profilerNameStore = CreateNameStore();

	std::unique_ptr<void, decltype(ProfilerFree)> prof_release(static_cast<void *>(&ProfilerFree), ProfilerFree);

	profiler_start();
	profile_register_root(run_program_init, 0);

	ScopeProfiler prof{run_program_init};

	QCoreApplication::addLibraryPath(".");

	if (!obs_main_host_pre_app()) {
		return ret;
	}

	/* NOTE: This disables an optimisation in Qt that attempts to determine if
	 * any "siblings" intersect with a widget when determining the approximate
	 * visible/unobscured area. However, by Qt's own admission this is slow
	 * and in the case of OBS it significantly slows down lists with many
	 * elements (e.g. Hotkeys) and it is actually faster to disable it. */
	qputenv("QT_NO_SUBTRACTOPAQUESIBLINGS", "1");

	OBSApp program(argc, argv, profilerNameStore.get());
	try {
		QAccessible::installFactory(accessibleFactory);
		QFontDatabase::addApplicationFont(":/fonts/OpenSans-Regular.ttf");
		QFontDatabase::addApplicationFont(":/fonts/OpenSans-Bold.ttf");
		QFontDatabase::addApplicationFont(":/fonts/OpenSans-Italic.ttf");

		bool created_log = false;

		program.AppInit();
		delete_oldest_file(false, "obs-studio/profiler_data");

		OBSTranslator translator;
		program.installTranslator(&translator);

		/* --------------------------------------- */
		/* check and warn if already running       */

		bool cancel_launch = false;
		bool already_running = false;

		[[maybe_unused]] auto rom = obs_main_host_check_running(already_running);

		if (!already_running) {
			goto run;
		}

		if (!multi) {
			QMessageBox mb(QMessageBox::Question, QTStr("AlreadyRunning.Title"),
				       QTStr("AlreadyRunning.Text"));
			mb.addButton(QTStr("AlreadyRunning.LaunchAnyway"), QMessageBox::YesRole);
			QPushButton *cancelButton = mb.addButton(QTStr("Cancel"), QMessageBox::NoRole);
			mb.setDefaultButton(cancelButton);

			mb.exec();
			cancel_launch = mb.clickedButton() == cancelButton;
		}

		if (cancel_launch) {
			return 0;
		}

		if (!created_log) {
			create_log_file(logFile);
			created_log = true;
		}

		if (multi) {
			blog(LOG_INFO, "User enabled --multi flag and is now "
				       "running multiple instances of OBS.");
		} else {
			blog(LOG_WARNING, "================================");
			blog(LOG_WARNING, "Warning: OBS is already running!");
			blog(LOG_WARNING, "================================");
			blog(LOG_WARNING, "User is now running multiple "
					  "instances of OBS!");
			/* Clear unclean_shutdown flag as multiple instances
			 * running from the same config will lead to a
			 * false-positive detection.*/
			unclean_shutdown = false;
		}

		/* --------------------------------------- */
	run:

		if (obs_main_host_refuse_launch()) {
			return 0;
		}

		if (!created_log) {
			create_log_file(logFile);
		}

		obs_main_host_verify_startup();
		program.checkForUncleanShutdown();

		qInstallMessageHandler([](QtMsgType type, const QMessageLogContext &, const QString &message) {
			switch (type) {
#ifdef _DEBUG
			case QtDebugMsg:
				blog(LOG_DEBUG, "%s", QT_TO_UTF8(message));
				break;
			case QtInfoMsg:
				blog(LOG_INFO, "%s", QT_TO_UTF8(message));
				break;
#else
			case QtDebugMsg:
			case QtInfoMsg:
				break;
#endif
			case QtWarningMsg:
				blog(LOG_WARNING, "%s", QT_TO_UTF8(message));
				break;
			case QtCriticalMsg:
			case QtFatalMsg:
				blog(LOG_ERROR, "%s", QT_TO_UTF8(message));
				break;
			}
		});

		obs_main_host_permissions();

		if (argc > 1) {
			stringstream stor;
			stor << argv[1];
			for (int i = 2; i < argc; ++i) {
				stor << " " << argv[i];
			}
			blog(LOG_INFO, "Command Line Arguments: %s", stor.str().c_str());
		}

		if (!program.OBSInit()) {
			return 0;
		}

		prof.Stop();

		if (program.quitRequested()) {
			QTimer::singleShot(0, &program, &OBSApp::quit);
		}
		ret = program.exec();

	} catch (const char *error) {
		blog(LOG_ERROR, "%s", error);
		OBSErrorBox(nullptr, "%s", error);
	}

	if (restart || restart_safe) {
		arguments = qApp->arguments();

		if (restart_safe) {
			arguments.append("--safe-mode");
		} else {
			arguments.removeAll("--safe-mode");
		}
	}

	return ret;
}

static inline bool arg_is(const char *arg, const char *long_form, const char *short_form)
{
	return (long_form && strcmp(arg, long_form) == 0) || (short_form && strcmp(arg, short_form) == 0);
}

/* Where portable-mode marker files sit, and whether portable mode exists:
 * the host headers in the PCH may set both (#865). */
#ifndef BASE_PATH
#define BASE_PATH "../.."
#endif

#ifndef ALLOW_PORTABLE_MODE
#ifdef ENABLE_PORTABLE_CONFIG
#define ALLOW_PORTABLE_MODE 1
#else
#define ALLOW_PORTABLE_MODE 0
#endif
#endif

int main(int argc, char *argv[])
{
	InstallPlatformAppSignalHandlers();

	if (!obs_main_host_early_init()) {
		return 1;
	}

	base_get_log_handler(&def_log_handler, nullptr);

	obs_set_cmdline_args(argc, argv);

	for (int i = 1; i < argc; i++) {
		if (arg_is(argv[i], "--multi", "-m")) {
			multi = true;

#if ALLOW_PORTABLE_MODE
		} else if (arg_is(argv[i], "--portable", "-p")) {
			portable_mode = true;

#endif
		} else if (arg_is(argv[i], "--verbose", nullptr)) {
			log_verbose = true;

		} else if (arg_is(argv[i], "--safe-mode", nullptr)) {
			safe_mode = true;

		} else if (arg_is(argv[i], "--only-bundled-plugins", nullptr)) {
			disable_3p_plugins = true;

		} else if (arg_is(argv[i], "--disable-shutdown-check", nullptr)) {
			disable_shutdown_check = true;

		} else if (arg_is(argv[i], "--always-on-top", nullptr)) {
			opt_always_on_top = true;

		} else if (arg_is(argv[i], "--unfiltered_log", nullptr)) {
			unfiltered_log = true;

		} else if (arg_is(argv[i], "--startstreaming", nullptr)) {
			opt_start_streaming = true;

		} else if (arg_is(argv[i], "--startrecording", nullptr)) {
			opt_start_recording = true;

		} else if (arg_is(argv[i], "--startreplaybuffer", nullptr)) {
			opt_start_replaybuffer = true;

		} else if (arg_is(argv[i], "--startvirtualcam", nullptr)) {
			opt_start_virtualcam = true;

		} else if (arg_is(argv[i], "--collection", nullptr)) {
			if (++i < argc) {
				opt_starting_collection = argv[i];
			}

		} else if (arg_is(argv[i], "--profile", nullptr)) {
			if (++i < argc) {
				opt_starting_profile = argv[i];
			}

		} else if (arg_is(argv[i], "--scene", nullptr)) {
			if (++i < argc) {
				opt_starting_scene = argv[i];
			}

		} else if (arg_is(argv[i], "--minimize-to-tray", nullptr)) {
			opt_minimize_tray = true;

		} else if (arg_is(argv[i], "--studio-mode", nullptr)) {
			opt_studio_mode = true;

		} else if (arg_is(argv[i], "--allow-opengl", nullptr)) {
			opt_allow_opengl = true;

		} else if (arg_is(argv[i], "--disable-updater", nullptr)) {
			opt_disable_updater = true;

		} else if (arg_is(argv[i], "--disable-missing-files-check", nullptr)) {
			opt_disable_missing_files_check = true;

		} else if (arg_is(argv[i], "--steam", nullptr)) {
			steam = true;

		} else if (arg_is(argv[i], "--help", "-h")) {
			std::string help =
				"--help, -h: Get list of available commands.\n\n"
				"--startstreaming: Automatically start streaming.\n"
				"--startrecording: Automatically start recording.\n"
				"--startreplaybuffer: Start replay buffer.\n"
				"--startvirtualcam: Start virtual camera (if available).\n\n"
				"--collection <string>: Use specific scene collection."
				"\n"
				"--profile <string>: Use specific profile.\n"
				"--scene <string>: Start with specific scene.\n\n"
				"--studio-mode: Enable studio mode.\n"
				"--minimize-to-tray: Minimize to system tray.\n"
#if ALLOW_PORTABLE_MODE
				"--portable, -p: Use portable mode.\n"
#endif
				"--multi, -m: Don't warn when launching multiple instances.\n\n"
				"--safe-mode: Run in Safe Mode (disables third-party plugins, scripting, and WebSockets).\n"
				"--only-bundled-plugins: Only load included (first-party) plugins\n"
				"--verbose: Make log more verbose.\n"
				"--always-on-top: Start in 'always on top' mode.\n\n"
				"--unfiltered_log: Make log unfiltered.\n\n"
				"--disable-updater: Disable built-in updater (Windows/Mac only)\n\n"
				"--disable-missing-files-check: Disable the missing files dialog which can appear on startup.\n\n"
				"--disable-shutdown-check: Do not show the crash dialog after an unclean shutdown; log it and launch normally.\n\n";

			obs_main_host_show_help(help);
			exit(0);

		} else if (arg_is(argv[i], "--version", "-V")) {
#ifdef OBS_RUST_VERSION
			std::cout << App()->GetProductString(false) << "\n";
#else
			std::cout << "OBS Studio - " << App()->GetVersionString(false) << "\n";
#endif
			exit(0);
		}
	}

#if ALLOW_PORTABLE_MODE
	if (!portable_mode) {
		portable_mode = os_file_exists(BASE_PATH "/portable_mode") ||
				os_file_exists(BASE_PATH "/obs_portable_mode") ||
				os_file_exists(BASE_PATH "/portable_mode.txt") ||
				os_file_exists(BASE_PATH "/obs_portable_mode.txt");
	}

	if (!opt_disable_updater) {
		opt_disable_updater = os_file_exists(BASE_PATH "/disable_updater") ||
				      os_file_exists(BASE_PATH "/disable_updater.txt");
	}

	if (!opt_disable_missing_files_check) {
		opt_disable_missing_files_check = os_file_exists(BASE_PATH "/disable_missing_files_check") ||
						  os_file_exists(BASE_PATH "/disable_missing_files_check.txt");
	}
#endif

	fstream logFile;

	curl_global_init(CURL_GLOBAL_ALL);
	int ret = run_program(logFile, argc, argv);

	obs_main_host_shutdown();

	blog(LOG_INFO, "Number of memory leaks: %ld", bnum_allocs());
	base_set_log_handler(nullptr, nullptr);

	if (restart || restart_safe) {
		auto executable = arguments.takeFirst();
		QProcess::startDetached(executable, arguments);
	}

	return ret;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef MAX_REPEATED_LINES
#undef MAX_CHAR_VARIATION
#undef BASE_PATH
#undef ALLOW_PORTABLE_MODE
#undef OBS_MAIN_LOG_ECHO
#undef OBS_MAIN_LOG_ECHO_ALL
