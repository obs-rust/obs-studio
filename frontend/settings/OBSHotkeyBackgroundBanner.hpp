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

#pragma once

#include "OBSHotkeyBackground.hpp"

#include <obs.hpp>

#include <QFrame>
#include <QPointer>

#include <functional>

class QLabel;
class QPushButton;
class QTimer;

/* The status line at the top of Settings -> Hotkeys on desktops where
 * background hotkeys go through the desktop (Wayland). Hidden when OBS reads
 * the keyboard itself. */
class OBSHotkeyBackgroundBanner : public QFrame {
	Q_OBJECT

public:
	/* combos: every key combination set in Settings right now (saved or
	 * not); offBySetting: the hotkey focus setting turns background
	 * hotkeys off. */
	OBSHotkeyBackgroundBanner(QWidget *parent, std::function<std::vector<obs_key_combination_t>()> combos,
				  std::function<bool()> offBySetting);
	~OBSHotkeyBackgroundBanner();

	const HotkeyBackground::Input &CurrentInput() const { return input; }

	/* Text and tooltip of a row marker. */
	static QString MarkerText(HotkeyBackground::Marker marker);
	static QString MarkerToolTip(HotkeyBackground::Marker marker, const HotkeyBackground::Input &input,
				     const std::string &desktopTrigger);

public slots:
	/* Reads the status again (and the desktop's config, if readConfig). */
	void Refresh(bool readConfig = false);

signals:
	/* The status changed: markers must be updated. */
	void Refreshed();

private slots:
	void CopyConfigLines();
	void CheckAgain();
	void TestKey();
	void AskAgain();
	void StopTest();
	void ProbeReceived(QString id);

private:
	static void StatusChanged(void *data, calldata_t *params);
	static void Probe(void *data, calldata_t *params);

	void ReadDesktopConfig();
	void DesktopConfigRead(std::optional<HotkeyBackground::DesktopConfig> found);
	void StartTest(bool ask);
	void ShowText(const QString &text, HotkeyBackground::Level level);

	std::function<std::vector<obs_key_combination_t>()> combos;
	std::function<bool()> offBySetting;

	HotkeyBackground::Input input;
	HotkeyBackground::View view;
	std::optional<HotkeyBackground::DesktopConfig> desktop;

	QPointer<QLabel> icon;
	QPointer<QLabel> text;
	QPointer<QPushButton> copyButton;
	QPointer<QPushButton> checkButton;
	QPointer<QPushButton> testButton;
	QPointer<QPushButton> askButton;
	QPointer<QPushButton> cancelButton;
	QPointer<QTimer> testTimer;
	bool testing = false;
	/* "Ask again": the desktop may show its dialog first. */
	bool asking = false;
	bool readingConfig = false;
	bool configRead = false;
	/* Keys "Test a key" received this session. */
	std::set<std::string> confirmedIds;
	QString testResult;
	HotkeyBackground::Level testLevel = HotkeyBackground::Level::Info;

	OBSSignal statusSignal;
	OBSSignal probeSignal;
};
