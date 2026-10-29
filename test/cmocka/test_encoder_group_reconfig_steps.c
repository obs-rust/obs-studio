#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-encoder-group-reconfig.h"

/*
 * Characterization of the encoder group reconfigure rules of
 * libobs/obs-encoder.c, moved to libobs/obs-encoder-group-reconfig.c before
 * their Rust port (#2913):
 * - obs_encoder_group_reconfig_update(): the group branch of
 *   obs_encoder_update() for an active encoder;
 * - obs_encoder_group_reconfig_new_request():
 *   encoder_group_new_reconfigure_request();
 * - obs_encoder_group_reconfig_frame():
 *   handle_encoder_group_reconfigure_request(), for each video frame of a
 *   grouped encoder;
 * - obs_encoder_group_reconfig_lcm(): gcd() and lcm() of add_connection();
 * - obs_encoder_group_reconfig_reset(): remove_connection() once the group is
 *   idle.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. obs-encoder.c keeps the
 * group mutex and copies the fields in and out. test_encoder_group_reconfig_walk
 * covers the callers through the real libobs.
 *
 * An encoder's cur_pts counts in units of 1 / timebase_den and steps by
 * timebase_num * frame_rate_divisor for each frame it encodes. Not covered,
 * undefined in C: an LCM above INT_MAX, a divisor of 0 or above INT_MAX, a
 * timestep of 0 and next_pts beyond int64_t.
 */

#define NTSC_NUM 1001 /* timebase_num of 29.97 fps */

/* ------------------------------------------------------------------------ */
/* helpers */

static struct obs_encoder_group_reconfig group_of(uint64_t request, int64_t next_pts, uint32_t updated,
						  uint32_t reconfigured, bool again)
{
	struct obs_encoder_group_reconfig group = {
		.reconfigure_request = request,
		.next_pts = next_pts,
		.encoders_updated_next_pts = updated,
		.encoders_reconfigured = reconfigured,
		.reconfigure_again = again,
	};
	return group;
}

static void check_group(const char *what, const struct obs_encoder_group_reconfig *got,
			const struct obs_encoder_group_reconfig *want)
{
	if (got->reconfigure_request != want->reconfigure_request || got->next_pts != want->next_pts ||
	    got->encoders_updated_next_pts != want->encoders_updated_next_pts ||
	    got->encoders_reconfigured != want->encoders_reconfigured ||
	    got->reconfigure_again != want->reconfigure_again) {
		fprintf(stderr,
			"%s: request %llu next %lld updated %u reconfigured %u again %d; "
			"expected %llu %lld %u %u %d\n",
			what, (unsigned long long)got->reconfigure_request, (long long)got->next_pts,
			got->encoders_updated_next_pts, got->encoders_reconfigured, got->reconfigure_again,
			(unsigned long long)want->reconfigure_request, (long long)want->next_pts,
			want->encoders_updated_next_pts, want->encoders_reconfigured, want->reconfigure_again);
		fail();
	}
}

static void check_encoder(const char *what, const struct obs_encoder_reconfig *got, uint64_t last, uint64_t handled)
{
	if (got->last_reconfigure_request != last || got->last_handled_reconfigure_request != handled) {
		fprintf(stderr, "%s: last %llu handled %llu; expected %llu %llu\n", what,
			(unsigned long long)got->last_reconfigure_request,
			(unsigned long long)got->last_handled_reconfigure_request, (unsigned long long)last,
			(unsigned long long)handled);
		fail();
	}
}

static void check_u32(const char *what, uint32_t got, uint32_t want)
{
	if (got != want) {
		fprintf(stderr, "%s: %u; expected %u\n", what, got, want);
		fail();
	}
}

/* A group of grouped encoders on one video, as receive_video() drives them:
 * each video frame, every encoder whose divisor the frame count meets asks
 * frame() at its cur_pts, in the group's order, then encodes (cur_pts steps). */
#define MAX_SIM 4
#define MAX_APPLIED 8

struct sim_encoder {
	struct obs_encoder_reconfig reconfig;
	uint32_t divisor;
	int64_t cur_pts;
	size_t num_applied;
	int64_t applied[MAX_APPLIED];
};

struct sim {
	struct obs_encoder_group_reconfig group;
	uint32_t lcm;
	uint32_t timebase_num;
	size_t num;
	struct sim_encoder encoders[MAX_SIM];
};

static void sim_init(struct sim *sim, uint32_t timebase_num, size_t num, const uint32_t *divisors)
{
	*sim = (struct sim){0};
	sim->timebase_num = timebase_num;
	sim->num = num;
	sim->lcm = 1;
	for (size_t i = 0; i < num; i++) {
		sim->encoders[i].divisor = divisors[i];
		sim->lcm = obs_encoder_group_reconfig_lcm(sim->lcm, divisors[i]);
	}
}

static void sim_update(struct sim *sim, size_t idx)
{
	obs_encoder_group_reconfig_update(&sim->group, sim->num,
					  sim->encoders[idx].reconfig.last_handled_reconfigure_request);
}

static void sim_frame(struct sim *sim, uint64_t frame)
{
	for (size_t i = 0; i < sim->num; i++) {
		struct sim_encoder *enc = &sim->encoders[i];
		if (frame % enc->divisor != 0)
			continue;
		if (obs_encoder_group_reconfig_frame(&sim->group, sim->num, (uint32_t)sim->num, sim->lcm,
						     &enc->reconfig, enc->cur_pts, sim->timebase_num, enc->divisor)) {
			assert_true(enc->num_applied < MAX_APPLIED);
			enc->applied[enc->num_applied++] = enc->cur_pts;
		}
		enc->cur_pts += (int64_t)sim->timebase_num * enc->divisor;
	}
}

static void sim_frames(struct sim *sim, uint64_t from, uint64_t to)
{
	for (uint64_t frame = from; frame < to; frame++)
		sim_frame(sim, frame);
}

static void check_applied(const char *what, const struct sim_encoder *enc, size_t num, const int64_t *pts)
{
	bool same = enc->num_applied == num;
	for (size_t i = 0; same && i < num; i++)
		same = enc->applied[i] == pts[i];
	if (!same) {
		fprintf(stderr, "%s: applied %zu times:", what, enc->num_applied);
		for (size_t i = 0; i < enc->num_applied; i++)
			fprintf(stderr, " %lld", (long long)enc->applied[i]);
		fprintf(stderr, "; expected %zu times:", num);
		for (size_t i = 0; i < num; i++)
			fprintf(stderr, " %lld", (long long)pts[i]);
		fprintf(stderr, "\n");
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* obs_encoder_update(): the request */

static void update_first_request(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(0, 600, 2, 1, true);
	obs_encoder_group_reconfig_update(&group, 3, 0);
	struct obs_encoder_group_reconfig want = group_of(1, 600, 0, 0, false);
	check_group("first request: counts reset, next_pts kept", &group, &want);
}

static void update_while_encoders_apply(void **state)
{
	(void)state;
	/* Request 1 is under way: 1 of 3 encoders applied it, this one not yet.
	 * The new settings ride on request 1. */
	struct obs_encoder_group_reconfig group = group_of(1, 600, 3, 1, false);
	struct obs_encoder_group_reconfig want = group;
	obs_encoder_group_reconfig_update(&group, 3, 0);
	check_group("encoder not applied yet: no change", &group, &want);
}

static void update_after_this_encoder_applied(void **state)
{
	(void)state;
	/* This encoder applied request 1, the others not yet: run again after. */
	struct obs_encoder_group_reconfig group = group_of(1, 600, 3, 1, false);
	obs_encoder_group_reconfig_update(&group, 3, 1);
	struct obs_encoder_group_reconfig want = group_of(1, 600, 3, 1, true);
	check_group("encoder applied: reconfigure_again", &group, &want);

	obs_encoder_group_reconfig_update(&group, 3, 1);
	check_group("a third update: still held", &group, &want);
}

static void update_after_all_applied(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(1, 600, 3, 3, false);
	obs_encoder_group_reconfig_update(&group, 3, 1);
	struct obs_encoder_group_reconfig want = group_of(2, 600, 0, 0, false);
	check_group("all applied: a new request", &group, &want);
}

static void update_all_applied_wins_over_again(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(4, 0, 2, 2, true);
	obs_encoder_group_reconfig_update(&group, 2, 4);
	struct obs_encoder_group_reconfig want = group_of(5, 0, 0, 0, false);
	check_group("all applied and again set: a new request", &group, &want);
}

static void update_stale_handled_request(void **state)
{
	(void)state;
	/* The encoder handled an older request (2 of request 3): neither branch. */
	struct obs_encoder_group_reconfig group = group_of(3, 90, 1, 1, false);
	struct obs_encoder_group_reconfig want = group;
	obs_encoder_group_reconfig_update(&group, 2, 2);
	check_group("older handled request: no change", &group, &want);
}

static void update_empty_group(void **state)
{
	(void)state;
	/* encoders.num 0 (characterized, not endorsed: libobs only updates an
	 * encoder in its group): 0 reconfigured equals 0, every update starts a
	 * new request. */
	struct obs_encoder_group_reconfig group = group_of(7, 0, 0, 0, true);
	obs_encoder_group_reconfig_update(&group, 0, 0);
	struct obs_encoder_group_reconfig want = group_of(8, 0, 0, 0, false);
	check_group("empty group", &group, &want);
}

static void new_request_counter(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(UINT64_MAX, -3, 5, 4, true);
	obs_encoder_group_reconfig_new_request(&group);
	/* characterized, not endorsed: the counter wraps to 0, which update()
	 * then takes for "no request yet". */
	struct obs_encoder_group_reconfig want = group_of(0, -3, 0, 0, false);
	check_group("wraps", &group, &want);
}

/* ------------------------------------------------------------------------ */
/* handle_encoder_group_reconfigure_request(): one frame */

static void frame_new_request_sets_next_pts(void **state)
{
	(void)state;
	/* 29.97 fps, divisors 1 and 3 (LCM 3): the timestep is 3003. */
	struct obs_encoder_group_reconfig group = group_of(1, 0, 0, 0, false);
	struct obs_encoder_reconfig enc = {0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 3, &enc, 0, NTSC_NUM, 1));
	struct obs_encoder_group_reconfig want = group_of(1, 3003, 1, 0, false);
	check_group("pts 0 + 1001, up to 3003", &group, &want);
	check_encoder("takes the request", &enc, 1, 0);
}

static void frame_next_pts_on_the_boundary(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(1, 0, 0, 0, false);
	struct obs_encoder_reconfig enc = {0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 3, &enc, 2002, NTSC_NUM, 1));
	struct obs_encoder_group_reconfig want = group_of(1, 3003, 1, 0, false);
	check_group("pts 2002 + 1001 = 3003, kept", &group, &want);

	group = group_of(1, 0, 0, 0, false);
	enc = (struct obs_encoder_reconfig){0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 3, &enc, 3003, NTSC_NUM, 1));
	want = group_of(1, 6006, 1, 0, false);
	check_group("pts 3003 + 1001, up to 6006", &group, &want);

	group = group_of(1, 0, 0, 0, false);
	enc = (struct obs_encoder_reconfig){0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 3, &enc, 3003, NTSC_NUM, 3));
	want = group_of(1, 6006, 1, 0, false);
	check_group("divisor 3: pts 3003 + 3003 = 6006, kept", &group, &want);
}

static void frame_largest_next_pts_wins(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(1, 9009, 1, 0, false);
	struct obs_encoder_reconfig enc = {0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 3, &enc, 0, NTSC_NUM, 1));
	struct obs_encoder_group_reconfig want = group_of(1, 9009, 2, 0, false);
	check_group("smaller next pts: kept", &group, &want);
}

static void frame_recomputes_on_the_shared_frame(void **state)
{
	(void)state;
	/* Not every encoder took the request yet (1 of 2), and this one reaches
	 * next_pts: it moves next_pts on, without counting again. */
	struct obs_encoder_group_reconfig group = group_of(1, 4, 1, 0, false);
	struct obs_encoder_reconfig enc = {1, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 4, 1, 1));
	struct obs_encoder_group_reconfig want = group_of(1, 6, 1, 0, false);
	check_group("pts 4 + 1, up to 6", &group, &want);
	check_encoder("same request", &enc, 1, 0);
}

static void frame_applies_on_the_shared_frame(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(1, 6, 2, 0, false);
	struct obs_encoder_reconfig enc = {1, 0};
	assert_true(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 6, 1, 1));
	struct obs_encoder_group_reconfig want = group_of(1, 6, 2, 1, false);
	check_group("first applies", &group, &want);
	check_encoder("handled", &enc, 1, 1);

	/* The same encoder again on the same pts: handled, nothing more. */
	struct obs_encoder_group_reconfig before = group;
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 6, 1, 1));
	before.next_pts = 7 + 1; /* characterized, not endorsed: the shared frame moves on (6 + 1, up to 8) */
	check_group("again on the shared frame", &group, &before);
	check_encoder("still handled", &enc, 1, 1);
}

static void frame_each_apply_condition(void **state)
{
	(void)state;
	struct obs_encoder_reconfig enc;
	struct obs_encoder_group_reconfig group;

	/* Not all encoders took the request: no apply, pts 6 == next moves it. */
	group = group_of(1, 6, 1, 0, false);
	enc = (struct obs_encoder_reconfig){1, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 6, 1, 1));

	/* Another pts: nothing at all. */
	group = group_of(1, 6, 2, 0, false);
	struct obs_encoder_group_reconfig want = group;
	enc = (struct obs_encoder_reconfig){1, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 5, 1, 1));
	check_group("pts before the shared frame", &group, &want);
	check_encoder("pts before the shared frame", &enc, 1, 0);

	/* Already handled, on another pts: nothing. */
	group = group_of(1, 6, 2, 1, false);
	want = group;
	enc = (struct obs_encoder_reconfig){1, 1};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 7, 1, 1));
	check_group("handled, later pts", &group, &want);

	/* No request at all (0, encoder 0): nothing on a pts past next_pts. */
	group = group_of(0, 0, 0, 0, false);
	want = group;
	enc = (struct obs_encoder_reconfig){0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 1, 1, 1));
	check_group("no request", &group, &want);

	/* characterized, not endorsed: no request, but cur_pts 0 equals
	 * next_pts 0 on the first frame, so next_pts moves to the next shared
	 * frame. */
	group = group_of(0, 0, 0, 0, false);
	enc = (struct obs_encoder_reconfig){0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 0, 1, 1));
	want = group_of(0, 2, 0, 0, false);
	check_group("no request, first frame", &group, &want);
}

static void frame_last_apply_starts_held_request(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(1, 6, 2, 1, true);
	struct obs_encoder_reconfig enc = {1, 0};
	assert_true(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 6, 1, 2));
	struct obs_encoder_group_reconfig want = group_of(2, 6, 0, 0, false);
	check_group("last apply with again: request 2", &group, &want);
	check_encoder("handled 1", &enc, 1, 1);

	group = group_of(1, 6, 3, 1, true);
	enc = (struct obs_encoder_reconfig){1, 0};
	assert_true(obs_encoder_group_reconfig_frame(&group, 3, 3, 2, &enc, 6, 1, 2));
	want = group_of(1, 6, 3, 2, true);
	check_group("second of three with again: held", &group, &want);

	group = group_of(1, 6, 2, 1, false);
	enc = (struct obs_encoder_reconfig){1, 0};
	assert_true(obs_encoder_group_reconfig_frame(&group, 2, 2, 2, &enc, 6, 1, 2));
	want = group_of(1, 6, 2, 2, false);
	check_group("last apply without again: done", &group, &want);
}

static void frame_u32_products_wrap(void **state)
{
	(void)state;
	/* characterized, not endorsed: timebase_num * frame_rate_divisor wraps
	 * as uint32_t to 0, so next_pts is cur_pts, then rounded up to the
	 * timestep 0x80000000 (timebase_num * LCM 1). */
	struct obs_encoder_group_reconfig group = group_of(1, 0, 0, 0, false);
	struct obs_encoder_reconfig enc = {0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 1, 1, 1, &enc, 5, 0x80000000u, 2));
	struct obs_encoder_group_reconfig want = group_of(1, 0x80000000LL, 1, 0, false);
	check_group("wrapped step", &group, &want);

	/* timebase_num * LCM wraps too: 0x10001 * 0x10000 is 0x10000. */
	group = group_of(1, 0, 0, 0, false);
	enc = (struct obs_encoder_reconfig){0, 0};
	assert_false(obs_encoder_group_reconfig_frame(&group, 1, 1, 0x10000u, &enc, 1, 0x10001u, 1));
	want = group_of(1, 0x20000LL, 1, 0, false);
	check_group("wrapped timestep", &group, &want);
}

/* ------------------------------------------------------------------------ */
/* the group over frames */

static void walk_two_encoders(void **state)
{
	(void)state;
	/* 30 fps and 15 fps (divisors 1 and 2), timebase_num 1: the update
	 * before frame 3 applies on pts 6 for both, once each. */
	const uint32_t divisors[] = {1, 2};
	struct sim sim;
	sim_init(&sim, 1, 2, divisors);
	check_u32("lcm", sim.lcm, 2);
	sim_frames(&sim, 0, 3);
	sim_update(&sim, 0);
	sim_frames(&sim, 3, 20);
	const int64_t want[] = {6};
	check_applied("encoder 1", &sim.encoders[0], 1, want);
	check_applied("encoder 2", &sim.encoders[1], 1, want);
}

static void walk_three_encoders_ntsc(void **state)
{
	(void)state;
	/* 29.97 fps, divisors 1, 2 and 3 (LCM 6, timestep 6006): an update
	 * before frame 7 applies on the next multiple of 6006 after every
	 * encoder took the request. */
	const uint32_t divisors[] = {1, 2, 3};
	struct sim sim;
	sim_init(&sim, NTSC_NUM, 3, divisors);
	check_u32("lcm", sim.lcm, 6);
	sim_frames(&sim, 0, 7);
	sim_update(&sim, 2);
	sim_frames(&sim, 7, 40);
	const int64_t want[] = {12 * NTSC_NUM};
	check_applied("encoder 1", &sim.encoders[0], 1, want);
	check_applied("encoder 2", &sim.encoders[1], 1, want);
	check_applied("encoder 3", &sim.encoders[2], 1, want);
}

static void walk_second_update_while_pending(void **state)
{
	(void)state;
	/* Two updates before any encoder applies: one apply each. */
	const uint32_t divisors[] = {1, 2};
	struct sim sim;
	sim_init(&sim, 1, 2, divisors);
	sim_frames(&sim, 0, 3);
	sim_update(&sim, 0);
	sim_frame(&sim, 3);
	sim_update(&sim, 1);
	sim_frames(&sim, 4, 20);
	const int64_t want[] = {6};
	check_applied("encoder 1", &sim.encoders[0], 1, want);
	check_applied("encoder 2", &sim.encoders[1], 1, want);
}

static void walk_second_update_after_one_applied(void **state)
{
	(void)state;
	/* Divisors 1 and 2: on frame 6 encoder 1 applies, and an update to it
	 * before encoder 2's turn on the same frame is held (reconfigure_again).
	 * Encoder 2's apply starts request 2, which applies on a later shared
	 * frame. */
	const uint32_t divisors[] = {1, 2};
	struct sim sim;
	sim_init(&sim, 1, 2, divisors);
	sim_frames(&sim, 0, 3);
	sim_update(&sim, 0);
	sim_frames(&sim, 3, 6);

	/* frame 6, encoder 1 only, then the update, then encoder 2 */
	struct sim_encoder *e1 = &sim.encoders[0], *e2 = &sim.encoders[1];
	assert_true(obs_encoder_group_reconfig_frame(&sim.group, 2, 2, sim.lcm, &e1->reconfig, e1->cur_pts, 1, 1));
	e1->applied[e1->num_applied++] = e1->cur_pts;
	e1->cur_pts += 1;
	sim_update(&sim, 0);
	assert_true(sim.group.reconfigure_again);
	assert_true(obs_encoder_group_reconfig_frame(&sim.group, 2, 2, sim.lcm, &e2->reconfig, e2->cur_pts, 1, 2));
	e2->applied[e2->num_applied++] = e2->cur_pts;
	e2->cur_pts += 2;
	check_u32("request 2", (uint32_t)sim.group.reconfigure_request, 2);

	sim_frames(&sim, 7, 30);
	const int64_t want[] = {6, 10};
	check_applied("encoder 1", e1, 2, want);
	check_applied("encoder 2", e2, 2, want);
}

/* ------------------------------------------------------------------------ */
/* add_connection() and remove_connection() */

static void lcm_of_divisors(void **state)
{
	(void)state;
	check_u32("1, 1", obs_encoder_group_reconfig_lcm(1, 1), 1);
	check_u32("1, 2", obs_encoder_group_reconfig_lcm(1, 2), 2);
	check_u32("2, 2", obs_encoder_group_reconfig_lcm(2, 2), 2);
	check_u32("2, 3", obs_encoder_group_reconfig_lcm(2, 3), 6);
	check_u32("3, 2", obs_encoder_group_reconfig_lcm(3, 2), 6);
	check_u32("2, 4", obs_encoder_group_reconfig_lcm(2, 4), 4);
	check_u32("4, 2", obs_encoder_group_reconfig_lcm(4, 2), 4);
	check_u32("4, 6", obs_encoder_group_reconfig_lcm(4, 6), 12);
	check_u32("6, 4", obs_encoder_group_reconfig_lcm(6, 4), 12);
	check_u32("60, 7", obs_encoder_group_reconfig_lcm(60, 7), 420);
	check_u32("largest in int", obs_encoder_group_reconfig_lcm(0x7fffffffu, 1), 0x7fffffffu);
	check_u32("46340, 46339", obs_encoder_group_reconfig_lcm(46340, 46339), 2147349260u);
}

static void reset_when_idle(void **state)
{
	(void)state;
	struct obs_encoder_group_reconfig group = group_of(9, 12012, 3, 2, true);
	obs_encoder_group_reconfig_reset(&group);
	struct obs_encoder_group_reconfig want = group_of(0, 0, 0, 0, false);
	check_group("reset", &group, &want);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(update_first_request),
		cmocka_unit_test(update_while_encoders_apply),
		cmocka_unit_test(update_after_this_encoder_applied),
		cmocka_unit_test(update_after_all_applied),
		cmocka_unit_test(update_all_applied_wins_over_again),
		cmocka_unit_test(update_stale_handled_request),
		cmocka_unit_test(update_empty_group),
		cmocka_unit_test(new_request_counter),
		cmocka_unit_test(frame_new_request_sets_next_pts),
		cmocka_unit_test(frame_next_pts_on_the_boundary),
		cmocka_unit_test(frame_largest_next_pts_wins),
		cmocka_unit_test(frame_recomputes_on_the_shared_frame),
		cmocka_unit_test(frame_applies_on_the_shared_frame),
		cmocka_unit_test(frame_each_apply_condition),
		cmocka_unit_test(frame_last_apply_starts_held_request),
		cmocka_unit_test(frame_u32_products_wrap),
		cmocka_unit_test(walk_two_encoders),
		cmocka_unit_test(walk_three_encoders_ntsc),
		cmocka_unit_test(walk_second_update_while_pending),
		cmocka_unit_test(walk_second_update_after_one_applied),
		cmocka_unit_test(lcm_of_divisors),
		cmocka_unit_test(reset_when_idle),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
