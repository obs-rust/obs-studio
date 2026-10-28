#!/usr/bin/env -S uv run --script
"""Tier 2 Linux jobs reuse their apt downloads (#1055).

    uv run python -m unittest ci/test_tier2_apt_cache.py

Every Tier 2 Linux job (build + complete OFF/ON suites) downloaded the same 339 MB of
.debs (~20 s) into the stock ubuntu:26.04 container. The jobs now keep the
downloads (the image's docker-clean apt config deletes them) and cache
/var/cache/apt/archives, keyed on the package list and the image digest.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "build-project.yaml"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name}"
    return m.group(1)


class AptCacheTest(unittest.TestCase):
    def test_manifest_reader_has_uv_in_both_stock_job_containers(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            self.assertRegex(j, r"uses: astral-sh/setup-uv@[0-9a-f]{40}", name)
            self.assertLess(j.index("astral-sh/setup-uv@"), j.index("run: bash rust/tools/linux-validate/validate.sh" if name.endswith("-build") else "uv run --no-project rust/tools/linux-validate/ci-runtime.py"), name)

    def test_every_tier2_linux_job_caches_apt_archives(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            self.assertIn("docker-clean", j if name.endswith("-build") else (WORKFLOW.parents[2]/"rust/tools/linux-validate/ci-runtime.py").read_text(), name)
            self.assertIn("path: /var/cache/apt/archives", j, name)
            self.assertIn("hashFiles('rust/tools/linux-validate/apt-packages.txt')", j, name)

    def test_build_job_dpkg_skips_fsync_and_docs(self):
        # #2072: dpkg spent 29 s unpacking with the .debs already restored.
        j = job("rust-linux-tier2-build")
        self.assertIn("force-unsafe-io", j)
        self.assertIn("path-exclude=/usr/share/doc/*", j)
        self.assertLess(j.index("force-unsafe-io"), j.index("name: Install Tier 2 Packages"))

    def test_cache_is_restored_before_the_install(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            self.assertLess(j.index("path: /var/cache/apt/archives"), j.index("Install Tier 2 Packages" if name.endswith("-build") else "Test Tier 2 ("), name)
            self.assertLess(j.index("actions/checkout@"), j.index("path: /var/cache/apt/archives"), name)

    def test_package_index_is_fetched_once_per_fresh_container(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            updates = re.findall(r"^\s+(?:sudo )?apt-get update[^\n]*", j, re.M)
            if name.endswith("-build"):
                self.assertEqual(len(updates), 1, name)
                self.assertLess(j.index(updates[0].strip()), j.index("actions/checkout@"), name)
            else:
                helper = (WORKFLOW.parents[2]/"rust/tools/linux-validate/ci-runtime.py").read_text()
                self.assertEqual(helper.count("'apt-get', 'update'"), 1)
                self.assertIn("if variant == 'stock':", helper)
                self.assertEqual(len(updates), 0)

    def test_zstd_is_there_before_the_restore(self):
        # actions/cache versions an entry by its compressor: restoring before
        # zstd is installed looks for a gzip entry the zstd save never wrote,
        # so master missed its own cache (run 38888785774).
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            if name.endswith("-build"):
                install_git = j[j.index("Install Git"):j.index("actions/checkout@")]
                self.assertIn("zstd", install_git, name)
            else:
                self.assertLess(j.index("command -v zstd"), j.index("uses: ./.github/actions/cache"))

    def test_scoped_host_archive_is_used_and_only_master_saves(self):
        text=WORKFLOW.read_text()
        restore=text.index('- name: Restore Touched Target apt Downloads')
        build=text.index('- name: Build Touched Linux Targets')
        self.assertLess(restore,build)
        self.assertIn('path: ${{ github.workspace }}/.targets-apt/*.deb',text[restore:build])
        self.assertIn('targets-apt-ubuntu2604-',text[restore:build])
        install=text[build:text.index('- name: Save Touched Target ccache',build)]
        self.assertIn('Dir::Cache::archives=$APT_ARCHIVES',install)
        self.assertIn('APT::Keep-Downloaded-Packages=true',install)
        save=text[text.index('- name: Save Touched Target apt Downloads'):]
        save=save.split('\n\n',1)[0]
        self.assertIn("github.event_name == 'push'",save)
        self.assertIn("refs/heads/master",save)
        self.assertIn('/.targets-apt/*.deb',save)
        self.assertIn('evict = "lru"',(WORKFLOW.parents[2]/'ci.toml').read_text().split('targets-apt =',1)[1].splitlines()[0])

    def test_scoped_host_snapshot_bound_executes_before_save(self):
        import os
        import subprocess
        text=WORKFLOW.read_text()
        self.assertIn('steps.target-apt.outputs.image',text)
        guard=text.split('id: target-apt-size',1)[1]
        body=re.search(r'        run: \|\n((?:          .*\n|\n)*)',guard).group(1)
        script='\n'.join(line[10:] for line in body.splitlines())
        self.assertIn("steps.target-apt-size.outputs.save == 'true'",text)
        for size,expected in [(142,'true'),(200,'true'),(201,'false')]:
            env={**os.environ,'APT_ARCHIVES':'ignored','GITHUB_OUTPUT':'/dev/stdout','TEST_SIZE':str(size)}
            result=subprocess.run(['bash','-e','-c','sudo() { "$@"; }; du() { echo "$TEST_SIZE"; };\n'+script],env=env,capture_output=True,text=True,check=True)
            self.assertIn('save='+expected,result.stdout)

    def test_only_master_build_job_saves(self):
        # Every PR and queue run saved its own 323 MB entry and pushed the
        # repo past its 10 GB cache limit, evicting the ccache.
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            self.assertNotIn("uses: actions/cache@", j, name)
            self.assertRegex(j, r"uses: (?:actions/cache/restore@|\./\.github/actions/cache)", name)
        self.assertNotIn("actions/cache/save@", job("rust-linux-tier2"))
        build = job("rust-linux-tier2-build")
        save = build[build.index("Save apt Downloads"):]
        self.assertIn("refs/heads/master", save.split("\n\n")[0])


if __name__ == "__main__":
    unittest.main()
