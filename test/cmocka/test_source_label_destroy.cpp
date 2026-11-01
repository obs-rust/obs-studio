/* The source name label in the Sources dock (OBSSourceLabel, used by every
 * SourceTreeItem) and its source being destroyed at the same time.
 *
 * Quitting OBS (or switching scene collections) could crash on the main
 * thread in signal_handler_disconnect(), called from ~OBSSourceLabel() inside
 * ~SourceTreeItem(): the item releases its scene item first, which can drop
 * the last reference to the source, so libobs destroys the source on its
 * destruction thread while the main thread is still deleting the item's
 * child widgets, the label among them. The source's "destroy" signal ran
 * OBSSourceLabel::obsSourceDestroyed() on the destruction thread, which
 * disconnected the label's three OBSSignal members there. Each connection
 * holds a reference on the source's signal handler (keep_ref), so those
 * disconnects dropped the last references and the handler was freed, while
 * the main thread had already read the handler from the same OBSSignal and
 * was about to disconnect from it: a use after free.
 *
 * The test builds the real label (frontend/components/OBSSourceLabel.cpp.hpp)
 * on Qt's offscreen platform, against a real libobs signal handler that plays
 * the source's: obs_source_get_name() and obs_source_get_signal_handler() are
 * wrapped (--wrap) to return it, so no obs_startup() and no display are
 * needed. signal_handler_connect_ref() and signal_handler_disconnect() are
 * wrapped too: the test counts the handler's references the way libobs does
 * (one for the source, one per connection) and so knows when it has been
 * freed. A disconnect from a freed handler is counted and not passed on, so
 * the test fails instead of crashing.
 *
 * "Destroying the source" is what obs_source_destroy_defer() does on the
 * destruction thread: emit "destroy", then drop the source's reference to
 * its signal handler (signal_handler_destroy()). */

#include <QApplication>
#include <QObject>

#include <obs.h>
#include <obs.hpp>

#include "OBSSourceLabel.hpp"

#include <pthread.h>
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
extern "C" {
#include <cmocka.h>
}

/* ---- the source: a real signal handler behind a fake source pointer ---- */

static int fake_source_storage;
static obs_source_t *const fake_source = reinterpret_cast<obs_source_t *>(&fake_source_storage);
static signal_handler_t *handler;

extern "C" {
const char *__real_obs_source_get_name(const obs_source_t *source);
signal_handler_t *__real_obs_source_get_signal_handler(const obs_source_t *source);
void __real_signal_handler_connect_ref(signal_handler_t *h, const char *signal, signal_callback_t callback, void *data);
void __real_signal_handler_disconnect(signal_handler_t *h, const char *signal, signal_callback_t callback, void *data);

EXPORT const char *__wrap_obs_source_get_name(const obs_source_t *source);
EXPORT signal_handler_t *__wrap_obs_source_get_signal_handler(const obs_source_t *source);
EXPORT void __wrap_signal_handler_connect_ref(signal_handler_t *h, const char *signal, signal_callback_t callback,
					      void *data);
EXPORT void __wrap_signal_handler_disconnect(signal_handler_t *h, const char *signal, signal_callback_t callback,
					     void *data);

const char *__wrap_obs_source_get_name(const obs_source_t *source)
{
	return source == fake_source ? "Cam0" : __real_obs_source_get_name(source);
}

signal_handler_t *__wrap_obs_source_get_signal_handler(const obs_source_t *source)
{
	return source == fake_source ? handler : __real_obs_source_get_signal_handler(source);
}
}

/* ---- the handler's references, counted like libobs does ---------------- */

#define MAX_CONNECTIONS 8

struct connection {
	const char *signal;
	signal_callback_t callback;
	void *data;
};

static pthread_mutex_t model_mutex = PTHREAD_MUTEX_INITIALIZER;
static signal_handler_t *counted; /* the handler being counted */
static long refs;                 /* 0: freed */
static struct connection conns[MAX_CONNECTIONS];
static size_t num_conns;
static bool source_ref_held;
static pthread_t main_thread;
static int disconnects_off_main;   /* by the label, on another thread */
static int disconnects_after_free; /* from a freed handler */

/* Case 2: hold the main thread's first disconnect from the handler while the
 * source is destroyed on the other thread. */
static bool hold_first_disconnect;
static void (*while_held)(void);

static void start_counting(signal_handler_t *h)
{
	pthread_mutex_lock(&model_mutex);
	counted = h;
	refs = 1; /* the source's own reference */
	source_ref_held = true;
	num_conns = 0;
	disconnects_off_main = 0;
	disconnects_after_free = 0;
	pthread_mutex_unlock(&model_mutex);
}

/* The source drops its reference: signal_handler_destroy(). */
static void drop_source_ref(void)
{
	pthread_mutex_lock(&model_mutex);
	refs--;
	source_ref_held = false;
	pthread_mutex_unlock(&model_mutex);
	signal_handler_destroy(handler);
}

extern "C" void __wrap_signal_handler_connect_ref(signal_handler_t *h, const char *signal, signal_callback_t callback,
						  void *data)
{
	pthread_mutex_lock(&model_mutex);
	if (h && h == counted && num_conns < MAX_CONNECTIONS) {
		conns[num_conns++] = {signal, callback, data};
		refs++;
	}
	pthread_mutex_unlock(&model_mutex);
	__real_signal_handler_connect_ref(h, signal, callback, data);
}

extern "C" void __wrap_signal_handler_disconnect(signal_handler_t *h, const char *signal, signal_callback_t callback,
						 void *data)
{
	if (h && h == counted && hold_first_disconnect && pthread_equal(pthread_self(), main_thread)) {
		hold_first_disconnect = false;
		while_held();
	}

	pthread_mutex_lock(&model_mutex);
	if (h && h == counted) {
		if (!pthread_equal(pthread_self(), main_thread)) {
			disconnects_off_main++;
		}
		if (refs == 0) {
			disconnects_after_free++;
			pthread_mutex_unlock(&model_mutex);
			return;
		}
		for (size_t i = 0; i < num_conns; i++) {
			if (strcmp(conns[i].signal, signal) == 0 && conns[i].callback == callback &&
			    conns[i].data == data) {
				conns[i] = conns[--num_conns];
				refs--;
				break;
			}
		}
	}
	pthread_mutex_unlock(&model_mutex);
	__real_signal_handler_disconnect(h, signal, callback, data);
}

static long current_refs(void)
{
	pthread_mutex_lock(&model_mutex);
	long r = refs;
	pthread_mutex_unlock(&model_mutex);
	return r;
}

/* ---- destroying the source on its own thread --------------------------- */

static void *destroy_source_thread(void *)
{
	calldata_t cd = {};
	calldata_set_ptr(&cd, "source", fake_source);
	signal_handler_signal(handler, "destroy", &cd);
	calldata_free(&cd);

	drop_source_ref();
	return nullptr;
}

static void destroy_source_on_another_thread(void)
{
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, nullptr, destroy_source_thread, nullptr), 0);
	pthread_join(thread, nullptr);
}

static int setup(void **)
{
	handler = signal_handler_create();
	if (!handler) {
		return -1;
	}
	signal_handler_add(handler, "void rename(ptr source, string new_name, string prev_name)");
	signal_handler_add(handler, "void remove(ptr source)");
	signal_handler_add(handler, "void destroy(ptr source)");
	start_counting(handler);
	return 0;
}

static int teardown(void **)
{
	/* If the source was never destroyed, it still holds its reference. */
	if (source_ref_held) {
		drop_source_ref();
	}
	pthread_mutex_lock(&model_mutex);
	counted = nullptr;
	pthread_mutex_unlock(&model_mutex);
	handler = nullptr;
	return 0;
}

/* ---- cases ------------------------------------------------------------- */

/* Guard: the label follows a rename of its source. */
static void label_follows_rename(void **)
{
	OBSSourceLabel *label = new OBSSourceLabel(fake_source);
	assert_string_equal(label->text().toUtf8().constData(), "Cam0");

	calldata_t cd = {};
	calldata_set_ptr(&cd, "source", fake_source);
	calldata_set_string(&cd, "new_name", "Front camera");
	calldata_set_string(&cd, "prev_name", "Cam0");
	signal_handler_signal(handler, "rename", &cd);
	calldata_free(&cd);

	assert_string_equal(label->text().toUtf8().constData(), "Front camera");

	delete label;
	assert_int_equal(current_refs(), 1);
}

/* The source is destroyed on libobs's destruction thread while the label
 * lives on the main thread: the label still reports it (Settings relies on
 * OBSSourceLabel::destroyed), but leaves its connections to the main thread,
 * which owns them. Disconnecting them on the destruction thread races with
 * the main thread deleting the label (case 2). */
static void destroy_leaves_connections_to_the_label(void **)
{
	OBSSourceLabel *label = new OBSSourceLabel(fake_source);
	int destroyed = 0;
	QObject::connect(
		label, &OBSSourceLabel::destroyed, label, [&destroyed]() { destroyed++; }, Qt::DirectConnection);

	destroy_source_on_another_thread();

	assert_int_equal(destroyed, 1);
	if (disconnects_off_main) {
		print_error("the label disconnected %d of its signals on the destruction thread\n",
			    disconnects_off_main);
		fail();
	}

	delete label;
	assert_int_equal(disconnects_after_free, 0);
	/* Every reference is gone once the label is: nothing leaks. */
	assert_int_equal(current_refs(), 0);
}

/* What crashed on quit: the main thread starts deleting the label (it has
 * read the signal handler from the label's first OBSSignal), and the source
 * is destroyed on the destruction thread before that disconnect runs. */
static void destroy_source_now(void)
{
	destroy_source_on_another_thread();
}

static void label_deleted_while_its_source_is_destroyed(void **)
{
	OBSSourceLabel *label = new OBSSourceLabel(fake_source);

	while_held = destroy_source_now;
	hold_first_disconnect = true;
	delete label;
	hold_first_disconnect = false;

	if (disconnects_after_free) {
		print_error("the main thread disconnected %d time(s) from the source's signal handler "
			    "after it was freed\n",
			    disconnects_after_free);
		fail();
	}
	assert_int_equal(current_refs(), 0);
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);
	main_thread = pthread_self();

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(label_follows_rename, setup, teardown),
		cmocka_unit_test_setup_teardown(destroy_leaves_connections_to_the_label, setup, teardown),
		cmocka_unit_test_setup_teardown(label_deleted_while_its_source_is_destroyed, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
