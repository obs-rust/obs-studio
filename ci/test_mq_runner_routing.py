#!/usr/bin/env -S uv run --script
"""Merge queue runner selection: forge pool-select with a hosted fallback (no network)."""

import re
import unittest
from pathlib import Path

POOL_SELECT = 'forge-ci/ci.yml/actions/pool-select'
POOL_SELECT_SHA = '82945f807590d469a2833f2a421a0a7861bc37d1'  # ci.yml#564: dispatcher room (gh-runner#67)


class HostedRunnerRoutingTest(unittest.TestCase):
    """Pin the hosted fallback and the pool-select route (#5332, #5376)."""

    def job(self, workflow, name):
        path = Path(__file__).resolve().parent.parent / '.github/workflows' / workflow
        match = re.search(
            rf'^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)',
            path.read_text(), re.S | re.M,
        )
        self.assertIsNotNone(match, name)
        return match.group(1)

    def assert_runner(self, workflow, name, expected):
        match = re.search(r'^    runs-on: (.+)$', self.job(workflow, name), re.M)
        self.assertIsNotNone(match, name)
        self.assertEqual(match.group(1), expected, name)

    def test_tier2_selector_fallback_and_dell_route(self):
        # heavy-labels carries labels only when pool-select chose the pool; a
        # skipped, failed or hosted selection leaves it empty at the consumers.
        job = self.job('ci.yml', 'select-mq-runner')
        self.assertIn(
            "heavy-labels: ${{ steps.select.outputs.pool == 'true' && steps.select.outputs.runs-on || '' }}\n", job)
        self.assert_runner('build-project.yaml', 'rust-linux-tier2',
            "${{ inputs.mq-heavy-labels != '' && fromJSON(inputs.mq-heavy-labels) || 'ubuntu-latest' }}")
        build = self.job('build-project.yaml', 'rust-linux-tier2-build')
        self.assertIn('needs: [check-event, ci-mode, reserve-tier2-linux-build]', build)
        self.assertIn('fromJSON(needs.reserve-tier2-linux-build.outputs.runs-on)', build)
        self.assertIn(
            "(github.event_name != 'merge_group' || vars.FORGE_POOL_LEASE_CLASS == '' "
            "|| vars.FORGE_POOL_LABELS == '') && inputs.mq-heavy-labels != ''", build,
        )
        self.assertIn("|| 'ubuntu-latest'", build)
        reserve = self.job('build-project.yaml', 'reserve-tier2-linux-build')
        self.assertIn("github.event_name == 'merge_group' && needs.check-event.outputs.tier2 == 'true'", reserve)
        self.assertIn('lease-job: rust-linux-tier2-build', reserve)
        self.assertIn('id-token: write', reserve)

    def test_pool_select_reads_signal_behind_kill_switch(self):
        job = self.job('ci.yml', 'select-mq-runner')
        step = job.split('      - name: Select Pool or Hosted\n', 1)[1]
        # merge_group and same-repo PRs with FORGE_POOL_LABELS set ask the pool;
        # fork PRs never run the job.
        self.assertIn("    if: github.event_name != 'pull_request' || "
                      "github.event.pull_request.head.repo.full_name == github.repository\n", job)
        self.assertIn("        if: (github.event_name == 'merge_group' || github.event_name == 'pull_request') "
                      "&& vars.FORGE_POOL_LABELS != ''\n", step)
        self.assertIn('        continue-on-error: true\n', step)
        self.assertIn('          source: signal\n', step)
        self.assertIn('          labels: ${{ vars.FORGE_POOL_LABELS }}\n', step)
        # Tokenless: no App token, no secret and no write scope in the selector.
        self.assertIn('    permissions: {}\n', job)
        self.assertNotIn('secrets.', job)
        self.assertNotIn('token:', job)

    def test_queue_selector_uses_forge_quick_with_kill_switch(self):
        # merge_group and PR selection leave the hosted queue (#5376); an empty
        # FORGE_QUICK_LABELS is the kill switch back to ubuntu-latest.
        self.assert_runner('ci.yml', 'select-mq-runner',
            "${{ (github.event_name == 'merge_group' || github.event_name == 'pull_request') "
            "&& vars.FORGE_QUICK_LABELS != '' && "
            "fromJSON(vars.FORGE_QUICK_LABELS) || 'ubuntu-latest' }}")
        job = self.job('ci.yml', 'select-mq-runner')
        # forge-quick: 120 s wall clock, 256 MiB including the tmpfs workspace.
        self.assertIn('    timeout-minutes: 2\n', job)
        self.assertNotIn('container:', job)
        self.assertNotIn('services:', job)
        # No checkout: pool-select needs no repository files.
        self.assertNotIn('actions/checkout', job)

    # forge-quick pre-seeds these actions at these SHAs; anything else is
    # downloaded inside the job's 256 MiB / 120 s (forge-ci/ci.yml RUN-004,
    # ci_lint/data/forge_quick_allowlist.toml) (#5388). pool-select's own
    # composite step pins the allowlisted setup-uv 1db35dd.
    FORGE_QUICK_ALLOWLIST = {
        POOL_SELECT: POOL_SELECT_SHA,
    }

    def test_queue_selector_uses_only_forge_quick_allowlisted_actions(self):
        job = self.job('ci.yml', 'select-mq-runner')
        uses = re.findall(r'^\s+(?:- )?uses: ([^@\s]+)@(\S+)', job, re.M)
        self.assertEqual(sorted(action for action, _ in uses), sorted(self.FORGE_QUICK_ALLOWLIST))
        for action, sha in uses:
            with self.subTest(action=action):
                self.assertEqual(sha, self.FORGE_QUICK_ALLOWLIST[action])

    def test_queue_setup_uses_sized_runners_with_hosted_fallback(self):
        workflow = 'build-project.yaml'
        self.assert_runner(workflow, 'ci-mode',
            "${{ github.event_name == 'merge_group' && vars.FORGE_QUICK_LABELS != '' && "
            "fromJSON(vars.FORGE_QUICK_LABELS) || 'ubuntu-latest' }}")
        self.assert_runner(workflow, 'check-event',
            "${{ inputs.mq-event-labels != '' && "
            "fromJSON(inputs.mq-event-labels) || 'ubuntu-latest' }}")
        event = self.job(workflow, 'check-event')
        self.assertNotIn("    needs:", event)
        caller = self.job('ci.yml', 'build-project')
        self.assertIn("needs: [select-mq-runner, reserve-event-data]", caller)
        self.assertIn("!cancelled()", caller)
        self.assertIn("mq-event-labels: ${{ needs.reserve-event-data.outputs.runs-on }}", caller)
        reserve = self.job('ci.yml', 'reserve-event-data')
        self.assertIn("github.event_name == 'merge_group' && needs.select-mq-runner.outputs.heavy-labels != ''", reserve)
        self.assertIn("vars.FORGE_POOL_LEASE_CLASS != ''", reserve)
        self.assertIn("    timeout-minutes: 2\n", reserve)
        self.assertIn("      id-token: write\n", reserve)
        self.assertIn("          source: lease\n", reserve)
        self.assertIn("          fallback: ubuntu-latest\n", reserve)
        self.assertIn("          lease-job: check-event\n", reserve)
        mode = self.job(workflow, 'ci-mode')
        steps = mode.split('    steps:\n', 1)[1].split('      - ')
        for step in steps[1:]:
            self.assertIn("if: github.event_name == 'pull_request' && !inputs.full\n", step)

    def test_light_jobs_use_latest(self):
        for workflow, names in (
            ('build-project.yaml', ('rust-cross-proof',)),
            ('ci.yml', ('update-documentation', 'native-cache-consumer', 'tier2-lane-proof')),
        ):
            for name in names:
                with self.subTest(workflow=workflow, job=name):
                    self.assert_runner(workflow, name, 'ubuntu-latest')

    def test_host_package_jobs_keep_2604(self):
        for name in ('rust-tests-linux', 'cross-compile', 'linux-qt-frontend',
                     'rust-cross-lint', 'rust-cross-lint-release', 'rust-windows-link',
                     'cmocka-link', 'rust-tests-macos-build', 'rust-tier2-macos-build'):
            with self.subTest(job=name):
                # Every heavy Linux job follows the selector in merge_group too.
                self.assert_runner('build-project.yaml', name,
                    "${{ inputs.mq-heavy-labels != '' && fromJSON(inputs.mq-heavy-labels) || 'ubuntu-26.04' }}")

    def test_forge_build_jobs_share_the_persistent_checkpoint_format(self):
        workflow = 'build-project.yaml'
        for name in ('rust-tests-linux', 'cross-compile', 'rust-cross-lint',
                     'rust-cross-lint-release', 'rust-windows-link', 'cmocka-link',
                     'rust-tests-macos-build', 'rust-linux-tier2-build', 'rust-tier2-macos-build'):
            with self.subTest(job=name):
                job = self.job(workflow, name)
                self.assertIn('uses: zackees/setup-soldr@dd26c92eb35c553b208bb741326b6d54cf611436', job)
                self.assertIn(
                    "SETUP_SOLDR_PERSISTENT_CHECKPOINT: ${{ runner.environment == 'self-hosted' && 'boot' || '' }}",
                    job,
                )

    def test_packaging_and_setup_probes_keep_2604(self):
        for workflow, names in (
            ('build-project.yaml', ('flatpak-build', 'flatpak-compiler-consumer', 'flatpak-runtime-test')),
            ('ci.yml', ('flatpak-runtime-reuse', 'tier2-setup-producer', 'tier2-setup-consumer')),
        ):
            for name in names:
                with self.subTest(workflow=workflow, job=name):
                    self.assert_runner(workflow, name, 'ubuntu-26.04')
        self.assertIn('os: [ubuntu-26.04]', self.job('build-project.yaml', 'ubuntu-build'))


if __name__ == "__main__":
    unittest.main()
