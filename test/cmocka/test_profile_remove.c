#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "profile-remove.h"

/* Upstream obsproject/obs-websocket#1267 (reported by BarryCarlyon):
 * removing the current profile sends CurrentProfileChanging and
 * CurrentProfileChanged but no ProfileListChanged, while removing any other
 * profile sends ProfileListChanged. Through obs-websocket's RemoveProfile
 * (obs_frontend_delete_profile()) it is worse: the request succeeds, but the
 * frontend opens the "Remove profile?" confirmation on the OBS machine and
 * removes nothing until someone there clicks Yes.
 *
 * The model below stands in for OBSBasic:
 * - confirm() is the confirmation dialog; it counts how often it is shown and
 *   answers with what the case sets;
 * - switch_and_remove_current() takes the current profile out of the list and
 *   activates the first one left (OBSBasic::ActivateProfile, which emits
 *   OBS_FRONTEND_EVENT_PROFILE_CHANGED);
 * - remove_other() takes another profile out of the list;
 * - the event callbacks record each event with the number of profiles listed
 *   at that moment, which is what obs-websocket's ProfileListChanged
 *   reports. */

#define MAX_PROFILES 4
#define MAX_EVENTS 8

enum event { EV_CHANGING, EV_CHANGED, EV_LIST_CHANGED };

struct frontend_model {
	const char *list[MAX_PROFILES];
	int list_count;
	const char *current;
	const char *removing; /* the profile the frontend was asked to remove */

	bool answer_yes;
	int confirm_shown;

	int events;
	enum event event[MAX_EVENTS];
	int event_list_count[MAX_EVENTS];

	struct profile_remove_ops ops;
};

static void record(struct frontend_model *m, enum event e)
{
	if (m->events < MAX_EVENTS) {
		m->event[m->events] = e;
		m->event_list_count[m->events] = m->list_count;
	}
	m->events++;
}

static void take_from_list(struct frontend_model *m, const char *name)
{
	for (int i = 0; i < m->list_count; i++) {
		if (strcmp(m->list[i], name) == 0) {
			memmove(&m->list[i], &m->list[i + 1], sizeof(m->list[0]) * (size_t)(m->list_count - i - 1));
			m->list_count--;
			return;
		}
	}
}

static bool listed(const struct frontend_model *m, const char *name)
{
	for (int i = 0; i < m->list_count; i++) {
		if (strcmp(m->list[i], name) == 0) {
			return true;
		}
	}
	return false;
}

static bool model_confirm(void *param)
{
	struct frontend_model *m = param;
	m->confirm_shown++;
	return m->answer_yes;
}

static void model_profile_changing(void *param)
{
	record(param, EV_CHANGING);
}

static void model_switch_and_remove_current(void *param)
{
	struct frontend_model *m = param;
	take_from_list(m, m->current);
	m->current = m->list[0];
	record(m, EV_CHANGED); /* ActivateProfile */
}

static void model_remove_other(void *param)
{
	struct frontend_model *m = param;
	take_from_list(m, m->removing);
}

static void model_profile_list_changed(void *param)
{
	record(param, EV_LIST_CHANGED);
}

static void model_init(struct frontend_model *m, const char *removing, bool answer_yes)
{
	memset(m, 0, sizeof(*m));
	m->list[m->list_count++] = "Untitled";
	m->list[m->list_count++] = "Gaming";
	m->list[m->list_count++] = "Podcast";
	m->current = "Podcast";
	m->removing = removing;
	m->answer_yes = answer_yes;
	m->ops.param = m;
	m->ops.confirm = model_confirm;
	m->ops.profile_changing = model_profile_changing;
	m->ops.switch_and_remove_current = model_switch_and_remove_current;
	m->ops.remove_other = model_remove_other;
	m->ops.profile_list_changed = model_profile_list_changed;
}

static int count_events(const struct frontend_model *m, enum event e)
{
	int n = 0;
	for (int i = 0; i < m->events && i < MAX_EVENTS; i++) {
		if (m->event[i] == e) {
			n++;
		}
	}
	return n;
}

static int last_index_of(const struct frontend_model *m, enum event e)
{
	int idx = -1;
	for (int i = 0; i < m->events && i < MAX_EVENTS; i++) {
		if (m->event[i] == e) {
			idx = i;
		}
	}
	return idx;
}

/* obs-websocket RemoveProfile on the current profile: the client asked for
 * it, so nobody at the OBS machine is asked, the profile is gone when the
 * request has been handled, and ProfileListChanged reports the shorter
 * list. */
static void test_api_remove_current(void **state)
{
	(void)state;
	struct frontend_model m;
	/* Nobody answers the dialog on a remote-controlled or headless OBS. */
	model_init(&m, "Podcast", false);

	profile_remove_by_api(&m.ops, true);

	printf("API remove of the current profile: confirmation shown %d time(s), still listed: %s, "
	       "ProfileListChanged: %d\n",
	       m.confirm_shown, listed(&m, "Podcast") ? "yes" : "no", count_events(&m, EV_LIST_CHANGED));

	assert_int_equal(m.confirm_shown, 0);
	assert_false(listed(&m, "Podcast"));
	assert_int_equal(count_events(&m, EV_CHANGING), 1);
	assert_int_equal(count_events(&m, EV_CHANGED), 1);
	assert_int_equal(count_events(&m, EV_LIST_CHANGED), 1);
	assert_int_equal(m.event_list_count[last_index_of(&m, EV_LIST_CHANGED)], 2);
}

/* Profile -> Remove on the current profile, confirmed: the profile list
 * changed, so it is announced, after the switch to the next profile. */
static void test_ui_remove_current(void **state)
{
	(void)state;
	struct frontend_model m;
	model_init(&m, "Podcast", true);

	profile_remove_current(&m.ops, true);

	printf("UI remove of the current profile: confirmation shown %d time(s), ProfileListChanged: %d\n",
	       m.confirm_shown, count_events(&m, EV_LIST_CHANGED));

	assert_int_equal(m.confirm_shown, 1);
	assert_false(listed(&m, "Podcast"));
	assert_int_equal(count_events(&m, EV_LIST_CHANGED), 1);
	assert_true(last_index_of(&m, EV_LIST_CHANGED) > last_index_of(&m, EV_CHANGED));
	assert_int_equal(m.event_list_count[last_index_of(&m, EV_LIST_CHANGED)], 2);
}

/* Guard: Profile -> Remove, answered No: nothing changes, nothing is
 * announced. */
static void test_ui_remove_current_cancelled(void **state)
{
	(void)state;
	struct frontend_model m;
	model_init(&m, "Podcast", false);

	profile_remove_current(&m.ops, true);

	assert_int_equal(m.confirm_shown, 1);
	assert_true(listed(&m, "Podcast"));
	assert_string_equal(m.current, "Podcast");
	assert_int_equal(m.events, 0);
}

/* Guard: removing a profile that is not in use through the API: no question,
 * no profile switch, one ProfileListChanged with the shorter list. */
static void test_api_remove_other(void **state)
{
	(void)state;
	struct frontend_model m;
	model_init(&m, "Gaming", false);

	profile_remove_by_api(&m.ops, false);

	assert_int_equal(m.confirm_shown, 0);
	assert_false(listed(&m, "Gaming"));
	assert_string_equal(m.current, "Podcast");
	assert_int_equal(count_events(&m, EV_CHANGING), 0);
	assert_int_equal(count_events(&m, EV_LIST_CHANGED), 1);
	assert_int_equal(m.event_list_count[0], 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_api_remove_current),
		cmocka_unit_test(test_ui_remove_current),
		cmocka_unit_test(test_ui_remove_current_cancelled),
		cmocka_unit_test(test_api_remove_other),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
