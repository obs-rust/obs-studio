#pragma once

#include <utility/dock-window.hpp>

#include <QDockWidget>

class QCloseEvent;
class QShowEvent;
class QString;

class OBSDock : public QDockWidget {
	Q_OBJECT

public:
	inline OBSDock(QWidget *parent = nullptr) : QDockWidget(parent) { SetUpDockWindow(this); }
	inline OBSDock(const QString &title, QWidget *parent = nullptr) : QDockWidget(title, parent)
	{
		SetUpDockWindow(this);
	}

	virtual void closeEvent(QCloseEvent *event);
	virtual void showEvent(QShowEvent *event);
};
