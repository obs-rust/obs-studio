#!/usr/bin/env -S uv run --script
"""Tier 2 Linux jobs reuse their apt downloads (#1055).

    uv run python -m unittest ci/test_tier2_apt_cache.py

Every Tier 2 Linux job (build + complete OFF/ON suites) downloaded the same 339 MB of
.debs (~20 s) into the stock ubuntu:26.04 container. The jobs now keep the
downloads (the image's docker-clean apt config deletes them) and cache a
job-local archive directory, keyed on the package list and the image digest.
"""

from __future__ import annotations

import os
import re
import select
import shutil
import signal
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "build-project.yaml"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name}"
    return m.group(1)


def install_script() -> str:
    step = job("rust-linux-tier2-build").split("name: Install Tier 2 Packages", 1)[1]
    step = step.split("\n      - ", 1)[0]
    return textwrap.dedent(step.split("run: |\n", 1)[1])


class AptCacheTest(unittest.TestCase):
    def test_manifest_reader_has_uv_in_both_stock_job_containers(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            self.assertRegex(j, r"uses: astral-sh/setup-uv@[0-9a-f]{40}", name)
            self.assertLess(j.index("astral-sh/setup-uv@"), j.index("rust/tools/linux-validate/validate.sh" if name.endswith("-build") else "uv run --no-project rust/tools/linux-validate/ci-runtime.py"), name)

    def test_every_tier2_linux_job_caches_apt_archives(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            self.assertIn("docker-clean", j if name.endswith("-build") else (WORKFLOW.parents[2]/"rust/tools/linux-validate/ci-runtime.py").read_text(), name)
            self.assertIn("path: ${{ runner.temp }}/apt-archives", j, name)
            self.assertIn("hashFiles('rust/tools/linux-validate/apt-packages.txt')", j, name)

    def test_consumers_request_the_master_producers_exact_key(self):
        build = job("rust-linux-tier2-build")
        producer = build.split("- name: Save apt Downloads", 1)[1].split("\n      - ", 1)[0]
        key = re.search(r"^          key: (.+)$", producer, re.M)[1]
        self.assertIn("github.event_name == 'push' && github.ref == 'refs/heads/master'", producer)
        self.assertIn("mode: save", producer)
        self.assertNotIn("github.", key, "apt contents must not be keyed by the run or PR")
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            with self.subTest(job=name):
                restore = job(name).split("id: apt-cache", 1)[1].split("\n      - ", 1)[0]
                self.assertIn("mode: restore", restore)
                self.assertEqual(re.search(r"^          key: (.+)$", restore, re.M)[1], key)

    def test_archive_version_is_portable_between_hosted_container_and_dell(self):
        # actions/cache hashes the literal input paths into its hidden version.
        # Absolute runner.temp paths differ across the container and Dell slots;
        # identical visible keys cannot compensate for those version mismatches.
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            with self.subTest(job=name):
                paths = re.findall(r"^          path: (.+apt[^\n]*)$", job(name), re.M)
                self.assertEqual(paths, [".tier2-apt/*.deb"] * (2 if name.endswith("-build") else 1))

    def test_tier2_archive_directories_are_job_local(self):
        # Hosted jobs retain separate archives and cache extraction. The build
        # may select Dell's shared apt-locked archives, skipping Actions restore.
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            with self.subTest(job=name):
                j = job(name)
                self.assertNotIn("/var/cache/apt/archives", j)
                if name.endswith("-build"):
                    self.assertIn('[ -n "$dir" ] || dir="$RUNNER_TEMP/apt-archives"', j)
                    self.assertIn("if: steps.pcache.outputs.apt == ''", j)
                    directory = "$dir"
                else:
                    directory = "$RUNNER_TEMP/apt-archives"
                self.assertIn(f'mkdir -p "{directory}"', j)
                self.assertIn(f'chmod a+rwx "{directory}"', j)
                self.assertLess(j.index(f'mkdir -p "{directory}"'),
                                j.index("id: apt-cache"))
                paths = re.findall(r"^\s+path: (.+apt[^\n]*)$", j, re.M)
                self.assertEqual(paths, ["${{ runner.temp }}/apt-archives"] *
                                 (2 if name.endswith("-build") else 1))
        self.assertIn('--archives "$RUNNER_TEMP/apt-archives"', job("rust-linux-tier2"))
        self.assertIn('-o "Dir::Cache::archives=$TIER2_APT_ARCHIVES"',
                      job("rust-linux-tier2-build"))

    def test_build_job_waits_for_the_shared_archive_lock(self):
        # On the Dell every Tier 2 build shares /var/cache/obs-runner/apt-archives.
        # apt takes that directory's fcntl lock without a timeout
        # (DPkg::Lock::Timeout covers only dpkg's lock), and another container
        # holding it shows as "held by process 0". Three retries 15 s and 30 s
        # apart then failed run 44049453484 (#4954). Queue on our own flock
        # instead, long enough for a cold archive to download.
        j = job("rust-linux-tier2-build")
        install = j[j.index("name: Install Tier 2 Packages"):]
        install = install[:install.index("\n      - ")]
        lock = 'flock -w 1800 "$TIER2_APT_ARCHIVES/.tier2-install.lock"'
        self.assertIn(lock, install)
        command = install[install.index("xargs "):]
        self.assertLess(command.index(lock), command.index("sudo eatmydata apt-get install"))

    def test_tier2_install_uses_eatmydata(self):
        # force-unsafe-io still syncs dpkg's database for every package. On
        # Dell's HDD mirror the same install took 468 s plain, 47 s wrapped.
        j = job("rust-linux-tier2-build")
        install = j[j.index("name: Install Tier 2 Packages"):]
        install = install.split("\n      - ", 1)[0]
        self.assertIn("sudo eatmydata apt-get install", install)

    def test_shared_archive_is_explicitly_selected(self):
        step = job("rust-linux-tier2-build").split("name: Install Tier 2 Packages", 1)[1]
        step = step.split("\n      - ", 1)[0]
        self.assertIn("TIER2_APT_SHARED: ${{ steps.pcache.outputs.apt != '' }}", step)
        self.assertNotIn("Debug::NoLocking", step)

    @unittest.skipUnless(os.name == "posix" and shutil.which("flock"), "requires POSIX flock")
    def test_shared_downloads_unlock_before_concurrent_private_installs(self):
        # Execute the workflow, with real flock and mocked apt/dpkg. Hold the
        # first installer at a FIFO barrier: the second must finish while it
        # is still blocked. No timing-based sleeps or actual package changes.
        mock = r'''#!/bin/sh
set -eu
case "$*" in
  'apt-get install --download-only '*) phase=download ;;
  'eatmydata dpkg --configure -a') phase=configure ;;
  'eatmydata apt-get -f install '*) phase=repair ;;
  'eatmydata apt-get install '*) phase=install ;;
  *) exit 99 ;;
esac
cache=
for arg do
  case "$arg" in Dir::Cache::archives=*) cache=${arg#*=} ;; esac
done
if flock -n "$SHARED_ARCHIVES/.tier2-install.lock" true; then
  locked=no
else
  locked=yes
fi
printf '%s\t%s\t%s\n' "$phase" "$locked" "$cache" >> "$TRACE"
printf '%s\n' "$@" > "$TRACE.$phase.args"
if [ "$phase" = download ]; then
  [ "$locked" = yes ] && [ "$cache" = "$SHARED_ARCHIVES" ]
  count=$(grep -c '^download' "$TRACE")
  [ "$count" -gt "$FAIL_DOWNLOADS" ] || exit 42
  printf 'complete package\n' > "$cache/fixture.deb"
else
  [ "$locked" = no ]
  if [ "$phase" != configure ]; then
    [ "$cache" != "$SHARED_ARCHIVES" ]
    [ "$(cat "$cache/fixture.deb")" = 'complete package' ]
    [ ! -e "$cache/partial/shared-partial" ]
    mkdir -p "$cache/partial"
    printf 'private lock\n' > "$cache/lock"
  fi
  if [ "$phase" = install ]; then
    printf 'private mutation\n' > "$cache/fixture.deb"
    if [ "$BLOCK_INSTALL" = yes ]; then
      printf 'install-ready\n' > "$READY_FIFO"
      read -r release < "$RELEASE_FIFO"
    fi
  fi
fi
'''
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archives = root / "shared archives"
            archives.mkdir()
            (archives / "partial").mkdir()
            (archives / "partial/shared-partial").write_text("unfinished")
            (archives / "lock").write_text("shared lock")
            mock_bin = root / "bin"
            mock_bin.mkdir()
            sudo = mock_bin / "sudo"
            sudo.write_text(mock)
            sudo.chmod(0o755)
            ready, release = root / "ready", root / "release"
            os.mkfifo(ready)
            os.mkfifo(release)
            # O_RDWR prevents opening either end from blocking the test driver.
            ready_fd = os.open(ready, os.O_RDWR | os.O_NONBLOCK)
            release_fd = os.open(release, os.O_RDWR | os.O_NONBLOCK)
            env = {**os.environ, "GITHUB_WORKSPACE": str(WORKFLOW.parents[2]),
                   "PATH": str(mock_bin) + os.pathsep + os.environ["PATH"],
                   "TIER2_APT_ARCHIVES": str(archives), "SHARED_ARCHIVES": str(archives),
                   "TIER2_APT_SHARED": "true", "RETRY_ATTEMPTS": "1", "RETRY_DELAY": "0",
                   "READY_FIFO": str(ready), "RELEASE_FIFO": str(release)}
            command = ["bash", "-e", "-c", install_script()]
            first_temp, second_temp = root / "job one", root / "job two"
            first_temp.mkdir()
            second_temp.mkdir()
            first = subprocess.Popen(
                command, cwd=WORKFLOW.parents[2], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                text=True, start_new_session=True,
                env={**env, "RUNNER_TEMP": str(first_temp), "TRACE": str(root / "first.trace"),
                     "BLOCK_INSTALL": "yes", "FAIL_DOWNLOADS": "1", "RETRY_ATTEMPTS": "3"},
            )
            try:
                self.assertTrue(select.select([ready_fd], [], [], 5)[0],
                                "first job never reached an unlocked private install")
                self.assertEqual(os.read(ready_fd, 128), b"install-ready\n")
                second = subprocess.run(
                    command, cwd=WORKFLOW.parents[2], capture_output=True, text=True, timeout=5,
                    env={**env, "RUNNER_TEMP": str(second_temp), "TRACE": str(root / "second.trace"),
                         "BLOCK_INSTALL": "no", "FAIL_DOWNLOADS": "0"},
                )
                self.assertEqual(second.returncode, 0, second.stderr)
                self.assertIsNone(first.poll(), "first install must remain blocked until released")
                os.write(release_fd, b"release\n")
                _, stderr = first.communicate(timeout=5)
                self.assertEqual(first.returncode, 0, stderr)
                for name, private_root in (("first", first_temp), ("second", second_temp)):
                    calls = [line.split("\t") for line in (root / f"{name}.trace").read_text().splitlines()]
                    self.assertEqual([call[:2] for call in calls],
                                     [["download", "yes"]] * (2 if name == "first" else 1) +
                                     [["configure", "no"],
                                      ["repair", "no"], ["install", "no"]])
                    download_args = (root / f"{name}.trace.download.args").read_text().splitlines()
                    install_args = (root / f"{name}.trace.install.args").read_text().splitlines()
                    self.assertEqual(download_args[:4], ["apt-get", "install", "--download-only", "-o"])
                    self.assertEqual(download_args[5:], install_args[5:])
                    self.assertTrue(download_args[5:7] == ["-y", "--no-install-recommends"])
                    self.assertEqual(calls[0][2], str(archives))
                    cache = Path(calls[-1][2])
                    self.assertTrue(cache.is_relative_to(private_root), cache)
                    self.assertEqual(calls[-2][2], str(cache))
                    self.assertNotEqual((cache / "fixture.deb").stat().st_ino,
                                        (archives / "fixture.deb").stat().st_ino)
                self.assertEqual((archives / "fixture.deb").read_text(), "complete package\n")
                self.assertEqual((archives / "lock").read_text(), "shared lock")
                self.assertEqual((archives / "partial/shared-partial").read_text(), "unfinished")
            finally:
                os.write(release_fd, b"release\n")
                if first.poll() is None:
                    os.killpg(first.pid, signal.SIGKILL)
                first.communicate(timeout=5)
                os.close(ready_fd)
                os.close(release_fd)

    def test_eatmydata_is_installed_before_tier2_packages(self):
        j = job("rust-linux-tier2-build")
        bootstrap = j.index("name: Install Git")
        tier2 = j.index("name: Install Tier 2 Packages")
        self.assertLess(bootstrap, tier2)
        install_git = j[bootstrap:].split("\n      - ", 1)[0]
        self.assertRegex(install_git, r"apt-get install[^\n]*\beatmydata\b")

    def test_tier2_install_recovers_dpkg_before_each_retry(self):
        # Job 134123589625 was interrupted mid-unpack; retrying apt alone
        # then failed on unmet dependencies. Execute the real step/retry helper
        # with mocked sudo so recovery ordering is checked across attempts.
        # Model /bin/sh dropping Bash functions even where sh is Bash.
        script = "unset -f sudo\n" + install_script()
        # The retry helper uses /bin/sh, which can discard exported Bash
        # functions. A PATH executable survives that boundary on hosted Ubuntu.
        mock = r'''#!/bin/sh
printf '%s\n' "$*" >> "$TRACE"
case "$*" in
  'eatmydata dpkg --configure -a') exit "$DPKG_STATUS" ;;
  'eatmydata apt-get -f install -y '*) exit 0 ;;
  'eatmydata apt-get install '*)
    count=$(grep -c '^eatmydata apt-get install ' "$TRACE")
    [ "$count" -gt "$FAIL_INSTALLS" ] ;;
  *) exit 99 ;;
esac
'''
        for failures, dpkg_status in ((0, 0), (2, 0), (2, 1), (3, 0)):
            with self.subTest(failures=failures, dpkg_status=dpkg_status), tempfile.TemporaryDirectory() as tmp:
                archives = Path(tmp) / "apt archives"
                archives.mkdir()
                trace = Path(tmp) / "trace"
                mock_bin = Path(tmp) / "bin"
                mock_bin.mkdir()
                sudo = mock_bin / "sudo"
                sudo.write_text(mock)
                sudo.chmod(0o755)
                result = subprocess.run(
                    ["bash", "-e", "-c", script],
                    cwd=WORKFLOW.parents[2], capture_output=True, text=True,
                    env={**os.environ, "GITHUB_WORKSPACE": str(WORKFLOW.parents[2]),
                         "PATH": str(mock_bin) + os.pathsep + os.environ["PATH"],
                         "TIER2_APT_ARCHIVES": str(archives), "TRACE": str(trace),
                         "TIER2_APT_SHARED": "false", "RUNNER_TEMP": tmp,
                         "FAIL_INSTALLS": str(failures), "DPKG_STATUS": str(dpkg_status),
                         "RETRY_ATTEMPTS": "3", "RETRY_DELAY": "0"},
                    timeout=10,
                )
                self.assertEqual(result.returncode == 0, failures < 3, result.stderr)
                calls = trace.read_text().splitlines()
                attempts = min(failures + 1, 3)
                self.assertEqual(len(calls), attempts * 3, calls)
                for attempt in range(attempts):
                    configure, repair, install_call = calls[attempt * 3:attempt * 3 + 3]
                    self.assertEqual(configure, "eatmydata dpkg --configure -a")
                    self.assertEqual(repair, f"eatmydata apt-get -f install -y -o Dir::Cache::archives={archives}")
                    self.assertTrue(install_call.startswith(
                        f"eatmydata apt-get install -o Dir::Cache::archives={archives} -y --no-install-recommends "), install_call)

    def test_build_job_dpkg_skips_fsync_and_docs(self):
        # #2072: dpkg spent 29 s unpacking with the .debs already restored.
        j = job("rust-linux-tier2-build")
        self.assertIn("force-unsafe-io", j)
        self.assertIn("path-exclude=/usr/share/doc/*", j)
        self.assertLess(j.index("force-unsafe-io"), j.index("name: Install Tier 2 Packages"))

    def test_cache_is_restored_before_the_install(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            self.assertLess(j.index("path: ${{ runner.temp }}/apt-archives"), j.index("Install Tier 2 Packages" if name.endswith("-build") else "Test Tier 2 ("), name)
            self.assertLess(j.index("actions/checkout@"), j.index("path: ${{ runner.temp }}/apt-archives"), name)

    def test_package_index_is_fetched_once_per_fresh_container(self):
        for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
            j = job(name)
            updates = re.findall(r"^\s+(?:timeout --kill-after=10s \d+ )?(?:sudo )?apt-get update[^\n]*", j, re.M)
            if name.endswith("-build"):
                self.assertEqual(len(updates), 1, name)
                self.assertLess(j.index(updates[0].strip()), j.index("actions/checkout@"), name)
            else:
                helper = (WORKFLOW.parents[2]/"rust/tools/linux-validate/ci-runtime.py").read_text()
                self.assertEqual(helper.count("'apt-get', 'update'"), 1)
                self.assertIn("if variant == 'stock':", helper)
                self.assertEqual(len(updates), 0)

    def test_build_index_fetch_skips_unused_suites_and_components(self):
        # Every Tier 2 build package (and git) resolves from main/universe in
        # resolute, -updates and -security; restricted, multiverse and
        # backports were 8 of 17 index downloads on the critical path.
        j = job("rust-linux-tier2-build")
        install_git = j[j.index("Install Git"):j.index("actions/checkout@")]
        update = install_git.index("apt-get update")
        trim = install_git.index("/etc/apt/sources.list.d/ubuntu.sources")
        self.assertLess(trim, update)
        self.assertIn("Suites: resolute resolute-updates/", install_git)
        self.assertIn("Components: main universe/", install_git)
        # The edit must fail loudly if the stock image's layout changes.
        self.assertIn("grep -qx 'Components: main universe'", install_git)

    def test_install_git_retries_with_a_fresh_index(self):
        # Run 42755735949: a stale index 404'd perl from security.ubuntu.com
        # and one mirror connection failed; one apt attempt failed the job.
        j = job("rust-linux-tier2-build")
        install_git = j[j.index("Install Git"):j.index("actions/checkout@")]
        self.assertIn("for attempt in 1 2 3", install_git)
        self.assertIn("--fix-missing", install_git)

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
