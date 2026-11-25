#!/usr/bin/env -S uv run --script
"""The pool-signal canary must fail on the shapes the selector silently eats.

pool-select is deliberately lenient: a null dispatcher adds no room and moves
on. That is right for a merge gate and wrong for a monitor, so these tests pin
every shape that makes the document unusable as a *problem* rather than a
quiet zero (forge-ci/gh-runner#90, 2026-11-24).
"""
from __future__ import annotations

import unittest

import pool_signal_canary as canary


def payload(entry):
    return {'generated_at': '2026-11-24T00:00:00Z', 'orgs': {'obs-rust': entry}}


def healthy(**overrides):
    host = {'host': 'dell-fsx3mn2', 'labels': ['self-hosted', 'forge-pool'],
            'spare': 4, 'age_s': 12.5}
    host.update(overrides)
    return {'state': 'ok', 'error': None, 'dispatcher': {'hosts': [host]},
            'runners': [{'labels': ['self-hosted'], 'online': 2, 'busy': 0}]}


class EvaluateTest(unittest.TestCase):
    def test_a_healthy_document_has_no_problems(self):
        self.assertEqual(canary.evaluate(payload(healthy()), 'obs-rust'), [])

    def test_an_absent_org_is_a_problem(self):
        problems = canary.evaluate(payload(healthy()), 'FastLED')
        self.assertTrue(problems)
        self.assertIn('missing', problems[0])

    def test_a_null_dispatcher_is_a_problem_even_with_runners_online(self):
        entry = healthy()
        entry['dispatcher'] = None
        problems = canary.evaluate(payload(entry), 'obs-rust')
        self.assertEqual(len(problems), 1)
        self.assertIn('dispatcher is null', problems[0])
        self.assertIn('2 runner(s) report online', problems[0])

    def test_a_null_dispatcher_without_runners_is_still_a_problem(self):
        entry = healthy()
        entry['dispatcher'] = None
        entry['runners'] = []
        problems = canary.evaluate(payload(entry), 'obs-rust')
        self.assertIn('dispatcher is null', problems[0])

    def test_a_dispatcher_with_no_lanes_is_a_problem(self):
        entry = healthy()
        entry['dispatcher'] = {'hosts': []}
        self.assertIn('no lanes', canary.evaluate(payload(entry), 'obs-rust')[0])

    def test_a_stale_lane_is_a_problem(self):
        problems = canary.evaluate(payload(healthy(age_s=900)), 'obs-rust')
        self.assertTrue(any('older than 180s' in problem for problem in problems), problems)

    def test_a_malformed_spare_is_a_problem(self):
        problems = canary.evaluate(payload(healthy(spare=None)), 'obs-rust')
        self.assertTrue(any('spare is' in problem for problem in problems), problems)

    def test_a_lane_without_labels_is_a_problem(self):
        problems = canary.evaluate(payload(healthy(labels=[])), 'obs-rust')
        self.assertTrue(any('no labels' in problem for problem in problems), problems)

    def test_a_failing_state_is_a_problem(self):
        entry = healthy()
        entry['state'] = 'error'
        entry['error'] = 'scrape timed out'
        problems = canary.evaluate(payload(entry), 'obs-rust')
        self.assertTrue(any('state is' in problem for problem in problems), problems)

    def test_a_max_age_override_is_honoured(self):
        # 100 s is stale when the caller asks for 60, fresh at the default 180.
        self.assertEqual(canary.evaluate(payload(healthy(age_s=100)), 'obs-rust'), [])
        self.assertTrue(canary.evaluate(payload(healthy(age_s=100)), 'obs-rust', max_age=60))


class SummariseTest(unittest.TestCase):
    def test_a_healthy_payload_reads_as_actionable(self):
        lines, problems = canary.summarise(payload(healthy()), ('obs-rust',), 180)
        self.assertEqual(problems, [])
        self.assertTrue(any('signal is actionable' in line for line in lines))

    def test_the_null_dispatcher_line_says_so(self):
        entry = healthy()
        entry['dispatcher'] = None
        lines, problems = canary.summarise(payload(entry), ('obs-rust',), 180)
        self.assertTrue(any('dispatcher=null' in line for line in lines))
        self.assertEqual(len(problems), 1)


if __name__ == '__main__':
    unittest.main()
