"""Coalescing must retain work from superseded master pushes."""
import os
import subprocess
import unittest
from unittest.mock import Mock, patch
from master_scope_base import scope_base

class MasterScopeBaseTest(unittest.TestCase):
    def test_uses_successful_ancestor_instead_of_immediate_push_parent(self):
        sha = 'a' * 40
        run = Mock(side_effect=[subprocess.CompletedProcess([], 0, '[{"headSha":"'+sha+'"}]'), subprocess.CompletedProcess([], 0), subprocess.CompletedProcess([], 0, '# before coalescing')])
        self.assertEqual(scope_base(run), sha)
        self.assertIn('--status', run.call_args_list[0].args[0])
        self.assertIn('success', run.call_args_list[0].args[0])

    def test_legacy_coalesced_success_does_not_advance_baseline(self):
        legacy, safe = 'b' * 40, 'a' * 40
        run = Mock(side_effect=[
            subprocess.CompletedProcess([], 0, '[{"headSha":"'+legacy+'"},{"headSha":"'+safe+'"}]'),
            subprocess.CompletedProcess([], 0),
            subprocess.CompletedProcess([], 0, 'group: push-build-master'),
            subprocess.CompletedProcess([], 0, '# legacy helper without bootstrap protection'),
            subprocess.CompletedProcess([], 0),
            subprocess.CompletedProcess([], 0, '# before master coalescing'),
        ])
        self.assertEqual(scope_base(run), safe)

    def test_current_catchup_run_is_a_valid_baseline(self):
        from master_scope_base import baseline_covers_coalesced_pushes
        run = Mock(side_effect=[subprocess.CompletedProcess([], 0, 'group: push-build-master'), subprocess.CompletedProcess([], 0, 'def baseline_covers_coalesced_pushes(sha, run):')])
        self.assertTrue(baseline_covers_coalesced_pushes('a' * 40, run))

    def test_unreadable_historical_workflow_is_not_trusted(self):
        from master_scope_base import baseline_covers_coalesced_pushes
        run = Mock(return_value=subprocess.CompletedProcess([], 128, ''))
        self.assertFalse(baseline_covers_coalesced_pushes('a' * 40, run))

    def test_nonancestor_is_ignored_and_empty_tree_is_safe_fallback(self):
        run = Mock(side_effect=[subprocess.CompletedProcess([], 0, '[{"headSha":"'+'b'*40+'"}]'), subprocess.CompletedProcess([], 1)])
        self.assertEqual(scope_base(run), 'ALL')

    def test_api_failure_uses_entire_tree(self):
        run = Mock(side_effect=[subprocess.CalledProcessError(1, 'gh')])
        self.assertEqual(scope_base(run), 'ALL')

    def test_fallback_scopes_all_tracked_paths(self):
        from unittest.mock import patch
        from ci_scope import changed_files
        with patch('ci_scope.subprocess.run', return_value=subprocess.CompletedProcess([], 0, 'plugins/linux-v4l2/build/plugin.c\n')) as run:
            self.assertEqual(changed_files('ALL'), ['plugins/linux-v4l2/build/plugin.c'])
            self.assertEqual(run.call_args.args[0], ['git', 'ls-files'])

    def test_gh_json_is_parsed_without_colour_codes(self):
        """setup-soldr exports FORCE_COLOR through GITHUB_ENV; gh then colourises --json."""
        sha = 'a' * 40
        run = Mock(side_effect=[subprocess.CompletedProcess([], 0, '[{"headSha":"' + sha + '"}]'),
                                subprocess.CompletedProcess([], 0),
                                subprocess.CompletedProcess([], 0, '# before coalescing')])
        hostile = {'CLICOLOR_FORCE': '1', 'FORCE_COLOR': '1', 'GH_FORCE_TTY': '120', 'PATH': '/bin'}
        with patch.dict(os.environ, hostile, clear=True):
            self.assertEqual(scope_base(run), sha)
        env = run.call_args_list[0].kwargs['env']
        for key in ('CLICOLOR_FORCE', 'FORCE_COLOR', 'GH_FORCE_TTY'):
            self.assertNotIn(key, env)
        self.assertEqual(env['NO_COLOR'], '1')
        self.assertEqual(env['PATH'], '/bin')

    def test_master_workflow_uses_catchup_scope(self):
        from pathlib import Path
        text = Path('.github/workflows/build-project.yaml').read_text()
        self.assertIn('base="$(uv run --no-project ci/master_scope_base.py)"', text)
        self.assertIn('GH_TOKEN: ${{ github.token }}', text)

if __name__ == '__main__':
    unittest.main()
