/* A properties refresh while a field's dialog is open
 * (obsproject/obs-studio#10423). The Browse button of a path field, the
 * font and color buttons and the editable list's add/edit buttons open a
 * modal dialog, which runs its own event loop. If the source refreshes its
 * properties meanwhile (obs_source_update_properties, which plugins and
 * scripts call from a timer), the view rebuilt every field and deleted the
 * field's widgets and its WidgetInfo under the open dialog. Picking a file
 * (or font, color, entry) then wrote to freed memory and OBS crashed.
 *
 * Builds the real OBSPropertiesView on Qt's offscreen platform (Qt's own
 * dialogs, no display) and, while the dialog is open, reloads the view the
 * way the Properties window does on a source's "update_properties" signal.
 * Then it picks a value in the dialog. Built with ASan, so a freed widget
 * fails the test; it also checks that the picked value reached the settings
 * and the rebuilt field. */

#include <QApplication>
#include <QColorDialog>
#include <QDir>
#include <QFile>
#include <QFileDialog>
#include <QFontDialog>
#include <QLineEdit>
#include <QListWidget>
#include <QPointer>
#include <QPushButton>
#include <QTemporaryDir>
#include <QTest>
#include <QTimer>

#include <obs.hpp>

#include "properties-view.hpp"

#include <functional>
#include <memory>
#include <string>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

/* properties-view picks its help icon by theme. */
extern "C" bool obs_frontend_is_theme_dark(void)
{
	return false;
}

enum class Field { Path, Font, Color, StringList, FileList };

static Field field;
static int dummy_object;
static QTemporaryDir *tmp;

static obs_properties_t *test_properties(void *)
{
	obs_properties_t *props = obs_properties_create();
	switch (field) {
	case Field::Path:
		obs_properties_add_path(props, "file", "File", OBS_PATH_FILE, "All files (*)", nullptr);
		break;
	case Field::Font:
		obs_properties_add_font(props, "font", "Font");
		break;
	case Field::Color:
		obs_properties_add_color(props, "color", "Color");
		break;
	case Field::StringList:
		obs_properties_add_editable_list(props, "list", "List", OBS_EDITABLE_LIST_TYPE_STRINGS, nullptr,
						 nullptr);
		break;
	case Field::FileList:
		obs_properties_add_editable_list(props, "list", "List", OBS_EDITABLE_LIST_TYPE_FILES, "All files (*)",
						 nullptr);
		break;
	}
	return props;
}

static void no_update(void *, obs_data_t *, obs_data_t *) {}

struct Fixture {
	OBSDataAutoRelease settings = obs_data_create();
	OBSPropertiesView *view = nullptr;
	int refreshes = 0;
};

static Fixture *make_view(Field f, obs_data_t *initial = nullptr)
{
	field = f;
	auto *fx = new Fixture;
	if (initial) {
		obs_data_apply(fx->settings, initial);
	}
	fx->view = new OBSPropertiesView(fx->settings.Get(), (void *)&dummy_object, test_properties, no_update);
	fx->view->show();
	/* The view builds its widgets from a queued call. */
	QTest::qWait(50);
	QObject::connect(fx->view, &OBSPropertiesView::PropertiesRefreshed, [fx]() { fx->refreshes++; });
	return fx;
}

static int teardown(void **state)
{
	auto *fx = static_cast<Fixture *>(*state);
	if (fx) {
		delete fx->view;
		delete fx;
	}
	*state = nullptr;
	return 0;
}

static const char *utf8(const QString &str)
{
	static std::string buf[2];
	static int next = 0;
	std::string &b = buf[next++ % 2];
	b = str.toStdString();
	return b.c_str();
}

static QString make_file(const char *name)
{
	QString path = QDir(tmp->path()).filePath(name);
	QFile f(path);
	if (f.open(QIODevice::WriteOnly)) {
		f.write("x");
		f.close();
	}
	return path;
}

/* Waits for the modal dialog of type T, refreshes the view the way the
 * Properties window does when the source emits "update_properties" (a
 * queued ReloadProperties), lets the event loop of the dialog run the
 * refresh, then hands the dialog to pick(). refresh = false is the control:
 * the same pick without a refresh. */
template<typename T> static void when_dialog_opens(OBSPropertiesView *view, bool refresh, std::function<void(T *)> pick)
{
	QPointer<OBSPropertiesView> v(view);
	auto *poll = new QTimer;
	poll->setInterval(20);
	auto found = std::make_shared<bool>(false);
	QObject::connect(poll, &QTimer::timeout, poll, [poll, found, v, refresh, pick]() {
		T *dlg = qobject_cast<T *>(QApplication::activeModalWidget());
		if (*found || !dlg || !dlg->isVisible()) {
			return;
		}
		*found = true;
		poll->deleteLater();
		if (refresh && v) {
			QMetaObject::invokeMethod(v, &OBSPropertiesView::ReloadProperties, Qt::QueuedConnection);
		}
		QPointer<T> d(dlg);
		/* 200 ms of the dialog's own event loop: the refresh (and the
		 * deferred deletes of the old widgets) run in there. */
		QTimer::singleShot(200, [d, pick]() {
			if (d) {
				pick(d);
			}
		});
	});
	poll->start();
	/* Never hang the run: close whatever this case left open after 10 s. */
	static int armed = 0;
	int mine = ++armed;
	QTimer::singleShot(10000, [mine]() {
		if (mine != armed) {
			return;
		}
		if (QWidget *w = QApplication::activeModalWidget()) {
			w->close();
		}
	});
}

/* What a user does in Qt's file dialog: type the full path into the file
 * name box and accept. (selectFile() leaves the box alone while it has the
 * focus.) */
static void type_file_name(QFileDialog *d, const QString &path)
{
	QLineEdit *name = d->findChild<QLineEdit *>("fileNameEdit");
	if (name) {
		name->setText(path);
	}
	static_cast<QDialog *>(d)->accept();
}

static QPushButton *find_button(OBSPropertiesView *view, const QString &textOrIcon)
{
	for (QPushButton *b : view->findChildren<QPushButton *>()) {
		if (b->text() == textOrIcon || b->property("class").toString().endsWith(textOrIcon)) {
			return b;
		}
	}
	return nullptr;
}

/* Clicks a field button; the dialog it opens runs inside click(). */
static void click(QPushButton *button)
{
	assert_non_null(button);
	button->click();
	/* Let the refresh that was held for the dialog (if any) run. */
	QTest::qWait(100);
}

static void path_pick(void **state, bool refresh)
{
	Fixture *fx = make_view(Field::Path);
	*state = fx;
	QString picked = make_file("picked.txt");

	when_dialog_opens<QFileDialog>(fx->view, refresh, [picked](QFileDialog *d) { type_file_name(d, picked); });
	click(find_button(fx->view, "Browse"));

	assert_string_equal(obs_data_get_string(fx->settings, "file"), utf8(picked));
	/* The refresh still ran, and the rebuilt field shows the file. */
	assert_int_equal(fx->refreshes, refresh ? 1 : 0);
	QList<QLineEdit *> edits = fx->view->findChildren<QLineEdit *>();
	assert_int_equal(edits.size(), 1);
	assert_string_equal(utf8(edits[0]->text()), utf8(picked));
}

/* The issue's steps: Browse, a refresh while the file dialog is open, pick a file. */
static void path_refresh_while_browsing(void **state)
{
	path_pick(state, true);
}

/* Control: Browse and pick a file without a refresh. */
static void path_no_refresh(void **state)
{
	path_pick(state, false);
}

static void font_refresh_while_open(void **state)
{
	Fixture *fx = make_view(Field::Font);
	*state = fx;

	when_dialog_opens<QFontDialog>(fx->view, true, [](QFontDialog *d) {
		QFont font = d->currentFont();
		font.setPointSize(37);
		d->setCurrentFont(font);
		static_cast<QDialog *>(d)->accept();
	});
	click(find_button(fx->view, "Basic.PropertiesWindow.SelectFont"));

	OBSDataAutoRelease font = obs_data_get_obj(fx->settings, "font");
	assert_non_null(font.Get());
	assert_int_equal(obs_data_get_int(font, "size"), 37);
	assert_int_equal(fx->refreshes, 1);
}

static void color_refresh_while_open(void **state)
{
	Fixture *fx = make_view(Field::Color);
	*state = fx;

	when_dialog_opens<QColorDialog>(fx->view, true, [](QColorDialog *d) {
		d->setCurrentColor(QColor(10, 20, 30));
		static_cast<QDialog *>(d)->accept();
	});
	click(find_button(fx->view, "Basic.PropertiesWindow.SelectColor"));

	/* 0xAABBGGRR, opaque */
	assert_int_equal(obs_data_get_int(fx->settings, "color"), 0xFF1E140A);
	assert_int_equal(fx->refreshes, 1);
}

static const char *list_value(obs_data_t *settings, size_t idx)
{
	OBSDataArrayAutoRelease array = obs_data_get_array(settings, "list");
	if (idx >= obs_data_array_count(array)) {
		return "";
	}
	OBSDataAutoRelease item = obs_data_array_item(array, idx);
	static std::string value;
	value = obs_data_get_string(item, "value");
	return value.c_str();
}

/* The editable list's "+" (strings list): the entry dialog. */
static void list_add_refresh_while_open(void **state)
{
	Fixture *fx = make_view(Field::StringList);
	*state = fx;

	when_dialog_opens<QDialog>(fx->view, true, [](QDialog *d) {
		QLineEdit *edit = d->findChild<QLineEdit *>();
		if (edit) {
			edit->setText("entry");
		}
		static_cast<QDialog *>(d)->accept();
	});
	click(find_button(fx->view, "icon-plus"));

	assert_string_equal(list_value(fx->settings, 0), "entry");
	assert_int_equal(fx->refreshes, 1);
	QListWidget *list = fx->view->findChild<QListWidget *>();
	assert_non_null(list);
	assert_int_equal(list->count(), 1);
}

/* The editable list's edit button on a file entry: the Browse file dialog. */
static void list_edit_file_refresh_while_open(void **state)
{
	QString first = make_file("first.txt");
	QString second = make_file("second.txt");
	OBSDataAutoRelease initial = obs_data_create();
	OBSDataArrayAutoRelease array = obs_data_array_create();
	OBSDataAutoRelease item = obs_data_create();
	obs_data_set_string(item, "value", utf8(first));
	obs_data_array_push_back(array, item);
	obs_data_set_array(initial, "list", array);

	Fixture *fx = make_view(Field::FileList, initial);
	*state = fx;
	QListWidget *list = fx->view->findChild<QListWidget *>();
	assert_non_null(list);
	assert_int_equal(list->count(), 1);
	list->item(0)->setSelected(true);

	when_dialog_opens<QFileDialog>(fx->view, true, [second](QFileDialog *d) { type_file_name(d, second); });
	click(find_button(fx->view, "icon-gear"));

	assert_string_equal(list_value(fx->settings, 0), utf8(second));
	assert_int_equal(fx->refreshes, 1);
	/* The refresh rebuilt the list from the settings. */
	list = fx->view->findChild<QListWidget *>();
	assert_non_null(list);
	assert_int_equal(list->count(), 1);
	assert_string_equal(utf8(list->item(0)->text()), utf8(second));
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);
	QTemporaryDir dir;
	tmp = &dir;

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(path_refresh_while_browsing, teardown),
		cmocka_unit_test_teardown(path_no_refresh, teardown),
		cmocka_unit_test_teardown(font_refresh_while_open, teardown),
		cmocka_unit_test_teardown(color_refresh_while_open, teardown),
		cmocka_unit_test_teardown(list_add_refresh_while_open, teardown),
		cmocka_unit_test_teardown(list_edit_file_refresh_while_open, teardown),
	};
	/* ASan stops at the first freed widget: run one case with its name. */
	if (argc > 1) {
		cmocka_set_test_filter(argv[1]);
	}
	return cmocka_run_group_tests(tests, NULL, NULL);
}
