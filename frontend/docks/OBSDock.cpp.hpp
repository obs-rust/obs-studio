#include "frontend/build/frontend-docks.pch.hpp"

void OBSDock::closeEvent(QCloseEvent *event)
{
	/* A floating main dock goes back into the main window, without the notice
	 * below (obs-rust/obs-studio#5408). */
	if (CloseRedocksMainDock(this, event, OBSBasic::Get()->isClosing())) {
		return;
	}

	auto msgBox = []() {
		QMessageBox msgbox(App()->GetMainWindow());
		msgbox.setWindowTitle(QTStr("DockCloseWarning.Title"));
		msgbox.setText(QTStr("DockCloseWarning.Text"));
		msgbox.setIcon(QMessageBox::Icon::Information);
		msgbox.addButton(QMessageBox::Ok);

		QCheckBox *cb = new QCheckBox(QTStr("DoNotShowAgain"));
		msgbox.setCheckBox(cb);

		msgbox.exec();

		if (cb->isChecked()) {
			config_set_bool(App()->GetUserConfig(), "General", "WarnedAboutClosingDocks", true);
			config_save_safe(App()->GetUserConfig(), "tmp", nullptr);
		}
	};

	bool warned = config_get_bool(App()->GetUserConfig(), "General", "WarnedAboutClosingDocks");
	if (!OBSBasic::Get()->isClosing() && !warned) {
		QMetaObject::invokeMethod(App(), &OBSApp::Exec, Qt::QueuedConnection, msgBox);
	}

	QDockWidget::closeEvent(event);

	if (widget() && event->isAccepted()) {
		QEvent widgetEvent(QEvent::Type(QEvent::User + QEvent::Close));
		qApp->sendEvent(widget(), &widgetEvent);
	}
}

void OBSDock::showEvent(QShowEvent *event)
{
	QDockWidget::showEvent(event);
}
