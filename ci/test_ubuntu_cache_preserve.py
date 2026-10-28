"""Restored Ubuntu app compiler caches must be consumed before sizing saves."""
from pathlib import Path
import re
import os
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]

def ubuntu_job():
    text=(ROOT/'.github/workflows/build-project.yaml').read_text()
    return re.search(r'^  ubuntu-build:\n(.*?)(?=^  [a-z0-9-]+:\n)',text,re.M|re.S).group(1)

class UbuntuCachePreserveTest(unittest.TestCase):
    def test_exact_restore_does_not_repack_immutable_snapshot(self):
        job=ubuntu_job()
        steps=re.split(r'(?m)^      - ',job)
        restore=next(step for step in steps if 'actions/cache/restore@' in step
                  or ('uses: ./.github/actions/cache' in step and re.search(r'(?m)^\s+mode: restore$', step)))
        save=next(step for step in steps if 'actions/cache/save@' in step
               or ('uses: ./.github/actions/cache' in step and re.search(r'(?m)^\s+mode: save$', step)))
        self.assertIn('id: ubuntu-compiler-cache',restore)
        guard=re.search(r'^        if: (.+)$',save,re.M).group(1)
        self.assertIn("steps.ubuntu-compiler-cache.outputs.cache-hit != 'true'",guard,
                      'An exact hit must skip compression and immutable-key reservation')
        self.assertIn("github.ref == 'refs/heads/master'",guard)
        self.assertIn("steps.cache-size.outputs.save == 'true'",guard)
        key_pattern=r'^          key: (.+)$'
        self.assertEqual(re.search(key_pattern,restore,re.M).group(1),
                         re.search(key_pattern,save,re.M).group(1))

    def test_restore_stays_within_mode_runner_arch_and_configuration(self):
        block=ubuntu_job().split('restore-keys: |',1)[1].split('\n\n',1)[0]
        for mode,opposite in [('OFF','ON'),('ON','OFF')]:
            rendered=block
            for expression,value in {
                '${{ matrix.os }}':'ubuntu-26.04',
                '${{ matrix.rust }}':mode,
                "${{ matrix.rust == 'ON' && 'OFF' || 'ON' }}":opposite,
                '${{ needs.check-event.outputs.config }}':'RelWithDebInfo',
            }.items():
                rendered=rendered.replace(expression,value)
            self.assertNotIn('${{',rendered)
            prefixes=rendered.split()
            own=f'zccache-ubuntu-ubuntu-26.04-x86_64-rust{mode}-RelWithDebInfo-old'
            survivor=f'zccache-ubuntu-ubuntu-26.04-x86_64-rust{opposite}-RelWithDebInfo-new'
            incompatible=[
                survivor,
                survivor.replace('ubuntu-26.04','ubuntu-24.04'),
                survivor.replace('x86_64','aarch64'),
                survivor.replace('RelWithDebInfo','Debug'),
                own.replace('RelWithDebInfo','Debug'),
                own.replace('ubuntu-26.04','ubuntu-24.04'),
                own.replace('x86_64','aarch64'),
            ]
            def restore(keys):
                return next((key for prefix in prefixes for key in keys if key.startswith(prefix)),None)
            with self.subTest(mode=mode):
                self.assertIsNone(restore([survivor,*incompatible]),
                                  'A foreign mode/configuration bloats the snapshot and can prevent seeding')
                self.assertEqual(restore([survivor,own]),own)
                self.assertIsNone(restore(incompatible))

    def test_restored_cache_is_not_deleted_before_build(self):
        job=ubuntu_job()
        before=job[:job.index('- name: Build OBS Studio')]
        self.assertNotIn('rm -rf "$dir"',before)
        self.assertNotIn('starting cold',before)

    def test_oversize_snapshot_skips_save_after_compiler_consumption(self):
        job=ubuntu_job()
        self.assertLess(job.index('- name: Build OBS Studio'),job.index('id: cache-size'))
        self.assertIn('8192',job)
        self.assertIn("steps.cache-size.outputs.save == 'true'",job)
        self.assertIn('save=false',job)
        self.assertIn('save=true',job)

    def test_real_snapshot_shell_keeps_normal_build_and_rejects_growth(self):
        block=ubuntu_job().split('id: cache-size',1)[1]
        body=re.search(r'        run: \|\n((?:          .*\n|\n)*)',block).group(1)
        script='\n'.join(line[10:] for line in body.splitlines())
        for size,expected in [(6144,'true'),(8192,'true'),(8193,'false')]:
            env={**os.environ,'CACHE_DIR':str(ROOT/'ci'),'GITHUB_OUTPUT':'/dev/stdout','TEST_SIZE':str(size)}
            run=subprocess.run(['bash','-e','-c','du() { echo "$TEST_SIZE"; }\n'+script],env=env,capture_output=True,text=True,check=True)
            self.assertIn('save='+expected,run.stdout)

if __name__ == '__main__':
    unittest.main()
