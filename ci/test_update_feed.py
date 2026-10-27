#!/usr/bin/env -S uv run --script
"""The in-app updaters offer only certified obs-rust releases.

Usage:
    uv run python -m unittest ci/test_update_feed.py

release-rust.yaml publishes each daily beta as a GitHub pre-release. A
maintainer certifies one by dispatching the workflow with `certify=<tag>`: that
job runs ci/update_feed.py to write the Sparkle appcasts, the Windows update
manifest and branches.json (RSA-signed) as assets of that release, then makes
it a full release marked Latest. The updaters read
https://github.com/obs-rust/obs-studio/releases/latest/download/..., which
GitHub resolves to the newest full release only, so a daily or nightly build is
never offered.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci" / "update_feed.py"
RELEASE = ROOT / ".github" / "workflows" / "release-rust.yaml"
VERSION_HEADER = ROOT / "frontend" / "utility" / "obs-rust-update.hpp"
TAG = "obs-studio-rust-v0.1.0-beta.20261020"
VERSION = "0.1.0-beta.20261020"
DOWNLOAD = f"https://github.com/obs-rust/obs-studio/releases/download/{TAG}/"
SPARKLE = "http://www.andymatuschak.org/xml-namespaces/sparkle"
UV = shutil.which("uv")
CRYPTO = "cryptography>=50.0.2,<51"
KEYGEN = """
import sys
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa
key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
open(sys.argv[1], "wb").write(key.private_bytes(serialization.Encoding.PEM,
    serialization.PrivateFormat.PKCS8, serialization.NoEncryption()))
open(sys.argv[2], "wb").write(key.public_key().public_bytes(serialization.Encoding.PEM,
    serialization.PublicFormat.SubjectPublicKeyInfo))
"""
# The app checks RSA PKCS#1 v1.5 over SHA-512 (crypto-helpers-*.cpp.hpp).
VERIFY = """
import sys
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding
pub = serialization.load_pem_public_key(open(sys.argv[1], "rb").read())
sig = bytes.fromhex(open(sys.argv[3]).read().strip())
pub.verify(sig, open(sys.argv[2], "rb").read(), padding.PKCS1v15(), hashes.SHA512())
"""


def uv_python(code: str, *args: str) -> subprocess.CompletedProcess:
    env = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
    with tempfile.TemporaryDirectory() as tmp:
        script = Path(tmp) / "helper.py"
        script.write_text(code)
        return subprocess.run(
            [UV, "run", "--no-project", "--with", CRYPTO, str(script), *args],
            capture_output=True, text=True, env=env,
        )


def job(text: str, name: str) -> str:
    match = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)", text, re.M | re.S)
    if not match:
        raise AssertionError(f"release-rust.yaml has no {name} job")
    return match.group(1)


def release_commands(text: str) -> list[str]:
    return [m.group(0) for m in re.finditer(r'gh release (?:create|edit) "\$TAG"[^\n]*(?:\\\n[^\n]*)*', text)]


@unittest.skipUnless(UV, "uv is required")
class UpdateFeedScriptTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.key = self.tmp / "throwaway.pem"
        self.pub = self.tmp / "throwaway.pub.pem"
        made = uv_python(KEYGEN, str(self.key), str(self.pub))
        self.assertEqual(made.returncode, 0, made.stderr)
        self.assets = self.tmp / "assets"
        self.assets.mkdir()
        sums = []
        for i, name in enumerate(
            (
                f"OBS-Studio-Rust-{VERSION}-macos-arm64.dmg",
                f"OBS-Studio-Rust-{VERSION}-macos-x86_64.dmg",
                f"OBS-Studio-Rust-{VERSION}-windows-x64.zip",
                f"OBS-Studio-Rust-{VERSION}-windows-arm64.zip",
            )
        ):
            data = f"{name} payload {i}".encode() * (i + 1)
            (self.assets / name).write_bytes(data)
            sums.append(f"{hashlib.sha256(data).hexdigest()}  {name}")
        (self.assets / "SHA256SUMS").write_text("\n".join(sums) + "\n")
        self.mac = self.tmp / "mac.json"
        self.mac.write_text(
            json.dumps(
                {
                    arch: {
                        "bundle_version": "20261020",
                        "short_version": VERSION,
                        "minimum_system_version": "12.0",
                        "ed_signature": f"edsig-{arch}==",
                    }
                    for arch in ("arm64", "x86_64")
                }
            )
        )
        self.out = self.tmp / "feed"

    def build(self, tag: str = TAG, *extra: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [
                UV, "run", "--script", str(SCRIPT), "build",
                "--tag", tag, "--assets", str(self.assets), "--mac-info", str(self.mac),
                "--rsa-key", str(self.key), "--out", str(self.out), *extra,
            ],
            capture_output=True,
            text=True,
            env={k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"},
        )

    def verify(self, name: str) -> None:
        result = uv_python(VERIFY, str(self.pub), str(self.out / name), str(self.out / f"{name}.sig"))
        self.assertEqual(result.returncode, 0, f"{name}: {result.stderr}")

    def test_script_pins_the_cryptography_these_tests_run(self) -> None:
        # The release job runs the script's own inline pin; keep it on the
        # line the tests exercise (#2371).
        script = (Path(__file__).resolve().parent / "update_feed.py").read_text(encoding="utf-8")
        self.assertIn(f'# dependencies = ["{CRYPTO}"]', script)

    def test_writes_signed_feed_for_the_certified_release(self) -> None:
        result = self.build()
        self.assertEqual(result.returncode, 0, result.stderr)
        for arch in ("arm64", "x86_64"):
            with self.subTest(arch=arch):
                dmg = f"OBS-Studio-Rust-{VERSION}-macos-{arch}.dmg"
                tree = ET.parse(self.out / f"updates_{arch}_v2.xml")
                items = tree.getroot().findall("./channel/item")
                self.assertEqual(len(items), 1)
                item = items[0]
                self.assertEqual(item.findtext(f"{{{SPARKLE}}}version"), "20261020")
                self.assertEqual(item.findtext(f"{{{SPARKLE}}}shortVersionString"), VERSION)
                self.assertEqual(item.findtext(f"{{{SPARKLE}}}minimumSystemVersion"), "12.0")
                self.assertIsNone(item.find(f"{{{SPARKLE}}}channel"), "certified releases are the default channel")
                enclosure = item.find("enclosure")
                self.assertEqual(enclosure.get("url"), DOWNLOAD + dmg)
                self.assertEqual(enclosure.get(f"{{{SPARKLE}}}edSignature"), f"edsig-{arch}==")
                self.assertEqual(int(enclosure.get("length")), (self.assets / dmg).stat().st_size)
                self.assertNotIn("obsproject.com", (self.out / f"updates_{arch}_v2.xml").read_text())

        manifest = json.loads((self.out / "manifest.json").read_text())
        self.assertEqual(manifest["version"], VERSION)
        self.assertEqual(manifest["tag"], TAG)
        self.assertIn(f"https://github.com/obs-rust/obs-studio/releases/tag/{TAG}", manifest["notes"])
        for arch in ("x64", "arm64"):
            with self.subTest(arch=arch):
                name = f"OBS-Studio-Rust-{VERSION}-windows-{arch}.zip"
                data = (self.assets / name).read_bytes()
                entry = manifest["windows"][arch]
                self.assertEqual(entry["url"], DOWNLOAD + name)
                self.assertEqual(entry["sha256"], hashlib.sha256(data).hexdigest())
                self.assertEqual(entry["size"], len(data))
        self.assertEqual(json.loads((self.out / "branches.json").read_text()), [])
        self.verify("manifest.json")
        self.verify("branches.json")

    def test_writes_signed_nightly_feed_for_the_continuous_release(self) -> None:
        # #1759: the daily run writes this onto the rolling "continuous"
        # pre-release; it names the daily's own assets.
        result = self.build(TAG, "--channel", "nightly")
        self.assertEqual(result.returncode, 0, result.stderr)
        for arch in ("arm64", "x86_64"):
            with self.subTest(arch=arch):
                dmg = f"OBS-Studio-Rust-{VERSION}-macos-{arch}.dmg"
                items = ET.parse(self.out / f"updates_{arch}_v2.xml").getroot().findall("./channel/item")
                self.assertEqual(len(items), 1)
                self.assertEqual(items[0].findtext(f"{{{SPARKLE}}}channel"), "nightly")
                self.assertEqual(items[0].find("enclosure").get("url"), DOWNLOAD + dmg)
        manifest = json.loads((self.out / "manifest.json").read_text())
        self.assertEqual(manifest["channel"], "nightly")
        self.assertEqual(manifest["tag"], TAG)
        self.assertEqual(manifest["windows"]["x64"]["url"], DOWNLOAD + f"OBS-Studio-Rust-{VERSION}-windows-x64.zip")
        self.verify("manifest.json")
        # Branches come from the certified feed only.
        self.assertFalse((self.out / "branches.json").exists())
        self.assertFalse((self.out / "branches.json.sig").exists())

    def test_stable_feed_is_the_default_channel(self) -> None:
        result = self.build(TAG, "--channel", "stable")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads((self.out / "manifest.json").read_text())["channel"], "stable")
        self.assertTrue((self.out / "branches.json").exists())

    def test_rejects_unknown_channels(self) -> None:
        self.assertNotEqual(self.build(TAG, "--channel", "beta").returncode, 0)

    def test_rejects_an_asset_that_does_not_match_sha256sums(self) -> None:
        (self.assets / f"OBS-Studio-Rust-{VERSION}-windows-x64.zip").write_bytes(b"tampered")
        result = self.build()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("SHA256SUMS", result.stderr)
        self.assertFalse((self.out / "manifest.json").exists())

    def test_rejects_tags_that_are_not_obs_rust_releases(self) -> None:
        for tag in ("continuous", "update-feed", "31.1.0"):
            with self.subTest(tag=tag):
                self.assertNotEqual(self.build(tag).returncode, 0)


def load_update_feed():
    import importlib.util

    spec = importlib.util.spec_from_file_location("update_feed", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SHA = "0123456789abcdef0123456789abcdef01234567"
NOW = "2026-10-22T09:00:00Z"


def green_run(run_id: int = 100, sha: str = SHA, event: str = "schedule") -> dict:
    jobs = [
        "Check version 🏷️",
        "Build ubuntu-26.04 x86_64 🧱",
        "Build windows-2025 x64 🧱",
        "Build macos-26 arm64 🧱",
        "AppImage 📦",
        "Native macOS arm64 DMG startup 🧪",
        "Release gate 🚦",
        "Publish pre-release 🚀",
        "Release gate 🚦 / Fast Gate ⚡",
        "Continuous AppImage ♻️",
    ]
    return {
        "id": run_id,
        "head_sha": sha,
        "event": event,
        "status": "completed",
        "conclusion": "success",
        "jobs": [{"name": name, "conclusion": "success"} for name in jobs]
        # Skipped by design: a reusable workflow's own job, and certify itself.
        + [
            {"name": "AppImage 📦 / Publish to GitHub Releases 🚀", "conclusion": "skipped"},
            {"name": "Certify release 🏅", "conclusion": "skipped"},
        ],
    }


def facts(published: str = "2026-10-20T09:00:00Z", runs: list | None = None) -> dict:
    return {
        "tag": TAG,
        "tag_sha": SHA,
        "release": {"isDraft": False, "isPrerelease": True, "publishedAt": published},
        "runs": [green_run()] if runs is None else runs,
    }


class CheckCertifiableTests(unittest.TestCase):
    """The certify gate (#1757): a fully green producing run, and an aged release."""

    def setUp(self) -> None:
        self.feed = load_update_feed()

    def refused(self, data: dict, min_age_days: float = 2, now: str = NOW) -> str:
        with self.assertRaises(self.feed.FeedError) as caught:
            self.feed.check_certifiable(data, now, min_age_days)
        return str(caught.exception)

    def test_green_release_two_days_old_is_certifiable(self) -> None:
        self.assertEqual(self.feed.check_certifiable(facts(), NOW, 2), [])

    def test_refuses_a_release_younger_than_min_age(self) -> None:
        message = self.refused(facts(published="2026-10-21T09:00:01Z"))
        self.assertIn("days old", message)
        self.assertIn("min_age_days", message)

    def test_min_age_is_configurable(self) -> None:
        young = facts(published="2026-10-21T21:00:00Z")
        self.assertEqual(self.feed.check_certifiable(young, NOW, 0.5), [])
        self.refused(young, min_age_days=3)

    def test_zero_min_age_is_an_emergency_override_with_a_warning(self) -> None:
        warnings = self.feed.check_certifiable(facts(published="2026-10-22T08:59:00Z"), NOW, 0)
        self.assertEqual(len(warnings), 1)
        self.assertIn("min_age_days=0", warnings[0])

    def test_negative_min_age_is_rejected(self) -> None:
        self.assertIn("min_age_days", self.refused(facts(), min_age_days=-1))

    def test_refuses_when_any_job_of_the_producing_run_did_not_succeed(self) -> None:
        for name, conclusion in (
            ("Native macOS arm64 DMG startup 🧪", "failure"),
            ("Build windows-2025 x64 🧱", "cancelled"),
            ("AppImage 📦", "skipped"),
            ("Continuous AppImage ♻️", "timed_out"),
            ("Release gate 🚦", None),
            ("Release gate 🚦", "skipped"),
            ("Release gate 🚦 / Fast Gate ⚡", "failure"),
        ):
            with self.subTest(name=name, conclusion=conclusion):
                run = green_run()
                for job in run["jobs"]:
                    if job["name"] == name:
                        job["conclusion"] = conclusion
                run["conclusion"] = "failure"
                message = self.refused(facts(runs=[run]))
                self.assertIn(name, message)
                self.assertIn("run 100", message)

    def test_refuses_when_the_run_itself_did_not_succeed(self) -> None:
        run = green_run()
        run["conclusion"] = "failure"
        self.assertIn("run 100", self.refused(facts(runs=[run])))

    def test_refuses_without_a_run_that_published_the_tag(self) -> None:
        other = green_run(sha="f" * 40)
        unpublished = green_run(run_id=101)
        for job in unpublished["jobs"]:
            if job["name"].startswith("Publish pre-release"):
                job["conclusion"] = "skipped"
        self.assertIn(SHA[:9], self.refused(facts(runs=[other, unpublished])))
        self.assertIn(SHA[:9], self.refused(facts(runs=[])))

    def test_the_latest_publishing_run_decides(self) -> None:
        failed = green_run(run_id=100)
        failed["conclusion"] = "failure"
        failed["jobs"][-3]["conclusion"] = "failure"  # continuous, after publishing
        repair = green_run(run_id=105, event="workflow_dispatch")
        self.assertEqual(self.feed.check_certifiable(facts(runs=[failed, repair]), NOW, 2), [])
        self.refused(facts(runs=[repair, {**failed, "id": 110}]))

    def test_ignores_the_running_certify_dispatch(self) -> None:
        certify = {
            "id": 200,
            "head_sha": SHA,
            "event": "workflow_dispatch",
            "status": "in_progress",
            "conclusion": None,
            "jobs": [{"name": "Check version 🏷️", "conclusion": "skipped"}, {"name": "Certify release 🏅", "conclusion": None}],
        }
        self.assertEqual(self.feed.check_certifiable(facts(runs=[green_run(), certify]), NOW, 2), [])

    def test_refuses_a_draft_or_mismatched_tag(self) -> None:
        draft = facts()
        draft["release"]["isDraft"] = True
        self.assertIn("draft", self.refused(draft))
        bad = facts()
        bad["tag"] = "continuous"
        self.refused(bad)

    @unittest.skipUnless(UV, "uv is required")
    def test_command_line_reports_and_exits_nonzero(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "facts.json"
            env = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
            base = [UV, "run", "--script", str(SCRIPT), "check-certifiable", "--facts", str(path), "--now", NOW]
            path.write_text(json.dumps(facts()))
            ok = subprocess.run([*base, "--min-age-days", "2"], capture_output=True, text=True, env=env)
            self.assertEqual(ok.returncode, 0, ok.stderr)
            override = subprocess.run([*base, "--min-age-days", "0"], capture_output=True, text=True, env=env)
            self.assertEqual(override.returncode, 0, override.stderr)
            self.assertIn("::warning::", override.stdout)
            path.write_text(json.dumps(facts(published="2026-10-21T12:00:00Z")))
            young = subprocess.run([*base, "--min-age-days", "2"], capture_output=True, text=True, env=env)
            self.assertNotEqual(young.returncode, 0)
            self.assertIn("::error::", young.stdout + young.stderr)


class CertifiedReleaseWorkflowTests(unittest.TestCase):
    def setUp(self) -> None:
        self.text = RELEASE.read_text(encoding="utf-8")

    def test_daily_release_is_a_pre_release(self) -> None:
        commands = release_commands(job(self.text, "release"))
        self.assertTrue(commands)
        for command in commands:
            with self.subTest(command=command):
                self.assertRegex(command, r"--prerelease(?!=false)")
                self.assertIn("--latest=false", command)

    def test_only_the_certify_job_makes_a_full_release(self) -> None:
        certify = job(self.text, "certify")
        outside = self.text.replace(certify, "")
        self.assertNotIn("--prerelease=false", outside)
        self.assertIsNone(re.search(r"--latest(?!=false)", outside.replace("releases/latest", "")))
        self.assertIn("--prerelease=false", certify)
        self.assertRegex(certify, r"--latest(?!=false)")

    def test_certify_is_a_maintainer_dispatch_of_a_gated_pre_release(self) -> None:
        inputs = self.text.split("workflow_dispatch:", 1)[1].split("permissions:", 1)[0]
        self.assertRegex(inputs, r"\n      certify:\n")
        certify = job(self.text, "certify")
        self.assertIn("github.event_name == 'workflow_dispatch'", certify)
        self.assertIn("inputs.certify", certify)
        self.assertIn("github.repository == 'obs-rust/obs-studio'", certify)
        # The feed is attached before the release becomes Latest, so
        # releases/latest/download/... never points at a release without it.
        upload = certify.index("gh release upload")
        self.assertLess(upload, certify.index("--prerelease=false"))
        self.assertIn("ci/update_feed.py build", certify)
        self.assertIn("secrets.OBS_RUST_SPARKLE_PRIVATE_KEY", certify)
        self.assertIn("secrets.OBS_RUST_UPDATE_RSA_PRIVATE_KEY", certify)
        # Only a gated daily: an obs-studio-rust-v* pre-release (release only
        # publishes after the gate) whose assets match its SHA256SUMS.
        self.assertIn("obs-studio-rust-v", certify)
        self.assertIn("isPrerelease", certify)
        # The gate (#1757): a fully green producing run, an aged release.
        self.assertIn("ci/update_feed.py check-certifiable", certify)
        self.assertIn("inputs.min_age_days", certify)
        self.assertLess(certify.index("check-certifiable"), upload)
        self.assertRegex(inputs, r"\n      min_age_days:\n(?:        .*\n)*?        default: '2'\n")
        # The daily build must not run on a certify dispatch.
        self.assertIn("inputs.certify == ''", job(self.text, "check"))

    def test_daily_run_writes_the_nightly_feed_onto_continuous(self) -> None:
        nightly = job(self.text, "nightly_feed")
        self.assertRegex(nightly, r"needs: \[check, release\]")
        # Only after a published (not verify-only) daily.
        self.assertIn("needs.release.result == 'success'", nightly)
        self.assertIn("needs.check.outputs.verify_only != 'true'", nightly)
        self.assertIn("ci/update_feed.py build", nightly)
        self.assertIn("--channel nightly", nightly)
        self.assertIn("gh release upload continuous", nightly)
        self.assertNotIn('gh release edit "$TAG"', nightly)
        self.assertIsNone(re.search(r"--latest(?!=false)", nightly.replace("releases/latest", "")))
        self.assertIn("secrets.OBS_RUST_SPARKLE_PRIVATE_KEY", nightly)
        self.assertIn("secrets.OBS_RUST_UPDATE_RSA_PRIVATE_KEY", nightly)
        # Missing secrets skip with a warning; a daily never fails on them.
        missing = nightly.split('if [ -z "$SPARKLE_KEY" ] || [ -z "$RSA_KEY" ]; then', 1)[1].split("fi", 1)[0]
        self.assertIn("::warning::", missing)
        self.assertIn("exit 0", missing)
        self.assertNotIn("exit 1", missing)
        # The signing step itself is shared with certify.
        self.assertIn("ci/sparkle_mac_info.py", nightly)
        self.assertIn("ci/sparkle_mac_info.py", job(self.text, "certify"))


class SparkleMacInfoTests(unittest.TestCase):
    """The macOS half of a feed's input, read out of the shipped DMGs (#1742)."""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.dist = self.root / "dist"
        self.dist.mkdir()
        for arch in ("arm64", "x86_64"):
            (self.dist / f"obs-studio-rust-0.1.0-macos-{arch}.dmg").write_bytes(b"dmg")
        self.calls = self.root / "calls.log"
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.stub("hdiutil", """
            echo "hdiutil $*" >>"$CALLS"
            [[ "$1" == attach ]] || exit 0
            mount=''; previous=''
            for arg in "$@"; do [[ "$previous" == -mountpoint ]] && mount=$arg; previous=$arg; done
            mkdir -p "$mount/OBS.app/Contents"
            : >"$mount/OBS.app/Contents/Info.plist"
        """)
        self.stub("plutil", """
            case "$2" in
                CFBundleVersion) echo 20261020 ;;
                CFBundleShortVersionString) echo 0.1.0 ;;
                LSMinimumSystemVersion) echo '12.0' ;;
                *) exit 1 ;;
            esac
        """)
        self.stub("sign_update", """
            [[ -n "${NO_SIGNATURE:-}" ]] && exit 0
            echo "sparkle:edSignature=\\"sig-$(basename "${!#}" .dmg)\\" length=\\"1\\""
        """)
        self.key = self.root / "sparkle.key"
        self.key.write_text("key")
        self.sign_update = self.bin / "sign_update"
        self.out = self.root / "mac.json"

    def stub(self, name: str, body: str) -> None:
        path = self.bin / name
        path.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + body)
        path.chmod(0o755)

    def run_script(self, *args: str, **env: str) -> subprocess.CompletedProcess:
        environment = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
        environment.update(PATH=f"{self.bin}{os.pathsep}{environment['PATH']}", CALLS=str(self.calls), **env)
        return subprocess.run(
            [sys.executable, str(ROOT / "ci" / "sparkle_mac_info.py"),
             str(self.dist), str(self.key), str(self.sign_update), str(self.out), *args],
            capture_output=True, text=True, env=environment,
        )

    def test_each_architecture_gets_its_plist_fields_and_signature(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(self.out.read_text(encoding="utf-8")), {
            "arm64": {"bundle_version": "20261020", "short_version": "0.1.0",
                      "minimum_system_version": "12.0",
                      "ed_signature": "sig-obs-studio-rust-0.1.0-macos-arm64"},
            "x86_64": {"bundle_version": "20261020", "short_version": "0.1.0",
                       "minimum_system_version": "12.0",
                       "ed_signature": "sig-obs-studio-rust-0.1.0-macos-x86_64"},
        })
        verbs = [line.split()[1] for line in self.calls.read_text().splitlines()]
        self.assertEqual(verbs, ["attach", "detach", "attach", "detach"])

    def test_a_signature_that_does_not_parse_fails_the_step(self) -> None:
        result = self.run_script(NO_SIGNATURE="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("::error::sign_update produced no signature", result.stderr)

    def test_an_architecture_with_two_dmgs_is_refused(self) -> None:
        for name in ("a", "b"):
            (self.dist / f"{name}-macos-arm64.dmg").write_bytes(b"dmg")
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exactly one", result.stderr)


class NightlyFeedUrlTests(unittest.TestCase):
    """frontend/utility/obs-rust-update.hpp's channel mapping (#1759)."""

    @unittest.skipUnless(shutil.which("c++"), "a C++ compiler is required")
    def test_feed_url_per_branch(self) -> None:
        stable = "https://github.com/obs-rust/obs-studio/releases/latest/download/"
        nightly = "https://github.com/obs-rust/obs-studio/releases/download/continuous/"
        cases = [
            (stable + "manifest.json", "stable", stable + "manifest.json"),
            (stable + "manifest.json", "nightly", nightly + "manifest.json"),
            (stable + "updates_arm64_v2.xml", "nightly", nightly + "updates_arm64_v2.xml"),
            (stable + "updates_x86_64_v2.xml", "beta", stable + "updates_x86_64_v2.xml"),
            (stable + "manifest.json", "", stable + "manifest.json"),
            # Never rewrite a URL that is not obs-rust's certified feed.
            ("https://example.com/releases/latest/download/x.xml", "nightly", "https://example.com/releases/latest/download/x.xml"),
        ]
        known = [("stable", 1), ("nightly", 1), ("beta", 0), ("", 0), ("Nightly", 0)]
        body = "\n".join(
            f'\tif (ObsRustFeedUrl("{a}", "{b}") != "{want}") {{ std::puts("{a} {b}"); bad = 1; }}'
            for a, b, want in cases
        ) + "\n" + "\n".join(
            f'\tif (ObsRustUpdateBranchIsKnown("{b}") != {want}) {{ std::puts("known {b}"); bad = 1; }}'
            for b, want in known
        )
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "t.cpp"
            src.write_text(
                f'#include "{VERSION_HEADER}"\n#include <cstdio>\n'
                f"int main() {{\n\tint bad = 0;\n{body}\n\treturn bad;\n}}\n"
            )
            exe = Path(tmp) / "t"
            built = subprocess.run(["c++", "-std=c++17", "-o", str(exe), str(src)], capture_output=True, text=True)
            self.assertEqual(built.returncode, 0, built.stderr)
            run = subprocess.run([str(exe)], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stdout)


@unittest.skipUnless(shutil.which("c++"), "a C++ compiler is required")
class VersionCompareTests(unittest.TestCase):
    def test_semver_precedence(self) -> None:
        cases = [
            ("0.1.0-beta.20261020", "0.1.0-beta.20261018", 1),
            ("0.1.0-beta.20261018", "0.1.0-beta.20261020", -1),
            ("0.1.0-beta.20261020", "0.1.0-beta.20261020", 0),
            ("0.1.0", "0.1.0-beta.20261020", 1),
            ("0.2.0-beta.1", "0.1.0", 1),
            ("0.1.0-beta.9", "0.1.0-beta.10", -1),
            ("0.1.0-alpha", "0.1.0-beta", -1),
            ("1.0.0", "0.99.99", 1),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "t.cpp"
            body = "\n".join(
                f'\tif (sign(CompareObsRustVersions("{a}", "{b}")) != {want}) {{ std::puts("{a} vs {b}"); bad = 1; }}'
                for a, b, want in cases
            )
            src.write_text(
                f'#include "{VERSION_HEADER}"\n#include <cstdio>\n'
                "static int sign(int v) { return (v > 0) - (v < 0); }\n"
                f"int main() {{\n\tint bad = 0;\n{body}\n"
                '\tif (ObsRustVersionIsValid("x.y") || !ObsRustVersionIsValid("0.1.0-beta.1")) { std::puts("valid"); bad = 1; }\n'
                "\treturn bad;\n}\n"
            )
            exe = Path(tmp) / "t"
            build = subprocess.run(["c++", "-std=c++17", "-o", str(exe), str(src)], capture_output=True, text=True)
            self.assertEqual(build.returncode, 0, build.stderr)
            run = subprocess.run([str(exe)], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stdout)


if __name__ == "__main__":
    unittest.main()
