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

#include "frontend/build/frontend-macos-arc-mm.pch.hpp"

using namespace std;

// Application icon policy (#725).
static bool macApplicationIconReady = false;

bool InitializeMacApplicationIcon()
{
	const QIcon icon(":/res/images/obs.png");
	if (icon.isNull()) {
		return false;
	}
	qApp->setWindowIcon(icon);

#if QT_VERSION >= QT_VERSION_CHECK(6, 11, 0) && QT_VERSION < QT_VERSION_CHECK(6, 12, 0)
	if (QGuiApplication::platformName() == "cocoa") {
		// Qt 6.11 compares the generic application icon synchronously through
		// IconServices before noticing our explicit icon. Its integration's
		// focus receiver only supplies a default Dock icon; native menus use
		// a different receiver. Keep all other focus connections intact.
		auto *receiver = dynamic_cast<QObject *>(QGuiApplicationPrivate::platformIntegration());
		if (!receiver) {
			return false;
		}
		if (!QObject::disconnect(qApp, SIGNAL(focusWindowChanged(QWindow *)), receiver, nullptr)) {
			return false;
		}
	}
#endif

	macApplicationIconReady = true;
	return true;
}

bool VerifyMacApplicationIcon()
{
	const bool ready = macApplicationIconReady && !qApp->windowIcon().isNull();
	blog(ready ? LOG_INFO : LOG_ERROR, "macOS application icon %s",
	     ready ? "verified: explicit" : "verification failed");
	return ready;
}

bool isInBundle()
{
	NSRunningApplication *app = [NSRunningApplication currentApplication];
	return [app bundleIdentifier] != nil;
}

bool GetDataFilePath(const char *data, string &output)
{
	NSURL *bundleUrl = [[NSBundle mainBundle] bundleURL];
	NSString *path = [[bundleUrl path] stringByAppendingFormat:@"/%@/%s", @"Contents/Resources", data];
	output = path.UTF8String;

	return !access(output.c_str(), R_OK);
}

void CheckIfAlreadyRunning(bool &already_running)
{
	NSString *bundleId = [[NSBundle mainBundle] bundleIdentifier];

	NSUInteger appCount = [[NSRunningApplication runningApplicationsWithBundleIdentifier:bundleId] count];

	already_running = appCount > 1;
}

string GetDefaultVideoSavePath()
{
	NSFileManager *fm = [NSFileManager defaultManager];
	NSURL *url = [fm URLForDirectory:NSMoviesDirectory
				inDomain:NSUserDomainMask
		       appropriateForURL:nil
				  create:true
				   error:nil];

	if (!url) {
		return getenv("HOME");
	}

	return url.path.fileSystemRepresentation;
}

vector<string> GetPreferredLocales()
{
	NSArray *preferred = [NSLocale preferredLanguages];

	auto locales = GetLocaleNames();
	auto lang_to_locale = [&locales](string lang) -> string {
		string lang_match = "";

		for (const auto &locale : locales) {
			if (locale.first == lang.substr(0, locale.first.size())) {
				return locale.first;
			}

			if (!lang_match.size() && locale.first.substr(0, 2) == lang.substr(0, 2)) {
				lang_match = locale.first;
			}
		}

		return lang_match;
	};

	vector<string> result;
	result.reserve(preferred.count);

	for (NSString *lang in preferred) {
		string locale = lang_to_locale(lang.UTF8String);
		if (!locale.size()) {
			continue;
		}

		if (find(begin(result), end(result), locale) != end(result)) {
			continue;
		}

		result.emplace_back(locale);
	}

	return result;
}

bool IsAlwaysOnTop(QWidget *window)
{
	return (window->windowFlags() & Qt::WindowStaysOnTopHint) != 0;
}

void disableColorSpaceConversion(QWidget *window)
{
	NSView *view = (__bridge NSView *)reinterpret_cast<void *>(window->winId());
	view.window.colorSpace = NSColorSpace.sRGBColorSpace;
}

void SetAlwaysOnTop(QWidget *window, bool enable)
{
	Qt::WindowFlags flags = window->windowFlags();

	if (enable) {
		NSView *view = (__bridge NSView *)reinterpret_cast<void *>(window->winId());

		[[view window] setLevel:NSScreenSaverWindowLevel];

		flags |= Qt::WindowStaysOnTopHint;
	} else {
		flags &= ~Qt::WindowStaysOnTopHint;
	}

	window->setWindowFlags(flags);
	window->show();
}

bool SetDisplayAffinitySupported(void)
{
	// Not implemented yet
	return false;
}

typedef void (*set_int_t)(int);

void EnableOSXVSync(bool enable)
{
	static bool initialized = false;
	static bool valid = false;
	static set_int_t set_debug_options = nullptr;
	static set_int_t deferred_updates = nullptr;

	if (!initialized) {
		void *quartzCore = dlopen("/System/Library/Frameworks/"
					  "QuartzCore.framework/QuartzCore",
					  RTLD_LAZY);
		if (quartzCore) {
			set_debug_options = (set_int_t)dlsym(quartzCore, "CGSSetDebugOptions");
			deferred_updates = (set_int_t)dlsym(quartzCore, "CGSDeferredUpdates");

			valid = set_debug_options && deferred_updates;
		}

		initialized = true;
	}

	if (valid) {
		set_debug_options(enable ? 0 : 0x08000000);
		deferred_updates(enable ? 1 : 0);
	}
}

void EnableOSXDockIcon(bool enable)
{
	if (enable) {
		[NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
	} else {
		[NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
	}
}

@interface DockView : NSView {
@private
	QIcon _icon;
}
@end

@implementation DockView

- (id)initWithIcon:(QIcon)icon
{
	self = [super init];
	_icon = icon;
	return self;
}

- (void)drawRect:(NSRect)dirtyRect
{
	CGSize size = dirtyRect.size;

	/* Draw regular app icon */
	NSImage *appIcon = [[NSWorkspace sharedWorkspace] iconForFile:[[NSBundle mainBundle] bundlePath]];
	[appIcon drawInRect:CGRectMake(0, 0, size.width, size.height)];

	/* Draw small icon on top */
	float iconSize = 0.45;
	CGImageRef image = _icon.pixmap(size.width, size.height).toImage().toCGImage();
	CGContextRef context = [[NSGraphicsContext currentContext] CGContext];
	CGContextDrawImage(context,
			   CGRectMake(size.width * (1 - iconSize), 0, size.width * iconSize, size.height * iconSize),
			   image);
	CGImageRelease(image);
}

@end

MacPermissionStatus CheckPermissionWithPrompt(MacPermissionType type, bool prompt_for_permission)
{
	__block MacPermissionStatus permissionResponse = kPermissionNotDetermined;

	switch (type) {
	case kAudioDeviceAccess: {
		AVAuthorizationStatus audioStatus = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];

		if (audioStatus == AVAuthorizationStatusNotDetermined && prompt_for_permission) {
			/* Without the event, prompt anyway, but do not wait for the answer. */
			os_event_t *block_finished = nullptr;
			if (os_event_init(&block_finished, OS_EVENT_TYPE_MANUAL) != 0) {
				blog(LOG_WARNING, "[macOS] Not waiting for the audio device access prompt");
			}
			[AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio
						 completionHandler:^(BOOL granted __attribute((unused))) {
							 if (block_finished) {
								 os_event_signal(block_finished);
							 }
						 }];
			if (block_finished) {
				os_event_wait(block_finished);
				os_event_destroy(block_finished);
			}
			audioStatus = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
		}

		permissionResponse = (MacPermissionStatus)audioStatus;

		blog(LOG_INFO, "[macOS] Permission for audio device access %s.",
		     permissionResponse == kPermissionAuthorized ? "granted" : "denied");

		break;
	}
	case kVideoDeviceAccess: {
		AVAuthorizationStatus videoStatus = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeVideo];

		if (videoStatus == AVAuthorizationStatusNotDetermined && prompt_for_permission) {
			/* Without the event, prompt anyway, but do not wait for the answer. */
			os_event_t *block_finished = nullptr;
			if (os_event_init(&block_finished, OS_EVENT_TYPE_MANUAL) != 0) {
				blog(LOG_WARNING, "[macOS] Not waiting for the video device access prompt");
			}
			[AVCaptureDevice requestAccessForMediaType:AVMediaTypeVideo
						 completionHandler:^(BOOL granted __attribute((unused))) {
							 if (block_finished) {
								 os_event_signal(block_finished);
							 }
						 }];

			if (block_finished) {
				os_event_wait(block_finished);
				os_event_destroy(block_finished);
			}
			videoStatus = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeVideo];
		}

		permissionResponse = (MacPermissionStatus)videoStatus;

		blog(LOG_INFO, "[macOS] Permission for video device access %s.",
		     permissionResponse == kPermissionAuthorized ? "granted" : "denied");

		break;
	}
	case kScreenCapture: {
		permissionResponse = (CGPreflightScreenCaptureAccess() ? kPermissionAuthorized : kPermissionDenied);

		if (permissionResponse != kPermissionAuthorized && prompt_for_permission) {
			permissionResponse =
				(CGRequestScreenCaptureAccess() ? kPermissionAuthorized : kPermissionDenied);
		}

		blog(LOG_INFO, "[macOS] Permission for screen capture %s.",
		     permissionResponse == kPermissionAuthorized ? "granted" : "denied");

		break;
	}
	case kInputMonitoring: {
		permissionResponse = (CGPreflightListenEventAccess() ? kPermissionAuthorized : kPermissionDenied);

		if (permissionResponse != kPermissionAuthorized && prompt_for_permission) {
			permissionResponse = (CGRequestListenEventAccess() ? kPermissionAuthorized : kPermissionDenied);
		}

		blog(LOG_INFO, "[macOS] Permission for input monitoring %s.",
		     permissionResponse == kPermissionAuthorized ? "granted" : "denied");
		break;
	}
	}

	return permissionResponse;
}

void OpenMacOSPrivacyPreferences(const char *tab)
{
	NSURL *url = [NSURL
		URLWithString:[NSString stringWithFormat:
						@"x-apple.systempreferences:com.apple.preference.security?Privacy_%s",
						tab]];
	[[NSWorkspace sharedWorkspace] openURL:url];
}

void SetMacOSDarkMode(bool dark)
{
	if (dark) {
		NSApp.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
	} else {
		NSApp.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
	}
}

void TaskbarOverlayInit() {}

void TaskbarOverlaySetStatus(TaskbarOverlayStatus status)
{
	QIcon icon;
	if (status == TaskbarOverlayStatusActive) {
		icon = QIcon::fromTheme("obs-active", QIcon(":/res/images/active_mac.png"));
	} else if (status == TaskbarOverlayStatusPaused) {
		icon = QIcon::fromTheme("obs-paused", QIcon(":/res/images/paused_mac.png"));
	}

	NSDockTile *tile = [NSApp dockTile];
	[tile setContentView:[[DockView alloc] initWithIcon:icon]];
	[tile display];
}

/*
 * This custom NSApplication subclass makes the app compatible with CEF. Qt
 * also has an NSApplication subclass, but it doesn't conflict thanks to Qt
 * using arcane magic to hook into the NSApplication superclass itself if the
 * program has its own NSApplication subclass.
 */

@protocol CrAppProtocol
- (BOOL)isHandlingSendEvent;
@end

@interface OBSApplication : NSApplication <CrAppProtocol>
@property (nonatomic, getter=isHandlingSendEvent) BOOL handlingSendEvent;
@end

@implementation OBSApplication

- (void)sendEvent:(NSEvent *)event
{
	_handlingSendEvent = YES;
	[super sendEvent:event];
	_handlingSendEvent = NO;
}

@end

void InstallNSThreadLocks()
{
	[[NSThread new] start];

	if ([NSThread isMultiThreaded] != 1) {
		abort();
	}
}

void InstallNSApplicationSubclass()
{
	[OBSApplication sharedApplication];
}

void *GetGlGetStringProc()
{
	void *gl = dlopen("/System/Library/Frameworks/OpenGL.framework/OpenGL", RTLD_LAZY);
	return gl ? dlsym(gl, "glGetString") : nullptr;
}

bool HighContrastEnabled()
{
	return [[NSWorkspace sharedWorkspace] accessibilityDisplayShouldIncreaseContrast];
}

/* OBSApp platform facade (#849) */

void InitPlatformConfigDefaults(config_t *appConfig)
{
	// TODO: Change this value to "Metal" once the renderer has reached production quality
	config_set_default_string(appConfig, "Video", "Renderer", "OpenGL");
	config_set_default_bool(appConfig, "General", "BrowserHWAccel", true);
	config_set_default_bool(appConfig, "Video", "DisableOSXVSync", true);
	config_set_default_bool(appConfig, "Video", "ResetOSXVSyncOnExit", true);
}

void ApplyPlatformAppSettings(config_t *appConfig)
{
	if (config_get_bool(appConfig, "Video", "DisableOSXVSync")) {
		EnableOSXVSync(false);
	}
}

void RevertPlatformAppSettings(config_t *appConfig)
{
	bool vsyncDisabled = config_get_bool(appConfig, "Video", "DisableOSXVSync");
	bool resetVSync = config_get_bool(appConfig, "Video", "ResetOSXVSyncOnExit");
	if (vsyncDisabled && resetVSync) {
		EnableOSXVSync(true);
	}
}

void LogPlatformSettings(config_t *) {}

bool PlatformSupportsBrowserHWAccel()
{
	return true;
}

std::vector<std::string> GetPlatformExtraConfigDirs()
{
	return {};
}

bool InitializeAppIdentity()
{
	return true;
}

void SetApplicationIcon()
{
	InitializeMacApplicationIcon();
}

void InitPlatformDisplay()
{
	QApplication::setAttribute(Qt::AA_DontCreateNativeWidgetSiblings);
}

bool GetPlatformHotkeyOverride(int qtKey, obs_key_t &key, bool &pressed)
{
	if (qtKey != Qt::Key_CapsLock) {
		return false;
	}
	// kVK_CapsLock == 57
	key = obs_key_from_virtual_key(57);
	pressed = true;
	return true;
}

void PlatformLocalTime(const time_t *now, struct tm *out)
{
	localtime_r(now, out);
}

const char *GetPlatformRenderModule(const char *renderer)
{
#if defined(__aarch64__)
	return (astrcmpi(renderer, "Metal") == 0) ? DL_METAL : DL_OPENGL;
#else
	UNUSED_PARAMETER(renderer);
	return DL_OPENGL;
#endif
}

std::string GetPlatformVersionName()
{
	return "mac";
}

const char *GetPlatformInputAudioSource()
{
	return "coreaudio_input_capture";
}

const char *GetPlatformOutputAudioSource()
{
	return "coreaudio_output_capture";
}

bool IsPortableModeAllowed()
{
#ifdef ENABLE_PORTABLE_CONFIG
	return true;
#else
	return false;
#endif
}

const char *GetPortableConfigBasePath()
{
	return "..";
}

bool IsUpdateBranchSupported(bool, bool macos)
{
	return macos;
}

bool PlatformSavesBeforeSignalQuit()
{
	return false;
}

/* OAuth and auto-configuration platform policy (#887). */
bool PlatformSupportsOAuthBrowser()
{
	return true;
}

void ConfigureOAuthLinkMessageBox(QMessageBox &dialog)
{
	/* Native NSAlert cannot show the clickable OAuth link. */
	dialog.setOption(QMessageBox::Option::DontUseNativeDialog);
}

bool PlatformUseModernWizardStyle()
{
	return true;
}

bool PlatformSupportsAppleHardwareEncoder(const char *id)
{
	if (strcmp(id, "com.apple.videotoolbox.videoencoder.ave.avc") != 0) {
		return false;
	}
#ifndef __aarch64__
	if (!os_get_emulation_status()) {
		return false;
	}
#endif
	if (__builtin_available(macOS 13.0, *)) {
		return true;
	}
	return false;
}

bool PlatformNeedsQsvStreamingFallback()
{
	return false;
}

/* Settings and hotkey editor platform policy (#869). */
bool PlatformNeedsNonNativeColorDialog()
{
	return false;
}

bool PlatformSupportsSimpleQsvEncoders()
{
	return false;
}

bool PlatformSupportsSimpleAppleEncoders()
{
	// H.264 and HEVC use the same architecture and OS availability policy.
	return PlatformSupportsAppleHardwareEncoder("com.apple.videotoolbox.videoencoder.ave.avc");
}

bool PlatformHotkeyEditHandlesKeyRelease()
{
	return true;
}

/* Importer platform policy (#881). Translation behavior is retained from
 * frontend/importers/studio.cpp, Copyright (C) 2019-2020 Dillon Pentz. */
size_t PlatformImporterLastSeparator(const std::string &path)
{
	return path.find_last_of('/');
}

bool GetPlatformClassicImportDirectory(char *, size_t)
{
	return false;
}

bool GetPlatformStreamlabsImportDirectory(char *directory, size_t size)
{
	return os_get_config_path(directory, size, "slobs-client/SceneCollections/") != -1;
}

bool GetPlatformXSplitImportDirectory(char *, size_t)
{
	return false;
}

void TranslatePlatformImportedSource(json11::Json &value)
{
	using json11::Json;
	Json::object source = value.object_items();
	Json::object settings = source["settings"].object_items();
	std::string id = source["id"].string_value();

#define DirectTranslation(before, after)                                      \
	if (id == before) {                                                   \
		source["id"] = after;                                         \
		source["versioned_id"] = obs_get_latest_input_type_id(after); \
	}

#define ClearTranslation(before, after)                                       \
	if (id == before) {                                                   \
		source["id"] = after;                                         \
		source["settings"] = Json::object{};                          \
		source["versioned_id"] = obs_get_latest_input_type_id(after); \
	}

	DirectTranslation("text_gdiplus", "text_ft2_source");

	ClearTranslation("game_capture", "syphon-input");

	ClearTranslation("wasapi_input_capture", "coreaudio_input_capture");
	ClearTranslation("wasapi_output_capture", "coreaudio_output_capture");
	ClearTranslation("pulse_input_capture", "coreaudio_input_capture");
	ClearTranslation("pulse_output_capture", "coreaudio_output_capture");

	ClearTranslation("jack_output_capture", "coreaudio_output_capture");
	ClearTranslation("alsa_input_capture", "coreaudio_input_capture");

	ClearTranslation("dshow_input", "av_capture_input");
	ClearTranslation("v4l2_input", "av_capture_input");

	ClearTranslation("xcomposite_input", "window_capture");

	if (id == "monitor_capture") {
		if (settings["show_cursor"].is_null() && !settings["capture_cursor"].is_null()) {
			bool cursor = settings["capture_cursor"].bool_value();

			settings["show_cursor"] = cursor;
		}
	}

	DirectTranslation("xshm_input", "monitor_capture");

	// Preserve the original final settings assignment, including its legacy
	// interaction with ClearTranslation and null cursor-property lookups.
	source["settings"] = settings;
	value = source;
#undef DirectTranslation
#undef ClearTranslation
}

/* Theme policy (#885). */
const char *GetPlatformThemePrefix()
{
	return "os_mac_";
}

QStyle *CreatePlatformThemeStyle()
{
	return new OBSProxyStyle();
}

void ApplyPlatformThemeDarkMode(bool dark)
{
	SetMacOSDarkMode(dark);
}

const char *GetPlatformFallbackTheme(const char *defaultTheme)
{
	return defaultTheme;
}

void InstallPlatformDllBlocklist() {}
void LogPlatformBlockedDlls() {}

/* Shared dialog platform policy (#879). */
const char *GetPlatformAuthorsFile()
{
	return "AUTHORS";
}

Qt::Key GetPlatformFilterRenameKey()
{
	return Qt::Key_Return;
}

void HandlePlatformDialogNativeEvent(QWidget *dialog, void *message)
{
	(void)dialog;
	(void)message;
}

int TranslatePlatformInteractionModifiers(Qt::KeyboardModifiers mods, bool mouseEvent)
{
	int obsModifiers = INTERACT_NONE;

	if (mods.testFlag(Qt::ShiftModifier)) {
		obsModifiers |= INTERACT_SHIFT_KEY;
	}
	if (mods.testFlag(Qt::AltModifier)) {
		obsModifiers |= INTERACT_ALT_KEY;
	}
	// Mac: Meta = Control, Control = Command
	if (mods.testFlag(Qt::ControlModifier)) {
		obsModifiers |= INTERACT_COMMAND_KEY;
	}
	if (mods.testFlag(Qt::MetaModifier)) {
		obsModifiers |= INTERACT_CONTROL_KEY;
	}

	if (!mouseEvent) {
		if (mods.testFlag(Qt::KeypadModifier)) {
			obsModifiers |= INTERACT_IS_KEY_PAD;
		}
	}

	return obsModifiers;
}

const char *GetPlatformPropertiesFontFace()
{
	return "Helvetica";
}

const char *GetPlatformPropertiesTextSource()
{
	return "text_ft2_source";
}

float GetPlatformPropertiesLabelHeight(uint32_t height)
{
	return height * 0.8;
}

void AdjustPlatformYouTubeDialogSize(QWidget *dialog)
{
	dialog->resize(dialog->width() + 200, dialog->height() + 120);
}

/* Component host policy (#877). */
const char *GetPlatformCaptureModuleId(PlatformCaptureKind kind)
{
	switch (kind) {
	case PlatformCaptureKind::Audio:
		return "mac-capture";
	case PlatformCaptureKind::Display:
		return "mac-capture";
	case PlatformCaptureKind::Window:
		return "mac-capture";
	}
	return nullptr;
}

const char *GetPlatformCaptureLabelKey(PlatformCaptureKind kind)
{
	switch (kind) {
	case PlatformCaptureKind::Audio:
		return "CoreAudio.Device";
	case PlatformCaptureKind::Display:
		return "DisplayCapture.Display";
	case PlatformCaptureKind::Window:
		return "WindowUtils.Window";
	}
	return nullptr;
}

const char *GetPlatformCaptureProperty(PlatformCaptureKind kind, bool &is_int)
{
	const char *prop_name = "device_id";
	if (kind == PlatformCaptureKind::Display) {
		prop_name = "display_uuid";
	} else if (kind == PlatformCaptureKind::Window) {
		prop_name = "window";
		is_int = true;
	}
	return prop_name;
}

bool PlatformNeedsNonNativeFontDialog()
{
	return true;
}

void ConfigurePlatformSourceItemWidget(QWidget *widget)
{
	widget->setAttribute(Qt::WA_LayoutUsesWidgetRect);
}

bool PlatformEndsSourceEdit()
{
	return true;
}

bool PlatformNeedsDeferredSourceProperties()
{
	return false;
}

QMessageBox::ButtonRole GetPlatformStreamWarningButtonRole(bool settings)
{
	QMessageBox::ButtonRole accept, reject;
	accept = QMessageBox::AcceptRole;
	reject = QMessageBox::ResetRole;
	return settings ? accept : reject;
}

QPalette::ColorGroup GetPlatformVisibilityPaletteGroup(bool active)
{
	QPalette::ColorGroup group = active ? QPalette::Active : QPalette::Inactive;
	return group;
}

QPalette::ColorRole GetPlatformVisibilityHighlightRole()
{
	QPalette::ColorRole highlightRole = QPalette::HighlightedText;
	return highlightRole;
}

/* Widget graphics handles and policy (#875). */
void ApplyPlatformPreviewTextOutline(obs_data_t *settings)
{
	(void)settings;
}

void SetPlatformWidgetIcon(QWidget *widget)
{
	(void)widget;
}

void ConfigurePlatformProjectorWindow(QWidget *widget)
{
	(void)widget;
}

bool FillPlatformGraphicsWindow(QWindow *window, gs_window &gswindow)
{
	// Qt owns the NSView; this bridge must not retain or release it.
	gswindow.view = (__bridge id)(void *)window->winId();
	return true;
}

bool PlatformResetsHiddenDisplay()
{
	return false;
}

void HandlePlatformDisplayNativeEvent(OBSQTDisplay *display, void *message)
{
	(void)display;
	(void)message;
}

void SetOutputNetworkOptions(config_t *, obs_data_t *)
{
	/* No host-specific streaming output options. */
}

void TranslateLegacyEncoderId(const char *&encoder)
{
	if (strcmp(encoder, "vt_h264_hw") == 0) {
		encoder = "com.apple.videotoolbox.videoencoder.h264.gva";
	} else if (strcmp(encoder, "vt_h264_sw") == 0) {
		encoder = "com.apple.videotoolbox.videoencoder.h264";
	}
}

void RaiseAfterFileDialog(QWidget *window)
{
	// TODO: Revisit when QTBUG-42661 is fixed
	window->raise();
}

bool UseNativeComboBoxPopup()
{
	return true;
}

enum gs_color_space ScreenshotColorSpace(obs_source_t *)
{
	/* Tonemap to SDR if HDR */
	return GS_CS_SRGB;
}

bool SaveHdrScreenshot(const char *, uint8_t *, uint32_t, uint32_t)
{
	/* No HDR image format here: screenshots are rendered in sRGB. */
	return false;
}

bool IsPlatformImageExtension(const char *)
{
	return false;
}

const char *GetPlatformTextSourceId()
{
	return "text_ft2_source";
}

void SetPlatformTextFileSettings(obs_data_t *settings, const char *file, QString &)
{
	obs_data_set_bool(settings, "from_file", true);
	obs_data_set_string(settings, "text_file", file);
}

bool ResolvePlatformDroppedFile(QString &, QFileInfo &, QString &)
{
	return true;
}

const char *GetPlatformMetaKeyText()
{
	return "Hotkeys.Super";
}

QString GetPlatformScreenName(QScreen *screen)
{
	return screen->name();
}

bool PlatformUsesDesktopAudioDevice()
{
	/* On macOS 13 and above, the SCK based audio capture provides a
	 * better alternative to the device-based audio capture. */
	if (__builtin_available(macOS 13.0, *)) {
		return false;
	}
	return true;
}

QIcon GetPlatformTrayIcon(TrayIconState state)
{
	const char *file = ":/res/images/obs_macos.svg";
	switch (state) {
	case TrayIconState::Active:
		file = ":/res/images/tray_active_macos.svg";
		break;
	case TrayIconState::Paused:
		file = ":/res/images/obs_paused_macos.svg";
		break;
	case TrayIconState::Idle:
		break;
	}
	QIcon icon(file);
	icon.setIsMask(true);
	return icon;
}

bool PlatformTrayActivationTogglesWindow()
{
	return false;
}

void SetPlatformDockIconVisible(bool visible)
{
	EnableOSXDockIcon(visible);
}

const char *GetPlatformNvencTestProgram()
{
	return nullptr;
}

void ShowPlatformPermissionsDialog(QWidget *parent)
{
	OBSPermissions check(parent, CheckPermission(kScreenCapture), CheckPermission(kVideoDeviceAccess),
			     CheckPermission(kAudioDeviceAccess), CheckPermission(kInputMonitoring));
	check.exec();
}

bool PlatformRecreatesWindowsOnTopToggle()
{
	return true;
}

bool PlatformSupportsBuiltinUpdater()
{
	return false;
}

QThread *CreatePlatformUpdateThread(bool, bool)
{
	return nullptr;
}

bool IsWhatsNewItemForPlatform(bool, bool macos, bool)
{
	return macos;
}
