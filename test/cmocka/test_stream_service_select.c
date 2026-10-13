#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include "stream-service-select.h"

/* Settings > Stream: choosing "Show all..." in the Service list reloads the
 * list with every service and keeps the selected service. Nothing changed, but
 * Apply became enabled and closing Settings asked to save unsaved changes.
 *
 * The model below stands in for the Service list and the settings dialog. */

#define SHOW_ALL "Show all..."

struct service_model {
	const char *selected; /* the entry the list shows */
	const char *service;  /* the service the fields were last updated for */
	bool expanded;        /* the list holds every service */
	int show_all_calls;
	int service_changed_calls;
	bool changed; /* Apply enabled, unsaved changes on close */
	struct stream_service_select_ops ops;
};

static bool model_is_show_all(void *param)
{
	return strcmp(((struct service_model *)param)->selected, SHOW_ALL) == 0;
}

/* LoadServices(true): the full list has no "Show all..." entry and selects
 * the service that was selected before. */
static void model_show_all(void *param)
{
	struct service_model *m = param;
	m->show_all_calls++;
	m->expanded = true;
	m->selected = m->service;
}

static void model_service_changed(void *param)
{
	struct service_model *m = param;
	m->service_changed_calls++;
	m->service = m->selected;
}

static void model_mark_changed(void *param)
{
	((struct service_model *)param)->changed = true;
}

/* Settings opened on the Stream page with a common service. */
static void model_open(struct service_model *m, const char *service)
{
	memset(m, 0, sizeof(*m));
	m->selected = service;
	m->service = service;
	m->ops.param = m;
	m->ops.is_show_all = model_is_show_all;
	m->ops.show_all = model_show_all;
	m->ops.service_changed = model_service_changed;
	m->ops.mark_changed = model_mark_changed;
}

static void model_select(struct service_model *m, const char *entry)
{
	m->selected = entry;
	stream_service_selected(&m->ops);
}

/* "Show all..." then closing the list without picking anything: the service
 * is the same, so there is nothing to apply or save. */
static void show_all_is_not_a_change(void **state)
{
	(void)state;
	struct service_model m;
	model_open(&m, "Twitch");

	model_select(&m, SHOW_ALL);

	assert_true(m.expanded);
	assert_int_equal(m.show_all_calls, 1);
	assert_string_equal(m.selected, "Twitch");
	assert_string_equal(m.service, "Twitch");
	assert_int_equal(m.service_changed_calls, 0);
	assert_false(m.changed);
}

/* Picking another service from the expanded list is a change. */
static void service_from_full_list_is_a_change(void **state)
{
	(void)state;
	struct service_model m;
	model_open(&m, "Twitch");

	model_select(&m, SHOW_ALL);
	model_select(&m, "Some Other Service");

	assert_int_equal(m.service_changed_calls, 1);
	assert_string_equal(m.service, "Some Other Service");
	assert_true(m.changed);
}

/* Picking another common service without expanding the list is a change. */
static void common_service_is_a_change(void **state)
{
	(void)state;
	struct service_model m;
	model_open(&m, "Twitch");

	model_select(&m, "YouTube - RTMPS");

	assert_int_equal(m.show_all_calls, 0);
	assert_int_equal(m.service_changed_calls, 1);
	assert_string_equal(m.service, "YouTube - RTMPS");
	assert_true(m.changed);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(show_all_is_not_a_change),
		cmocka_unit_test(service_from_full_list_is_a_change),
		cmocka_unit_test(common_service_is_a_change),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
