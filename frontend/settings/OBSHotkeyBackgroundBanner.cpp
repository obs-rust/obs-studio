/******************************************************************************
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

#include "OBSHotkeyBackgroundBanner.hpp"

#include <OBSApp.hpp>
#include <qt-wrappers.hpp>
#include <util/dstr.hpp>

#include <QCheckBox>
#include <QClipboard>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QHBoxLayout>
#include <QLabel>
#include <QPlainTextEdit>
#include <QProcess>
#include <QThread>
#include <QAccessible>
#include <QCoreApplication>
#include <QStyle>
#include <QPushButton>
#include <QTimer>
#include <QVBoxLayout>

#include "moc_OBSHotkeyBackgroundBanner.cpp"

using namespace HotkeyBackground;

/* How long "Test a key" waits for a key from the desktop; "Ask again" waits
 * longer, as the user first answers the desktop's dialog. */
#define TEST_TIMEOUT_MS 15000
#define ASK_TIMEOUT_MS 60000

/* ------------------------------------------------------------------------ */
/* Desktops that take the keys from their own config                         */

/* Runs a read-only query; empty if it did not answer. */
static std::string Query(const QString &program, const QStringList &args)
{
	QProcess process;
	process.start(program, args, QIODevice::ReadOnly);
	if (!process.waitForFinished(2000) || process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0) {
		process.kill();
		return {};
	}
	return process.readAllStandardOutput().toStdString();
}

static QString ConfigDir()
{
	QString base = qEnvironmentVariable("XDG_CONFIG_HOME");
	if (base.isEmpty()) {
		base = QDir::homePath() + "/.config";
	}
	return base;
}

static std::string DisplayPath(const QString &path)
{
	QString home = QDir::homePath();
	if (!home.isEmpty() && path.startsWith(home + "/")) {
		return ("~" + path.mid(home.size())).toStdString();
	}
	return path.toStdString();
}

/* Hyprland binds keys to portal shortcuts only in its config (xdph ignores
 * the trigger OBS suggests). 0.55+ reads hyprland.lua when it exists. */
static std::optional<DesktopConfig> ReadHyprland()
{
	if (qEnvironmentVariableIsEmpty("HYPRLAND_INSTANCE_SIGNATURE")) {
		return std::nullopt;
	}

	DesktopConfig desktop;
	desktop.name = "Hyprland";

	QString dir = ConfigDir() + "/hypr";
	bool hasLua = QFileInfo::exists(dir + "/hyprland.lua");
	bool hasConf = QFileInfo::exists(dir + "/hyprland.conf");
	auto version = ParseHyprlandVersion(Query("hyprctl", {"-j", "version"}));
	bool luaVersion = !version || *version >= std::make_pair(0, 55);
	desktop.lua = luaVersion && (hasLua || !hasConf);
	desktop.configFile = DisplayPath(dir + (desktop.lua ? "/hyprland.lua" : "/hyprland.conf"));

	std::string binds = Query("hyprctl", {"-j", "binds"});
	if (!binds.empty()) {
		desktop.binds = ParseHyprlandBinds(binds);
	}
	return desktop;
}

/* One entry per desktop of this kind; Sway or niri would be added here once
 * they offer GlobalShortcuts. */
static std::optional<DesktopConfig> (*const desktopReaders[])() = {ReadHyprland};

/* "Basic...NeedsConfig" for several, "Basic...NeedsConfig.One" for one. */
static QString QTStrN(const char *key, size_t n)
{
	return n == 1 ? QTStr((std::string(key) + ".One").c_str()) : QTStr(key);
}

/* ------------------------------------------------------------------------ */

OBSHotkeyBackgroundBanner::OBSHotkeyBackgroundBanner(QWidget *parent,
						     std::function<std::vector<obs_key_combination_t>()> combos_,
						     std::function<bool()> offBySetting_)
	: QFrame(parent),
	  combos(std::move(combos_)),
	  offBySetting(std::move(offBySetting_)),
	  statusSignal(obs_get_signal_handler(), "hotkey_background_status_changed",
		       &OBSHotkeyBackgroundBanner::StatusChanged, this),
	  probeSignal(obs_get_signal_handler(), "hotkey_background_probe", &OBSHotkeyBackgroundBanner::Probe, this)
{
	setObjectName("hotkeyBackgroundBanner");
	setFrameShape(QFrame::StyledPanel);

	icon = new QLabel(this);
	icon->setAlignment(Qt::AlignTop);
	text = new QLabel(this);
	text->setWordWrap(true);
	text->setTextInteractionFlags(Qt::TextSelectableByMouse | Qt::TextSelectableByKeyboard);
	text->setFocusPolicy(Qt::StrongFocus);
	text->setObjectName("hotkeyBackgroundText");
	text->setAccessibleDescription(QTStr("Basic.Settings.Hotkeys.Background.Status"));

	auto button = [this](const char *name, const char *label, void (OBSHotkeyBackgroundBanner::*slot)()) {
		auto b = new QPushButton(QTStr(label), this);
		b->setObjectName(name);
		connect(b, &QPushButton::clicked, this, slot);
		return b;
	};
	copyButton = button("hotkeyBackgroundCopy", "Basic.Settings.Hotkeys.Background.CopyLines",
			    &OBSHotkeyBackgroundBanner::CopyConfigLines);
	checkButton = button("hotkeyBackgroundCheck", "Basic.Settings.Hotkeys.Background.CheckAgain",
			     &OBSHotkeyBackgroundBanner::CheckAgain);
	testButton = button("hotkeyBackgroundTest", "Basic.Settings.Hotkeys.Background.TestKey",
			    &OBSHotkeyBackgroundBanner::TestKey);
	testButton->setToolTip(QTStr("Basic.Settings.Hotkeys.Background.TestKey.ToolTip"));
	askButton = button("hotkeyBackgroundAsk", "Basic.Settings.Hotkeys.Background.AskAgain",
			   &OBSHotkeyBackgroundBanner::AskAgain);
	configureButton = button("hotkeyBackgroundConfigure", "Basic.Settings.Hotkeys.Background.Configure",
				 &OBSHotkeyBackgroundBanner::Configure);
	configureButton->setToolTip(QTStr("Basic.Settings.Hotkeys.Background.Configure.ToolTip"));
	cancelButton = button("hotkeyBackgroundCancel", "Cancel", &OBSHotkeyBackgroundBanner::StopTest);

	auto buttons = new QHBoxLayout;
	buttons->addStretch();
	for (QPushButton *b : {copyButton.data(), checkButton.data(), configureButton.data(), testButton.data(),
			       askButton.data(), cancelButton.data()}) {
		buttons->addWidget(b);
	}

	auto top = new QHBoxLayout;
	top->addWidget(icon);
	top->addWidget(text, 1);

	auto layout = new QVBoxLayout(this);
	layout->setContentsMargins(8, 6, 8, 6);
	layout->addLayout(top);
	layout->addLayout(buttons);

	testTimer = new QTimer(this);
	testTimer->setSingleShot(true);
	connect(testTimer, &QTimer::timeout, this, [this]() {
		/* Still in the desktop's dialog: keep listening. */
		if ((asking && obs_hotkeys_get_background_state(nullptr) == OBS_HOTKEYS_BACKGROUND_WAITING) ||
		    configuring) {
			testTimer->start(TEST_TIMEOUT_MS);
			return;
		}
		testResult = QTStr("Basic.Settings.Hotkeys.Background.TestTimeout");
		testLevel = Level::Warning;
		StopTest();
	});

	Refresh(true);
}

OBSHotkeyBackgroundBanner::~OBSHotkeyBackgroundBanner()
{
	statusSignal.Disconnect();
	probeSignal.Disconnect();
	if (testing) {
		obs_hotkeys_set_background_probe(false);
	}
}

void OBSHotkeyBackgroundBanner::StatusChanged(void *data, calldata_t *)
{
	QMetaObject::invokeMethod(static_cast<OBSHotkeyBackgroundBanner *>(data), "Refresh", Qt::QueuedConnection);
}

void OBSHotkeyBackgroundBanner::Probe(void *data, calldata_t *params)
{
	QString id = QString::fromUtf8(calldata_string(params, "id"));
	QMetaObject::invokeMethod(static_cast<OBSHotkeyBackgroundBanner *>(data), "ProbeReceived", Qt::QueuedConnection,
				  Q_ARG(QString, id));
}

/* The readers run hyprctl; off the UI thread, so Settings never waits. */
void OBSHotkeyBackgroundBanner::ReadDesktopConfig()
{
	if (readingConfig) {
		return;
	}
	readingConfig = true;

	QPointer<OBSHotkeyBackgroundBanner> self = this;
	QThread *thread = QThread::create([self]() {
		std::optional<DesktopConfig> found;
		for (auto reader : desktopReaders) {
			found = reader();
			if (found) {
				break;
			}
		}
		QMetaObject::invokeMethod(
			qApp,
			[self, found]() {
				if (self) {
					self->DesktopConfigRead(found);
				}
			},
			Qt::QueuedConnection);
	});
	connect(thread, &QThread::finished, thread, &QObject::deleteLater);
	thread->start();
}

void OBSHotkeyBackgroundBanner::DesktopConfigRead(std::optional<HotkeyBackground::DesktopConfig> found)
{
	desktop = std::move(found);
	readingConfig = false;
	configRead = true;
	Refresh();
}

static bool CollectShortcut(void *data, obs_key_combination_t combo, const char *trigger, bool bound)
{
	auto shortcuts = static_cast<std::vector<Shortcut> *>(data);
	shortcuts->push_back({combo, trigger ? trigger : "", bound});
	return true;
}

static std::string TakeString(char *str)
{
	std::string out = str ? str : "";
	bfree(str);
	return out;
}

void OBSHotkeyBackgroundBanner::Refresh(bool readConfig)
{
	input = Input();
	input.state = obs_hotkeys_get_background_state(&input.flags);
	if (input.state == OBS_HOTKEYS_BACKGROUND_NATIVE) {
		setVisible(false);
		emit Refreshed();
		return;
	}

	if (readConfig) {
		ReadDesktopConfig();
	}

	input.error = TakeString(obs_hotkeys_get_background_error());
	const char *appId = obs_hotkeys_get_background_app_id();
	input.appId = appId ? appId : "";
	input.offBySetting = offBySetting && offBySetting();
	input.desktop = desktop;
	input.confirmedIds = confirmedIds;
	obs_hotkeys_enum_background_shortcuts(CollectShortcut, &input.shortcuts);

	for (auto &combo : combos ? combos() : std::vector<obs_key_combination_t>()) {
		if (obs_key_combination_is_empty(combo)) {
			continue;
		}
		bool known = std::any_of(input.combos.begin(), input.combos.end(), [&](const Combo &c) {
			return c.combo.key == combo.key && c.combo.modifiers == combo.modifiers;
		});
		if (!known) {
			input.combos.push_back({combo, TakeString(obs_hotkey_combination_get_background_id(combo)),
						TakeString(obs_hotkey_combination_get_background_trigger(combo))});
		}
	}

	view = Compute(input);
	if (configuring && view.configureFailed) {
		/* No editor opened: say so and how to change the keys instead,
		 * rather than waiting for Done. */
		testResult = QTStr("Basic.Settings.Hotkeys.Background.ConfigureFailed");
		testLevel = Level::Warning;
		QMetaObject::invokeMethod(this, &OBSHotkeyBackgroundBanner::StopTest, Qt::QueuedConnection);
	}
	if (!configRead && view.banner != Banner::Hidden) {
		/* Until OBS knows whether the desktop needs config lines. */
		view = View();
		view.banner = Banner::Starting;
	}
	setVisible(view.banner != Banner::Hidden);

	QString name = desktop ? QString::fromStdString(desktop->name) : QString();
	QString message;
	switch (view.banner) {
	case Banner::Hidden:
		break;
	case Banner::Starting:
		message = QTStr("Basic.Settings.Hotkeys.Background.Starting");
		break;
	case Banner::Working:
		message = desktop ? QTStr("Basic.Settings.Hotkeys.Background.WorkingConfig")
					    .arg(view.working)
					    .arg(view.keys)
					    .arg(name)
				  : QTStr("Basic.Settings.Hotkeys.Background.Working").arg(view.working).arg(view.keys);
		if (view.otherKey) {
			message +=
				" " +
				QTStrN("Basic.Settings.Hotkeys.Background.OtherKey", view.otherKey).arg(view.otherKey);
		}
		break;
	case Banner::NeedsConfig:
		message = QTStrN("Basic.Settings.Hotkeys.Background.NeedsConfig", view.missing.size())
				  .arg(view.missing.size())
				  .arg(name);
		if (view.unverified) {
			message += " " + QTStrN("Basic.Settings.Hotkeys.Background.Unverified", view.unverified)
						 .arg(view.unverified)
						 .arg(name);
		}
		break;
	case Banner::Unverified:
		message = QTStrN("Basic.Settings.Hotkeys.Background.Unverified", view.unverified)
				  .arg(view.unverified)
				  .arg(name);
		break;
	case Banner::PortalRestarting:
		message = QTStr("Basic.Settings.Hotkeys.Background.PortalRestarting");
		break;
	case Banner::NoKeysConfig:
		message = QTStr("Basic.Settings.Hotkeys.Background.NoKeysConfig").arg(name);
		break;
	case Banner::NeedsConfigUnknown:
		message = desktop ? QTStr("Basic.Settings.Hotkeys.Background.NeedsConfigUnread").arg(name)
				  : QTStr("Basic.Settings.Hotkeys.Background.NeedsConfigUnknown");
		break;
	case Banner::Waiting:
		message = QTStr("Basic.Settings.Hotkeys.Background.Waiting");
		break;
	case Banner::Declined:
		message = QTStr("Basic.Settings.Hotkeys.Background.Declined");
		break;
	case Banner::Closed:
		message = QTStr("Basic.Settings.Hotkeys.Background.Closed");
		break;
	case Banner::Unavailable:
		message = QTStr("Basic.Settings.Hotkeys.Background.Unavailable");
		break;
	case Banner::Failed:
		message = QTStr("Basic.Settings.Hotkeys.Background.Failed").arg(QString::fromStdString(input.error));
		break;
	case Banner::AppIdMissing:
		message =
			QTStr("Basic.Settings.Hotkeys.Background.AppIdMissing").arg(QString::fromStdString(input.appId));
		break;
	case Banner::OffBySetting:
		message = QTStr("Basic.Settings.Hotkeys.Background.OffBySetting");
		break;
	case Banner::FocusOnlyKeys:
		message = QTStr("Basic.Settings.Hotkeys.Background.FocusOnlyKeys");
		break;
	case Banner::NoKeys:
		message = QTStr("Basic.Settings.Hotkeys.Background.NoKeys");
		break;
	case Banner::Pending:
		message = QTStr("Basic.Settings.Hotkeys.Background.PendingOnly");
		break;
	}
	if (view.paused && (view.banner == Banner::Working || view.banner == Banner::NeedsConfig ||
			    view.banner == Banner::Unverified)) {
		message += " " + QTStr("Basic.Settings.Hotkeys.Background.Paused");
	}
	if (view.pending && view.banner != Banner::OffBySetting && view.banner != Banner::Pending) {
		message += " " + QTStr("Basic.Settings.Hotkeys.Background.Pending");
	}
	if (view.focusOnlyKeys && view.banner != Banner::FocusOnlyKeys && view.banner != Banner::Unavailable &&
	    view.banner != Banner::OffBySetting) {
		message += " " + QTStr("Basic.Settings.Hotkeys.Background.FocusOnlyKeys");
	}

	copyButton->setVisible(view.copyLines && !testing);
	checkButton->setVisible(view.checkAgain && !testing);
	testButton->setVisible(view.testKey && !testing);
	askButton->setVisible(view.askAgain && !testing);
	configureButton->setVisible(view.configure && !testing);
	cancelButton->setVisible(testing);
	cancelButton->setText(QTStr(configuring ? "Basic.Settings.Hotkeys.Background.Done" : "Cancel"));

	if (testing) {
		QString prompt = QTStr(configuring ? "Basic.Settings.Hotkeys.Background.Configuring"
				       : asking    ? "Basic.Settings.Hotkeys.Background.Asking"
						   : "Basic.Settings.Hotkeys.Background.Testing");
		if (!testResult.isEmpty()) {
			prompt = testResult + "\n" + prompt;
		}
		ShowText(prompt, testResult.isEmpty() ? Level::Info : testLevel);
	} else if (!testResult.isEmpty()) {
		ShowText(testResult + "\n" + message, testLevel);
	} else {
		ShowText(message, view.level);
	}
	emit Refreshed();
}

void OBSHotkeyBackgroundBanner::ShowText(const QString &message, Level level)
{
	/* ● working, ▲ needs the user, ○ for information. */
	static const char *glyphs[] = {"\u25cf", "\u25b2", "\u25cb"};
	static const char *classes[] = {"text-success", "text-warning", "text-muted"};
	static const char *names[] = {"Basic.Settings.Hotkeys.Background.Level.Ok",
				      "Basic.Settings.Hotkeys.Background.Level.Warning",
				      "Basic.Settings.Hotkeys.Background.Level.Info"};
	int i = static_cast<int>(level);
	icon->setText(QString::fromUtf8(glyphs[i]));
	icon->setProperty("class", classes[i]);
	icon->setAccessibleName(QTStr(names[i]));
	bool changed = text->text() != message;
	text->setText(message);
	if (changed) {
		/* Screen readers announce the new status. */
		QAccessibleEvent event(text, QAccessible::NameChanged);
		QAccessible::updateAccessibility(&event);
	}
	icon->style()->unpolish(icon);
	icon->style()->polish(icon);
}

void OBSHotkeyBackgroundBanner::CheckAgain()
{
	testResult.clear();
	Refresh(true);
}

void OBSHotkeyBackgroundBanner::StartTest(bool ask)
{
	testResult.clear();
	testing = true;
	asking = ask;
	obs_hotkeys_set_background_probe(true);
	testTimer->start(ask ? ASK_TIMEOUT_MS : TEST_TIMEOUT_MS);
	Refresh();
}

void OBSHotkeyBackgroundBanner::TestKey()
{
	StartTest(false);
}

void OBSHotkeyBackgroundBanner::AskAgain()
{
	/* Settings keeps background presses off while open; the probe binds
	 * the shortcuts now, so the desktop asks right away. The banner says
	 * to answer its dialog first, and listening goes on while the dialog
	 * is open. */
	obs_hotkeys_retry_background();
	StartTest(true);
}

/* The desktop's editor needs bound shortcuts; Settings keeps background
 * presses off, so the probe binds them while the editor is open. Keys the
 * user changes there arrive as ShortcutsChanged and show in the row
 * tooltips; Done ends it. */
void OBSHotkeyBackgroundBanner::Configure()
{
	/* The request first: it clears a failure left by an earlier one, and
	 * waits for the bind the probe makes. */
	obs_hotkeys_configure_background();
	configuring = true;
	StartTest(false);
}

void OBSHotkeyBackgroundBanner::StopTest()
{
	if (!testing) {
		return;
	}
	testing = false;
	asking = false;
	configuring = false;
	testTimer->stop();
	obs_hotkeys_set_background_probe(false);
	Refresh(true);
}

void OBSHotkeyBackgroundBanner::ProbeReceived(QString id)
{
	if (!testing) {
		return;
	}

	QString key = id;
	for (auto &combo : input.combos) {
		if (QString::fromStdString(combo.id) == id) {
			DStr str;
			obs_key_combination_to_str(combo.combo, str);
			key = QT_UTF8(str);
			break;
		}
	}
	/* It works, whatever OBS can read of the desktop's config. */
	confirmedIds.insert(input.appId + ":" + id.toStdString());
	testResult = QTStr("Basic.Settings.Hotkeys.Background.TestOk").arg(key);
	testLevel = Level::Ok;
	if (configuring) {
		/* Keep the editor's session until Done. */
		Refresh();
		return;
	}
	StopTest();
}

void OBSHotkeyBackgroundBanner::CopyConfigLines()
{
	QDialog dialog(this);
	dialog.setObjectName("hotkeyBackgroundConfigDialog");
	auto layout = new QVBoxLayout(&dialog);

	std::vector<Combo> lines = view.missing.empty() ? input.combos : view.missing;
	auto edit = new QPlainTextEdit(&dialog);
	edit->setObjectName("hotkeyBackgroundConfigLines");
	edit->setReadOnly(true);
	edit->setLineWrapMode(QPlainTextEdit::WidgetWidth);

	if (desktop) {
		dialog.setWindowTitle(QTStr("Basic.Settings.Hotkeys.Background.Config.Title")
					      .arg(QString::fromStdString(desktop->name)));
		QString version = desktop->lua ? QTStr("Basic.Settings.Hotkeys.Background.Config.Lua")
					       : QTStr("Basic.Settings.Hotkeys.Background.Config.Conf");
		auto intro = new QLabel(QTStr("Basic.Settings.Hotkeys.Background.Config.Intro")
						.arg(QString::fromStdString(desktop->configFile), version),
					&dialog);
		intro->setWordWrap(true);
		layout->addWidget(intro);
		layout->addWidget(edit);

		auto nonConsuming =
			new QCheckBox(QTStr("Basic.Settings.Hotkeys.Background.Config.NonConsuming"), &dialog);
		nonConsuming->setObjectName("hotkeyBackgroundNonConsuming");
		nonConsuming->setChecked(true);
		layout->addWidget(nonConsuming);

		auto update = [this, edit, nonConsuming, lines]() {
			edit->setPlainText(QString::fromStdString(
				FormatConfigLines(*desktop, input.appId, lines, nonConsuming->isChecked())));
		};
		connect(nonConsuming, &QCheckBox::toggled, &dialog, update);
		update();
	} else {
		/* A desktop OBS knows nothing about: list the ids to bind. */
		dialog.setWindowTitle(QTStr("Basic.Settings.Hotkeys.Background.CopyLines"));
		auto intro = new QLabel(QTStr("Basic.Settings.Hotkeys.Background.Config.Ids"), &dialog);
		intro->setWordWrap(true);
		layout->addWidget(intro);
		layout->addWidget(edit);
		QString ids;
		for (auto &combo : lines) {
			if (!combo.id.empty()) {
				ids += QString::fromStdString(input.appId + ":" + combo.id) + "\n";
			}
		}
		edit->setPlainText(ids);
	}

	auto hint = new QLabel(QTStr(desktop ? "Basic.Settings.Hotkeys.Background.Config.Hint"
					     : "Basic.Settings.Hotkeys.Background.Config.HintIds"),
			       &dialog);
	hint->setWordWrap(true);
	layout->addWidget(hint);

	auto buttons = new QDialogButtonBox(QDialogButtonBox::Close, &dialog);
	auto copy = buttons->addButton(QTStr("Copy"), QDialogButtonBox::ActionRole);
	copy->setObjectName("hotkeyBackgroundConfigCopy");
	connect(copy, &QPushButton::clicked, &dialog,
		[edit]() { QGuiApplication::clipboard()->setText(edit->toPlainText()); });
	connect(buttons, &QDialogButtonBox::rejected, &dialog, &QDialog::reject);
	layout->addWidget(buttons);

	dialog.resize(760, 300);
	dialog.exec();
}

QString OBSHotkeyBackgroundBanner::MarkerText(Marker marker)
{
	/* ✓ works, ○ focus only, ▲ needs the user, ? unknown, … not handed over
	 * yet. */
	switch (marker) {
	case Marker::None:
		return {};
	case Marker::Background:
		return QString::fromUtf8("\u2713 ") + QTStr("Basic.Settings.Hotkeys.Background.Marker.Background");
	case Marker::FocusOnly:
		return QString::fromUtf8("\u25cb ") + QTStr("Basic.Settings.Hotkeys.Background.Marker.FocusOnly");
	case Marker::NotInConfig:
		return QString::fromUtf8("\u25b2 ") + QTStr("Basic.Settings.Hotkeys.Background.Marker.NotInConfig");
	case Marker::Unverified:
		return "? " + QTStr("Basic.Settings.Hotkeys.Background.Marker.Unverified");
	case Marker::OtherKey:
		return QString::fromUtf8("\u25b2 ") + QTStr("Basic.Settings.Hotkeys.Background.Marker.OtherKey");
	case Marker::Pending:
		return QString::fromUtf8("\u2026 ") + QTStr("Basic.Settings.Hotkeys.Background.Marker.Pending");
	case Marker::NotBound:
		return QString::fromUtf8("\u25b2 ") + QTStr("Basic.Settings.Hotkeys.Background.Marker.NotBound");
	}
	return {};
}

QString OBSHotkeyBackgroundBanner::MarkerToolTip(Marker marker, const Input &input, const std::string &desktopTrigger)
{
	QString desktopName = input.desktop ? QString::fromStdString(input.desktop->name) : QString();
	QString tip;
	switch (marker) {
	case Marker::None:
		return {};
	case Marker::Background:
		tip = QTStr("Basic.Settings.Hotkeys.Background.Marker.Background.ToolTip");
		break;
	case Marker::FocusOnly:
		tip = QTStr("Basic.Settings.Hotkeys.Background.Marker.FocusOnly.ToolTip");
		break;
	case Marker::NotInConfig:
		tip = QTStr("Basic.Settings.Hotkeys.Background.Marker.NotInConfig.ToolTip").arg(desktopName);
		break;
	case Marker::Unverified:
		tip = QTStr("Basic.Settings.Hotkeys.Background.Marker.Unverified.ToolTip").arg(desktopName);
		break;
	case Marker::OtherKey:
		tip = QTStr("Basic.Settings.Hotkeys.Background.Marker.OtherKey.ToolTip");
		break;
	case Marker::Pending:
		tip = QTStr("Basic.Settings.Hotkeys.Background.Marker.Pending.ToolTip");
		break;
	case Marker::NotBound:
		tip = QTStr("Basic.Settings.Hotkeys.Background.Marker.NotBound.ToolTip");
		break;
	}
	if (!desktopTrigger.empty()) {
		tip += "\n" + QTStr("Basic.Settings.Hotkeys.Background.Marker.DesktopTrigger")
				      .arg(QString::fromStdString(desktopTrigger));
	}
	return tip;
}
