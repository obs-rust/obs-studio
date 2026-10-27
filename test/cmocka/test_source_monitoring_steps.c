#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <math.h>
#include <float.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-monitoring.h"

/*
 * Characterization of the monitoring, monitor-only mute, balance, mixer,
 * flag, push-to and audio active rules of the sources in
 * libobs/obs-source.c, moved to libobs/obs-source-monitoring.c before their
 * Rust port:
 * - obs_source_monitoring_changes(): the same-value rule of
 *   obs_source_set_flags(), obs_source_set_audio_mixers(),
 *   obs_source_set_monitoring_enabled() and the push-to enables;
 * - obs_source_monitoring_has_audio() and _mixers_store(): the audio gate of
 *   the mixers and the value a handler leaves;
 * - obs_source_monitoring_push_to_logs(): the push-to log rule;
 * - obs_source_monitoring_on_type(), _stored_type() and _monitor(): the
 *   deprecated and stored monitoring types (#2447) and the monitor route;
 * - obs_source_monitoring_route(), _off_reads_mute(), _off_unmutes() and
 *   _only_mutes(): the steps of obs_source_set_monitoring_type() (#2444,
 *   #2447);
 * - obs_source_monitoring_balance(): clamp_balance() (#2051);
 * - obs_source_monitoring_audio_active(): the signal of
 *   obs_source_set_audio_active().
 * Each is pinned over every combination of its flags and over the edges of
 * its numbers. The last cases run copies of the callers on a stand-in source
 * that records its signals, logs, mute calls and monitor, so the
 * composition is pinned too: obs_source_set_monitoring_type() for every type
 * from every start state, the deprecated type on the compatibility signal,
 * the balance and mixers a handler leaves, the push-to enables and the
 * audio active signal.
 *
 * Characterized, not endorsed:
 * - the mixers a handler leaves above UINT32_MAX or below 0 are truncated to
 *   their low 32 bits;
 * - a balance of -0.0 stays -0.0 (it is not below 0.0f);
 * - Monitor and Output unmutes a mute the user set, while none keeps it.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_source_monitoring_type,
 * test_source_monitor_only_load and test_source_audio_balance_mono cover
 * the callers through the real libobs.
 */

#define KEEP OBS_SOURCE_MONITORING_MONITOR_KEEP
#define CREATE OBS_SOURCE_MONITORING_MONITOR_CREATE
#define DESTROY OBS_SOURCE_MONITORING_MONITOR_DESTROY

#define ROUTE_OFF OBS_SOURCE_MONITORING_ROUTE_OFF
#define ROUTE_ONLY OBS_SOURCE_MONITORING_ROUTE_MONITOR_ONLY
#define ROUTE_BOTH OBS_SOURCE_MONITORING_ROUTE_MONITOR_AND_OUTPUT
#define ROUTE_ON OBS_SOURCE_MONITORING_ROUTE_ON

#define ACT_NONE OBS_SOURCE_MONITORING_ACTIVE_NONE
#define ACT_ON OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE
#define ACT_OFF OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE

#define T_NONE OBS_SOURCE_MONITORING_TYPE_NONE
#define T_ONLY OBS_SOURCE_MONITORING_TYPE_MONITOR_ONLY
#define T_BOTH OBS_SOURCE_MONITORING_TYPE_MONITOR_AND_OUTPUT

static uint32_t bits(float f)
{
	uint32_t b;
	memcpy(&b, &f, sizeof(b));
	return b;
}

static float from_bits(uint32_t b)
{
	float f;
	memcpy(&f, &b, sizeof(f));
	return f;
}

static void constants(void **state)
{
	(void)state;

	/* the header's mirror of libobs */
	assert_int_equal(OBS_SOURCE_MONITORING_AUDIO, OBS_SOURCE_AUDIO);
	assert_int_equal(T_NONE, (int)OBS_MONITORING_TYPE_NONE);
	assert_int_equal(T_ONLY, (int)OBS_MONITORING_TYPE_MONITOR_ONLY);
	assert_int_equal(T_BOTH, (int)OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT);
	assert_int_equal(sizeof(enum obs_monitoring_type), sizeof(int));
	assert_int_equal((int)KEEP, 0);
	assert_int_equal((int)CREATE, 1);
	assert_int_equal((int)DESTROY, 2);
	assert_int_equal((int)ROUTE_OFF, 0);
	assert_int_equal((int)ROUTE_ONLY, 1);
	assert_int_equal((int)ROUTE_BOTH, 2);
	assert_int_equal((int)ROUTE_ON, 3);
	assert_int_equal((int)ACT_NONE, 0);
	assert_int_equal((int)ACT_ON, 1);
	assert_int_equal((int)ACT_OFF, 2);
}

static const uint32_t U32S[] = {0,    1,     2,           3,           0x7f,        0x80,
				0xff, 0x100, 0x7fffffffu, 0x80000000u, 0xfffffffeu, 0xffffffffu};
#define N_U32S (sizeof(U32S) / sizeof(U32S[0]))

static void changes(void **state)
{
	(void)state;

	for (size_t i = 0; i < N_U32S; i++) {
		for (size_t j = 0; j < N_U32S; j++)
			assert_int_equal(obs_source_monitoring_changes(U32S[i], U32S[j]), i != j);
	}

	/* the bools of the push-to enables and monitoring_enabled */
	assert_false(obs_source_monitoring_changes(false, false));
	assert_true(obs_source_monitoring_changes(false, true));
	assert_true(obs_source_monitoring_changes(true, false));
	assert_false(obs_source_monitoring_changes(true, true));
}

static void has_audio(void **state)
{
	(void)state;

	assert_false(obs_source_monitoring_has_audio(0));
	assert_true(obs_source_monitoring_has_audio(OBS_SOURCE_AUDIO));
	assert_true(obs_source_monitoring_has_audio(0xffffffffu));
	assert_false(obs_source_monitoring_has_audio(~(uint32_t)OBS_SOURCE_AUDIO));
	for (int bit = 0; bit < 32; bit++)
		assert_int_equal(obs_source_monitoring_has_audio((uint32_t)1 << bit), bit == 1);
	assert_false(obs_source_monitoring_has_audio(OBS_SOURCE_VIDEO | OBS_SOURCE_ASYNC));
	assert_true(obs_source_monitoring_has_audio(OBS_SOURCE_VIDEO | OBS_SOURCE_AUDIO));
}

static void mixers_store(void **state)
{
	(void)state;

	assert_int_equal(obs_source_monitoring_mixers_store(0), 0);
	assert_int_equal(obs_source_monitoring_mixers_store(1), 1);
	assert_int_equal(obs_source_monitoring_mixers_store(0x3f), 0x3f);
	assert_int_equal(obs_source_monitoring_mixers_store(0xffffffffLL), 0xffffffffu);

	/* characterized, not endorsed: the low 32 bits */
	assert_int_equal(obs_source_monitoring_mixers_store(0x100000000LL), 0);
	assert_int_equal(obs_source_monitoring_mixers_store(0x100000005LL), 5);
	assert_int_equal(obs_source_monitoring_mixers_store(-1), 0xffffffffu);
	assert_int_equal(obs_source_monitoring_mixers_store(-2), 0xfffffffeu);
	assert_int_equal(obs_source_monitoring_mixers_store(INT64_MIN), 0);
	assert_int_equal(obs_source_monitoring_mixers_store(INT64_MAX), 0xffffffffu);
	assert_int_equal(obs_source_monitoring_mixers_store((int64_t)INT32_MIN), 0x80000000u);
}

static void push_to_logs(void **state)
{
	(void)state;

	for (size_t i = 0; i < N_U32S; i++) {
		bool audio = (U32S[i] & OBS_SOURCE_AUDIO) != 0;

		assert_false(obs_source_monitoring_push_to_logs(U32S[i], false));
		assert_int_equal(obs_source_monitoring_push_to_logs(U32S[i], true), audio);
	}
	assert_true(obs_source_monitoring_push_to_logs(OBS_SOURCE_AUDIO, true));
	assert_false(obs_source_monitoring_push_to_logs(OBS_SOURCE_VIDEO, true));
}

static void monitoring_types(void **state)
{
	(void)state;

	/* on and muted is Monitor Only (#2447) */
	assert_int_equal(obs_source_monitoring_on_type(true), T_ONLY);
	assert_int_equal(obs_source_monitoring_on_type(false), T_BOTH);

	/* the stored type: on is Monitor and Output */
	assert_int_equal(obs_source_monitoring_stored_type(true), T_BOTH);
	assert_int_equal(obs_source_monitoring_stored_type(false), T_NONE);

	/* the monitor */
	assert_int_equal(obs_source_monitoring_monitor(false, false), KEEP);
	assert_int_equal(obs_source_monitoring_monitor(false, true), CREATE);
	assert_int_equal(obs_source_monitoring_monitor(true, false), DESTROY);
	assert_int_equal(obs_source_monitoring_monitor(true, true), KEEP);
}

static void route(void **state)
{
	(void)state;

	assert_int_equal(obs_source_monitoring_route(T_NONE), ROUTE_OFF);
	assert_int_equal(obs_source_monitoring_route(T_ONLY), ROUTE_ONLY);
	assert_int_equal(obs_source_monitoring_route(T_BOTH), ROUTE_BOTH);
	assert_int_equal(obs_source_monitoring_route(3), ROUTE_ON);
	assert_int_equal(obs_source_monitoring_route(4), ROUTE_ON);
	assert_int_equal(obs_source_monitoring_route(-1), ROUTE_ON);
	assert_int_equal(obs_source_monitoring_route(INT32_MIN), ROUTE_ON);
	assert_int_equal(obs_source_monitoring_route(INT32_MAX), ROUTE_ON);
	for (int t = -1000; t <= 1000; t++) {
		int want = t == 0 ? ROUTE_OFF : t == 1 ? ROUTE_ONLY : t == 2 ? ROUTE_BOTH : ROUTE_ON;

		assert_int_equal(obs_source_monitoring_route(t), want);
	}

	/* the mute steps */
	assert_false(obs_source_monitoring_off_reads_mute(false));
	assert_true(obs_source_monitoring_off_reads_mute(true));
	assert_false(obs_source_monitoring_off_unmutes(false));
	assert_true(obs_source_monitoring_off_unmutes(true));
	assert_true(obs_source_monitoring_only_mutes(false));
	assert_false(obs_source_monitoring_only_mutes(true));
}

static void balance(void **state)
{
	(void)state;

	/* NaN is centred: quiet, negative and signalling */
	assert_int_equal(bits(obs_source_monitoring_balance(NAN)), bits(0.5f));
	assert_int_equal(bits(obs_source_monitoring_balance(-NAN)), bits(0.5f));
	assert_int_equal(bits(obs_source_monitoring_balance(from_bits(0x7f800001u))), bits(0.5f));
	assert_int_equal(bits(obs_source_monitoring_balance(from_bits(0xffbfffffu))), bits(0.5f));

	/* in range as it is */
	assert_int_equal(bits(obs_source_monitoring_balance(0.0f)), bits(0.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(0.25f)), bits(0.25f));
	assert_int_equal(bits(obs_source_monitoring_balance(0.5f)), bits(0.5f));
	assert_int_equal(bits(obs_source_monitoring_balance(1.0f)), bits(1.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(from_bits(1))), 1);
	assert_int_equal(bits(obs_source_monitoring_balance(nextafterf(1.0f, 0.0f))), bits(nextafterf(1.0f, 0.0f)));

	/* characterized, not endorsed: -0.0 is not below 0.0f, so it stays */
	assert_int_equal(bits(obs_source_monitoring_balance(-0.0f)), 0x80000000u);

	/* out of range clamped */
	assert_int_equal(bits(obs_source_monitoring_balance(nextafterf(1.0f, 2.0f))), bits(1.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(2.0f)), bits(1.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(FLT_MAX)), bits(1.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(INFINITY)), bits(1.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(from_bits(0x80000001u))), bits(0.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(-1.0f)), bits(0.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(-FLT_MAX)), bits(0.0f));
	assert_int_equal(bits(obs_source_monitoring_balance(-INFINITY)), bits(0.0f));
}

static void audio_active(void **state)
{
	(void)state;

	assert_int_equal(obs_source_monitoring_audio_active(false, false), ACT_NONE);
	assert_int_equal(obs_source_monitoring_audio_active(false, true), ACT_ON);
	assert_int_equal(obs_source_monitoring_audio_active(true, false), ACT_OFF);
	assert_int_equal(obs_source_monitoring_audio_active(true, true), ACT_NONE);
}

/* A stand-in source and copies of the callers, as obs-source.c has them,
 * on it: every signal, log, mute call and monitor is recorded. */

struct stand_in {
	bool valid;
	uint32_t flags;
	uint32_t audio_mixers;
	bool owns_info_id;
	uint32_t output_flags;
	bool push_to_mute_enabled;
	bool push_to_talk_enabled;
	bool monitoring_enabled;
	int monitoring_type;
	bool user_muted;
	bool monitor;
	bool monitor_only_muted;
	float balance;
	bool audio_active;

	/* what a handler leaves */
	bool handler_sets;
	int64_t handler_mixers;
	double handler_balance;

	/* recorded */
	char trace[512];
	int signals;
	int logs;
	int64_t compat_type;
	int64_t sent_mixers;
	double sent_balance;
};

static void record(struct stand_in *s, const char *what)
{
	strncat(s->trace, what, sizeof(s->trace) - strlen(s->trace) - 1);
	strncat(s->trace, ";", sizeof(s->trace) - strlen(s->trace) - 1);
}

static bool si_muted(struct stand_in *s)
{
	return s->valid ? s->user_muted : false;
}

static void si_set_muted(struct stand_in *s, bool muted)
{
	if (!s->valid)
		return;
	record(s, muted ? "mute" : "unmute");
	s->user_muted = muted;
	/* a mute set by anything else is not Monitor Only's to undo (#2447) */
	s->monitor_only_muted = false;
	s->signals++;
}

static void si_set_flags(struct stand_in *s, uint32_t flags)
{
	if (!s->valid)
		return;

	if (obs_source_monitoring_changes(s->flags, flags)) {
		s->flags = flags;
		record(s, "update_flags");
		s->signals++;
	}
}

static void si_set_audio_mixers(struct stand_in *s, uint32_t mixers)
{
	if (!s->valid)
		return;
	if (!s->owns_info_id && !obs_source_monitoring_has_audio(s->output_flags))
		return;

	if (!obs_source_monitoring_changes(s->audio_mixers, mixers))
		return;

	int64_t value = mixers;
	s->sent_mixers = value;
	record(s, "audio_mixers");
	s->signals++;
	if (s->handler_sets)
		value = s->handler_mixers;

	mixers = obs_source_monitoring_mixers_store(value);

	s->audio_mixers = mixers;
}

static uint32_t si_get_audio_mixers(struct stand_in *s)
{
	if (!s->valid)
		return 0;
	if (!s->owns_info_id && !obs_source_monitoring_has_audio(s->output_flags))
		return 0;

	return s->audio_mixers;
}

static void si_enable_push_to_mute(struct stand_in *s, bool enabled)
{
	if (!s->valid)
		return;

	bool changed = obs_source_monitoring_changes(s->push_to_mute_enabled, enabled);
	if (obs_source_monitoring_push_to_logs(s->output_flags, changed)) {
		record(s, "log");
		s->logs++;
	}

	s->push_to_mute_enabled = enabled;

	if (changed) {
		record(s, "push_to_mute_changed");
		s->signals++;
	}
}

static void si_enable_push_to_talk(struct stand_in *s, bool enabled)
{
	if (!s->valid)
		return;

	bool changed = obs_source_monitoring_changes(s->push_to_talk_enabled, enabled);
	if (obs_source_monitoring_push_to_logs(s->output_flags, changed)) {
		record(s, "log");
		s->logs++;
	}

	s->push_to_talk_enabled = enabled;

	if (changed) {
		record(s, "push_to_talk_changed");
		s->signals++;
	}
}

static void si_set_monitoring_enabled(struct stand_in *s, bool enabled)
{
	int compat_type;
	bool was_on;

	if (!s->valid)
		return;

	if (!obs_source_monitoring_changes(s->monitoring_enabled, enabled))
		return;

	compat_type = T_NONE;
	if (enabled)
		compat_type = obs_source_monitoring_on_type(s->user_muted);

	record(s, enabled ? "monitor_on" : "monitor_off");
	s->signals++;
	s->compat_type = compat_type;
	s->signals++;

	was_on = s->monitoring_enabled;
	s->monitoring_enabled = enabled;
	s->monitoring_type = obs_source_monitoring_stored_type(enabled);

	switch (obs_source_monitoring_monitor(was_on, enabled)) {
	case CREATE:
		record(s, "create");
		s->monitor = true;
		break;
	case DESTROY:
		record(s, "destroy");
		s->monitor = false;
		break;
	case KEEP:
		break;
	}
}

static void si_set_monitoring_type(struct stand_in *s, int type)
{
	bool muted_for_monitor_only;

	if (!s->valid)
		return;

	record(s, "deprecated");
	s->logs++;

	muted_for_monitor_only = s->monitor_only_muted;

	switch (obs_source_monitoring_route(type)) {
	case ROUTE_OFF:
		si_set_monitoring_enabled(s, false);
		if (obs_source_monitoring_off_reads_mute(muted_for_monitor_only) &&
		    obs_source_monitoring_off_unmutes(si_muted(s)))
			si_set_muted(s, false);
		s->monitor_only_muted = false;
		break;
	case ROUTE_ONLY:
		if (obs_source_monitoring_only_mutes(si_muted(s))) {
			si_set_muted(s, true);
			muted_for_monitor_only = true;
		}
		si_set_monitoring_enabled(s, true);
		s->monitor_only_muted = muted_for_monitor_only;
		break;
	case ROUTE_BOTH:
		si_set_muted(s, false);
		si_set_monitoring_enabled(s, true);
		break;
	case ROUTE_ON:
		si_set_monitoring_enabled(s, true);
		break;
	}
}

static int si_get_monitoring_type(struct stand_in *s)
{
	if (!s->valid || !s->monitoring_enabled)
		return T_NONE;

	return obs_source_monitoring_on_type(s->user_muted);
}

static void si_set_balance_value(struct stand_in *s, float balance)
{
	if (s->valid) {
		double value = obs_source_monitoring_balance(balance);

		s->sent_balance = value;
		record(s, "audio_balance");
		s->signals++;
		if (s->handler_sets)
			value = s->handler_balance;

		s->balance = obs_source_monitoring_balance((float)value);
	}
}

static void si_set_audio_active(struct stand_in *s, bool active)
{
	bool prev;

	if (!s->valid)
		return;

	prev = s->audio_active;
	s->audio_active = active;
	switch (obs_source_monitoring_audio_active(prev, active)) {
	case ACT_ON:
		record(s, "audio_activate");
		s->signals++;
		break;
	case ACT_OFF:
		record(s, "audio_deactivate");
		s->signals++;
		break;
	case ACT_NONE:
		break;
	}
}

static struct stand_in source(void)
{
	struct stand_in s;

	memset(&s, 0, sizeof(s));
	s.valid = true;
	s.output_flags = OBS_SOURCE_AUDIO;
	s.balance = 0.5f;
	s.compat_type = -1;
	return s;
}

/* obs_source_set_monitoring_type() from every start state: monitoring on or
 * off, muted or not, Monitor Only's mark set or not. */
static void monitoring_type_steps(void **state)
{
	static const int types[] = {T_NONE, T_ONLY, T_BOTH, 3, -1};
	(void)state;

	for (size_t t = 0; t < sizeof(types) / sizeof(types[0]); t++) {
		for (int start = 0; start < 8; start++) {
			struct stand_in s = source();
			bool on = (start & 1) != 0;
			bool muted = (start & 2) != 0;
			bool mark = (start & 4) != 0;
			int type = types[t];
			char want[512] = "deprecated;";

			s.monitoring_enabled = on;
			s.monitor = on;
			s.monitoring_type = on ? T_BOTH : T_NONE;
			s.user_muted = muted;
			s.monitor_only_muted = mark;

			si_set_monitoring_type(&s, type);

			if (type == T_NONE) {
				/* off, then the mute Monitor Only set goes, any
				 * other stays; the mark is cleared */
				if (on)
					strcat(want, "monitor_off;destroy;");
				if (mark && muted)
					strcat(want, "unmute;");
				assert_string_equal(s.trace, want);
				assert_false(s.monitoring_enabled);
				assert_false(s.monitor);
				assert_int_equal(s.monitoring_type, T_NONE);
				assert_int_equal(s.user_muted, muted && !mark);
				assert_false(s.monitor_only_muted);
				if (on)
					assert_int_equal(s.compat_type, T_NONE);
			} else if (type == T_ONLY) {
				/* the mute first, then on; the mark says whether
				 * this call muted it (an earlier mark stays) */
				if (!muted)
					strcat(want, "mute;");
				if (!on)
					strcat(want, "monitor_on;create;");
				assert_string_equal(s.trace, want);
				assert_true(s.monitoring_enabled);
				assert_true(s.monitor);
				assert_int_equal(s.monitoring_type, T_BOTH);
				assert_true(s.user_muted);
				assert_int_equal(s.monitor_only_muted, !muted || mark);
				if (!on)
					assert_int_equal(s.compat_type, T_ONLY);
			} else if (type == T_BOTH) {
				/* characterized, not endorsed: the unmute takes a
				 * mute the user set too */
				strcat(want, "unmute;");
				if (!on)
					strcat(want, "monitor_on;create;");
				assert_string_equal(s.trace, want);
				assert_true(s.monitoring_enabled);
				assert_true(s.monitor);
				assert_int_equal(s.monitoring_type, T_BOTH);
				assert_false(s.user_muted);
				assert_false(s.monitor_only_muted);
				if (!on)
					assert_int_equal(s.compat_type, T_BOTH);
			} else {
				/* not a type: on, the mute and the mark as they
				 * were */
				if (!on)
					strcat(want, "monitor_on;create;");
				assert_string_equal(s.trace, want);
				assert_true(s.monitoring_enabled);
				assert_true(s.monitor);
				assert_int_equal(s.user_muted, muted);
				assert_int_equal(s.monitor_only_muted, mark);
				if (!on)
					assert_int_equal(s.compat_type, muted ? T_ONLY : T_BOTH);
			}

			/* the getter: off is none, on and muted Monitor Only */
			assert_int_equal(si_get_monitoring_type(&s), !s.monitoring_enabled ? T_NONE
								     : s.user_muted        ? T_ONLY
											   : T_BOTH);
		}
	}

	/* Monitor Only, then none: the mute it set goes */
	struct stand_in s = source();
	si_set_monitoring_type(&s, T_ONLY);
	assert_true(s.user_muted);
	assert_true(s.monitor_only_muted);
	si_set_monitoring_type(&s, T_NONE);
	assert_false(s.user_muted);
	assert_false(s.monitoring_enabled);

	/* Monitor Only, the user mutes again, then none: the mute stays */
	s = source();
	si_set_monitoring_type(&s, T_ONLY);
	si_set_muted(&s, true);
	si_set_monitoring_type(&s, T_NONE);
	assert_true(s.user_muted);

	/* an invalid source: nothing */
	s = source();
	s.valid = false;
	si_set_monitoring_type(&s, T_ONLY);
	assert_string_equal(s.trace, "");
	assert_int_equal(si_get_monitoring_type(&s), T_NONE);
}

/* obs_source_set_monitoring_enabled(): the same value does nothing, and the
 * compatibility signal's type. */
static void monitoring_enabled_steps(void **state)
{
	(void)state;

	for (int start = 0; start < 4; start++) {
		bool on = (start & 1) != 0;
		bool muted = (start & 2) != 0;

		for (int e = 0; e < 2; e++) {
			struct stand_in s = source();
			bool enabled = e != 0;

			s.monitoring_enabled = on;
			s.monitor = on;
			s.user_muted = muted;
			si_set_monitoring_enabled(&s, enabled);

			if (on == enabled) {
				assert_string_equal(s.trace, "");
				assert_int_equal(s.signals, 0);
				assert_int_equal(s.compat_type, -1);
				continue;
			}
			assert_int_equal(s.signals, 2);
			assert_int_equal(s.compat_type, !enabled ? T_NONE : muted ? T_ONLY : T_BOTH);
			assert_int_equal(s.monitoring_type, enabled ? T_BOTH : T_NONE);
			assert_int_equal(s.monitor, enabled);
			assert_string_equal(s.trace, enabled ? "monitor_on;create;" : "monitor_off;destroy;");
			/* the mute is not touched */
			assert_int_equal(s.user_muted, muted);
		}
	}
}

static void balance_steps(void **state)
{
	static const float handler[] = {NAN, 2.0f, -1.0f, 0.75f, -0.0f};
	static const float want[] = {0.5f, 1.0f, 0.0f, 0.75f, -0.0f};
	(void)state;

	/* the value sent is clamped */
	struct stand_in s = source();
	si_set_balance_value(&s, NAN);
	assert_int_equal(bits((float)s.sent_balance), bits(0.5f));
	assert_int_equal(bits(s.balance), bits(0.5f));
	si_set_balance_value(&s, 3.0f);
	assert_int_equal(bits((float)s.sent_balance), bits(1.0f));
	assert_int_equal(bits(s.balance), bits(1.0f));
	si_set_balance_value(&s, -INFINITY);
	assert_int_equal(bits((float)s.sent_balance), bits(0.0f));
	assert_int_equal(bits(s.balance), bits(0.0f));

	/* and again the value a handler leaves (#2051) */
	for (size_t i = 0; i < sizeof(handler) / sizeof(handler[0]); i++) {
		s = source();
		s.handler_sets = true;
		s.handler_balance = handler[i];
		si_set_balance_value(&s, 0.25f);
		assert_int_equal(bits((float)s.sent_balance), bits(0.25f));
		assert_int_equal(bits(s.balance), bits(want[i]));
		assert_int_equal(s.signals, 1);
	}

	/* an invalid source: nothing */
	s = source();
	s.valid = false;
	si_set_balance_value(&s, 0.0f);
	assert_int_equal(s.signals, 0);
	assert_int_equal(bits(s.balance), bits(0.5f));
}

static void mixers_steps(void **state)
{
	(void)state;

	/* an audio source: the same value does nothing */
	struct stand_in s = source();
	s.audio_mixers = 0x3f;
	si_set_audio_mixers(&s, 0x3f);
	assert_int_equal(s.signals, 0);
	si_set_audio_mixers(&s, 0x5);
	assert_int_equal(s.signals, 1);
	assert_int_equal(s.sent_mixers, 0x5);
	assert_int_equal(s.audio_mixers, 0x5);
	assert_int_equal(si_get_audio_mixers(&s), 0x5);

	/* a handler's value, truncated: characterized, not endorsed */
	s = source();
	s.handler_sets = true;
	s.handler_mixers = 0x100000003LL;
	si_set_audio_mixers(&s, 1);
	assert_int_equal(s.audio_mixers, 3);
	s.handler_mixers = -1;
	si_set_audio_mixers(&s, 1);
	assert_int_equal(s.audio_mixers, 0xffffffffu);
	assert_int_equal(s.signals, 2);

	/* a video-only source: no mixers */
	s = source();
	s.output_flags = OBS_SOURCE_VIDEO;
	s.audio_mixers = 0x3f;
	si_set_audio_mixers(&s, 1);
	assert_int_equal(s.signals, 0);
	assert_int_equal(s.audio_mixers, 0x3f);
	assert_int_equal(si_get_audio_mixers(&s), 0);

	/* a video-only source that owns its info id: mixers */
	s.owns_info_id = true;
	si_set_audio_mixers(&s, 1);
	assert_int_equal(s.signals, 1);
	assert_int_equal(si_get_audio_mixers(&s), 1);

	/* an invalid source */
	s = source();
	s.valid = false;
	s.audio_mixers = 7;
	si_set_audio_mixers(&s, 1);
	assert_int_equal(s.audio_mixers, 7);
	assert_int_equal(si_get_audio_mixers(&s), 0);
}

static void flags_steps(void **state)
{
	(void)state;

	struct stand_in s = source();
	si_set_flags(&s, 0);
	assert_int_equal(s.signals, 0);
	si_set_flags(&s, OBS_SOURCE_FLAG_FORCE_MONO);
	assert_int_equal(s.signals, 1);
	assert_int_equal(s.flags, OBS_SOURCE_FLAG_FORCE_MONO);
	si_set_flags(&s, OBS_SOURCE_FLAG_FORCE_MONO);
	assert_int_equal(s.signals, 1);
	si_set_flags(&s, 0xffffffffu);
	assert_int_equal(s.signals, 2);
}

static void push_to_steps(void **state)
{
	(void)state;

	/* the same value: stored, no signal, no log */
	struct stand_in s = source();
	si_enable_push_to_mute(&s, false);
	si_enable_push_to_talk(&s, false);
	assert_string_equal(s.trace, "");

	/* a change on an audio source: log, then signal */
	si_enable_push_to_mute(&s, true);
	assert_string_equal(s.trace, "log;push_to_mute_changed;");
	assert_true(s.push_to_mute_enabled);
	si_enable_push_to_talk(&s, true);
	assert_string_equal(s.trace, "log;push_to_mute_changed;log;push_to_talk_changed;");
	assert_true(s.push_to_talk_enabled);
	si_enable_push_to_mute(&s, true);
	assert_int_equal(s.signals, 2);
	assert_int_equal(s.logs, 2);

	/* a video-only source: the signal, no log */
	s = source();
	s.output_flags = OBS_SOURCE_VIDEO;
	si_enable_push_to_mute(&s, true);
	si_enable_push_to_talk(&s, true);
	assert_string_equal(s.trace, "push_to_mute_changed;push_to_talk_changed;");
	assert_int_equal(s.logs, 0);
}

static void audio_active_steps(void **state)
{
	(void)state;

	struct stand_in s = source();
	si_set_audio_active(&s, true);
	si_set_audio_active(&s, true);
	assert_string_equal(s.trace, "audio_activate;");
	si_set_audio_active(&s, false);
	si_set_audio_active(&s, false);
	assert_string_equal(s.trace, "audio_activate;audio_deactivate;");
	assert_int_equal(s.signals, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(changes),
		cmocka_unit_test(has_audio),
		cmocka_unit_test(mixers_store),
		cmocka_unit_test(push_to_logs),
		cmocka_unit_test(monitoring_types),
		cmocka_unit_test(route),
		cmocka_unit_test(balance),
		cmocka_unit_test(audio_active),
		cmocka_unit_test(monitoring_type_steps),
		cmocka_unit_test(monitoring_enabled_steps),
		cmocka_unit_test(balance_steps),
		cmocka_unit_test(mixers_steps),
		cmocka_unit_test(flags_steps),
		cmocka_unit_test(push_to_steps),
		cmocka_unit_test(audio_active_steps),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
