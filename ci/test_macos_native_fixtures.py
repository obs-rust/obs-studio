"""Fail-closed contracts for Linux-prepared Darwin native fixtures."""
import json
import io
import hashlib
import subprocess
import tarfile
import tempfile
import threading
import unittest
from concurrent.futures import Future, ThreadPoolExecutor
from pathlib import Path
from unittest.mock import patch

import macos_test_artifact as artifacts
import macos_native_fixtures as fixtures


class ParallelFixtureTest(unittest.TestCase):
    def test_worker_budget_is_conservative_and_override_is_cpu_bounded(self):
        for cpus, expected in ((None, 1), (1, 1), (2, 2), (64, 2)):
            with self.subTest(cpus=cpus), patch.object(fixtures.os, "cpu_count", return_value=cpus):
                self.assertEqual(fixtures.fixture_workers({}), expected)
        with patch.object(fixtures.os, "cpu_count", return_value=4):
            for value, expected in (("1", 1), ("3", 3), ("999", 4)):
                self.assertEqual(fixtures.fixture_workers({"OBS_NATIVE_FIXTURE_WORKERS": value}), expected)
            for value in ("0", "-1", "many", ""):
                with self.subTest(value=value), self.assertRaisesRegex(ValueError, "OBS_NATIVE_FIXTURE_WORKERS"):
                    fixtures.fixture_workers({"OBS_NATIVE_FIXTURE_WORKERS": value})

    def test_prepare_uses_pool_and_preserves_index_order_when_second_finishes_first(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "native-fixtures"
            names = ["native_build::first", "native_build::second"]
            second_finished = threading.Event()
            completed = []

            def run(command, **kwargs):
                if command[0] == "soldr":
                    event = {"reason": "compiler-artifact", "target": {"name": "obs-tests-app"}, "executable": "app-tests"}
                    return subprocess.CompletedProcess(command, 0, json.dumps(event))
                name = command[2]
                self.assertTrue(kwargs["capture_output"])
                self.assertTrue(kwargs["check"])
                if name == names[0]:
                    self.assertTrue(second_finished.wait(3), "second fixture never ran concurrently")
                path = directory / name.replace("::", "--") / "manifest.json"
                path.parent.mkdir()
                path.write_text(json.dumps({"complete": True}))
                completed.append(name)
                if name == names[1]:
                    second_finished.set()
                return subprocess.CompletedProcess(command, 0, name + "\n", "")

            signed = [name.replace("::", "--") + "/fixture.dylib" for name in names]
            with patch.object(fixtures.platform, "system", return_value="Linux"), \
                    patch.object(fixtures, "TESTS", ("first", "second")), \
                    patch.object(fixtures.os, "cpu_count", return_value=4), \
                    patch.object(fixtures, "tool", return_value="tool"), \
                    patch.object(fixtures, "configure_swift"), \
                    patch.object(fixtures, "signed_fixtures", return_value=list(reversed(signed))), \
                    patch.object(fixtures.subprocess, "run", side_effect=run), \
                    patch.object(fixtures, "ThreadPoolExecutor", wraps=ThreadPoolExecutor) as pool:
                fixtures.prepare(root, root, {"OBS_NATIVE_FIXTURE_WORKERS": "2"})
            pool.assert_called_once_with(max_workers=2)
            self.assertEqual(completed, list(reversed(names)))
            index = json.loads((directory / "index.json").read_text())
            self.assertEqual(index["tests"], names)
            self.assertEqual(index["signed"], signed)
            self.assertIs(index["complete"], True)

    def test_first_completed_failure_reports_its_output_and_cancels_pending_work(self):
        first, failed, pending = Future(), Future(), Future()
        failure = subprocess.CalledProcessError(7, ["app-tests", "--exact", "second"],
                                                output="fixture stdout", stderr="fixture stderr")
        failed.set_exception(failure)
        with patch.object(fixtures, "ThreadPoolExecutor") as pool, \
                patch.object(fixtures, "as_completed", return_value=iter((failed, first, pending))), \
                patch("sys.stdout", new_callable=io.StringIO) as stdout, \
                patch("sys.stderr", new_callable=io.StringIO) as stderr:
            pool.return_value.__enter__.return_value.submit.side_effect = (first, failed, pending)
            with self.assertRaises(subprocess.CalledProcessError) as raised:
                fixtures.prepare_fixtures("app-tests", ["first", "second", "third"], Path("unused"), Path("unused"), {}, 2)
        self.assertIs(raised.exception, failure)
        self.assertIn("fixture stdout", stdout.getvalue())
        self.assertIn("fixture stderr", stderr.getvalue())
        self.assertTrue(pending.cancelled())

    def test_incomplete_fixture_still_fails_without_publishing_index(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            name = "native_build::incomplete"
            path = root / name.replace("::", "--") / "manifest.json"
            path.parent.mkdir()
            path.write_text('{"complete": false}')
            with patch.object(fixtures.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "", "")):
                with self.assertRaisesRegex(ValueError, "incomplete native fixture"):
                    fixtures.prepare_fixtures("app-tests", [name], root, root, {}, 1)
            self.assertFalse((root / "index.json").exists())


class NativeFixtureTest(unittest.TestCase):
    def test_complete_inventory_and_missing_per_test_manifest(self):
        from macos_native_fixtures import TESTS
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "native-fixtures"
            directory.mkdir()
            names = ["native_build::" + name for name in TESTS]
            (directory / "index.json").write_text(json.dumps({"version": 1, "target": artifacts.TARGET,
                "tests": names, "signed": ["fixture.dylib"], "complete": True}))
            for name in names:
                path = directory / name.replace("::", "--") / "manifest.json"
                path.parent.mkdir()
                path.write_text(json.dumps({"host": "x86_64-unknown-linux-gnu", "target": artifacts.TARGET,
                    "complete": True, "events": [{}]}))
            with patch.object(artifacts, "verify_macho") as verify:
                artifacts.validate_native_fixtures(root)
                verify.assert_called_once_with(directory / "fixture.dylib", allowed_filetypes=(2, 6, 8))
                path.unlink()
                with self.assertRaises(FileNotFoundError):
                    artifacts.validate_native_fixtures(root)

    def test_fixture_manifest_is_required_and_complete(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises((ValueError, FileNotFoundError)):
                artifacts.validate_native_fixtures(root)
            (root / "native-fixtures").mkdir()
            (root / "native-fixtures/index.json").write_text(json.dumps({
                "version": 1, "target": artifacts.TARGET, "tests": [], "complete": False,
            }))
            with self.assertRaises(ValueError):
                artifacts.validate_native_fixtures(root)

    def test_dylib_signature_uses_same_page_verifier(self):
        # Defaults still reject shared images in the executable collector.
        from test_macos_test_artifact import SignatureTest
        import struct
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "fixture.dylib"
            data = bytearray(SignatureTest.signed_image())
            struct.pack_into("<I", data, 12, 6)
            # Changing the header without re-signing must fail, even for a dylib.
            path.write_bytes(data)
            with self.assertRaises(ValueError):
                artifacts.verify_macho(path, allowed_filetypes=(2, 6, 8))


class DsymutilCacheTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.env = {"RUNNER_TOOL_CACHE": str(self.root / "tools")}
        self.contents = b"pinned debug tool"
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode="w:gz") as archive:
            member = tarfile.TarInfo("./package/bin/dsymutil")
            member.size = len(self.contents)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(self.contents))
        self.payload = output.getvalue()
        self.digest = hashlib.sha256(self.payload).hexdigest()
        for target, name, value in (
                (fixtures.platform, "system", lambda: "Linux"),
                (fixtures.platform, "machine", lambda: "x86_64"),
                (Path, "home", lambda: self.root),
                (fixtures, "LLVM_TOOLS_SHA256", self.digest),
                (fixtures, "DSYMUTIL_SHA256", hashlib.sha256(self.contents).hexdigest())):
            self.enterContext(patch.object(target, name, value))

    def download(self, url, destination):
        self.assertEqual(url, fixtures.LLVM_TOOLS_URL)
        Path(destination).write_bytes(self.payload)

    def test_clean_checkouts_share_verified_executable_without_retaining_archive(self):
        with patch.object(fixtures.urllib.request, "urlretrieve", side_effect=self.download) as download:
            first = fixtures.provision_dsymutil(self.root / "checkout-a", self.env)
            second = fixtures.provision_dsymutil(self.root / "checkout-b", self.env)
        self.assertEqual(first, second)
        self.assertTrue(first.is_relative_to(self.root / "tools"))
        self.assertIn(self.digest, first.parts)
        self.assertEqual(first.read_bytes(), self.contents)
        self.assertEqual(first.stat().st_mode & 0o111, 0o111)
        self.assertEqual(list((self.root / "tools").rglob("*.zst")), [])
        download.assert_called_once()

    def test_interrupted_extraction_never_publishes_partial_tool(self):
        extract = fixtures.tarfile.TarFile.extract

        def interrupted(archive, member, path, **kwargs):
            extract(archive, member, path, **kwargs)
            raise OSError("interrupted extraction")

        with patch.object(fixtures.urllib.request, "urlretrieve", side_effect=self.download) as download:
            with patch.object(fixtures.tarfile.TarFile, "extract", interrupted):
                with self.assertRaisesRegex(OSError, "interrupted"):
                    fixtures.provision_dsymutil(self.root, self.env)
            tool = fixtures.provision_dsymutil(self.root, self.env)
        self.assertEqual(tool.read_bytes(), self.contents)
        self.assertEqual(download.call_count, 2)

    def test_corrupt_shared_tool_is_rejected_without_network(self):
        with patch.object(fixtures.urllib.request, "urlretrieve", side_effect=self.download):
            tool = fixtures.provision_dsymutil(self.root, self.env)
        tool.write_bytes(b"corrupt")
        with patch.object(fixtures.urllib.request, "urlretrieve") as download:
            with self.assertRaisesRegex(ValueError, "dsymutil checksum mismatch"):
                fixtures.provision_dsymutil(self.root, self.env)
            download.assert_not_called()

    def test_invalid_extracted_tool_never_publishes(self):
        with patch.object(fixtures.urllib.request, "urlretrieve", side_effect=self.download), \
                patch.object(fixtures, "DSYMUTIL_SHA256", "0" * 64):
            with self.assertRaisesRegex(ValueError, "dsymutil checksum mismatch"):
                fixtures.provision_dsymutil(self.root, self.env)
        self.assertEqual(list((self.root / "tools/obs-darwin-dsymutil").iterdir()), [])

    def test_concurrent_publishers_return_same_complete_executable(self):
        ready = threading.Barrier(2)

        def download(url, destination):
            ready.wait(3)
            self.download(url, destination)

        with patch.object(fixtures.urllib.request, "urlretrieve", side_effect=download), \
                ThreadPoolExecutor(max_workers=2) as pool:
            tools = list(pool.map(lambda name: fixtures.provision_dsymutil(self.root / name, self.env), ("a", "b")))
        self.assertEqual(tools[0], tools[1])
        self.assertEqual(tools[0].read_bytes(), self.contents)

    def test_cache_can_be_published_during_initial_probe(self):
        cache = self.root / "tools/obs-darwin-dsymutil" / self.digest
        tool = cache / "package/bin/dsymutil"
        exists = Path.exists

        def published(path):
            if path == cache:
                tool.parent.mkdir(parents=True)
                tool.write_bytes(self.contents)
                return True
            return exists(path)

        with patch.object(Path, "exists", published), \
                patch.object(fixtures.urllib.request, "urlretrieve") as download:
            self.assertEqual(fixtures.provision_dsymutil(self.root, self.env), tool)
            download.assert_not_called()

    def test_incomplete_shared_cache_fails_without_downloading(self):
        cache = self.root / "tools/obs-darwin-dsymutil" / self.digest
        cache.mkdir(parents=True)
        with patch.object(fixtures.urllib.request, "urlretrieve") as download:
            with self.assertRaisesRegex(ValueError, "incomplete dsymutil toolchain cache"):
                fixtures.provision_dsymutil(self.root, self.env)
            download.assert_not_called()

    def test_existing_managed_tool_still_requires_checksum_and_avoids_download(self):
        managed = self.root / ".soldr/bin/syslib/llvm-tools/20.1.7/linux-x64-gnu/package/bin/dsymutil"
        managed.parent.mkdir(parents=True)
        managed.write_bytes(self.contents)
        with patch.object(fixtures.urllib.request, "urlretrieve") as download:
            self.assertEqual(fixtures.provision_dsymutil(self.root, self.env), managed)
            managed.write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "dsymutil checksum mismatch"):
                fixtures.provision_dsymutil(self.root, self.env)
            download.assert_not_called()


class FixtureInventoryTest(unittest.TestCase):
    def swift_archive(self, contents=b"pinned swift compiler"):
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode="w:gz") as archive:
            member = tarfile.TarInfo(f"{fixtures.SWIFT_RELEASE}/usr/bin/swiftc")
            member.size = len(contents)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(contents))
        return output.getvalue()

    def test_swift_install_is_shared_across_checkouts_and_keyed_by_checksum(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = self.swift_archive()
            digest = hashlib.sha256(payload).hexdigest()
            environment = {"RUNNER_TOOL_CACHE": str(root / "tools")}
            with patch.object(fixtures.platform, "system", return_value="Linux"), \
                    patch.object(fixtures.platform, "machine", return_value="x86_64"), \
                    patch.object(fixtures, "SWIFT_SHA256", digest), \
                    patch.object(fixtures.urllib.request, "urlretrieve",
                                 side_effect=lambda url, path: Path(path).write_bytes(payload)) as download:
                first = fixtures.swift_compiler(root / "checkout-a", environment)
                second = fixtures.swift_compiler(root / "checkout-b", environment)
                self.assertEqual(first, second)
                self.assertTrue(first.is_relative_to(root / "tools"))
                self.assertIn(digest, first.parts)
                self.assertEqual(first.read_bytes(), b"pinned swift compiler")
                self.assertEqual(first.stat().st_mode & 0o111, 0o111)
                download.assert_called_once()
                changed = self.swift_archive(b"updated swift compiler")
                with patch.object(fixtures, "SWIFT_SHA256", hashlib.sha256(changed).hexdigest()), \
                        patch.object(fixtures.urllib.request, "urlretrieve",
                                     side_effect=lambda url, path: Path(path).write_bytes(changed)):
                    third = fixtures.swift_compiler(root / "checkout-c", environment)
                self.assertNotEqual(first, third)
                self.assertEqual(third.read_bytes(), b"updated swift compiler")

    def test_swift_failed_extraction_cannot_publish_or_reuse_a_partial_install(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = self.swift_archive()
            environment = {"RUNNER_TOOL_CACHE": str(root / "tools")}
            original = fixtures.tarfile.TarFile.extractall

            def interrupted(archive, destination, **kwargs):
                original(archive, destination, **kwargs)
                raise OSError("interrupted after compiler extraction")

            with patch.object(fixtures.platform, "system", return_value="Linux"), \
                    patch.object(fixtures.platform, "machine", return_value="x86_64"), \
                    patch.object(fixtures, "SWIFT_SHA256", hashlib.sha256(payload).hexdigest()), \
                    patch.object(fixtures.urllib.request, "urlretrieve",
                                 side_effect=lambda url, path: Path(path).write_bytes(payload)) as download:
                with patch.object(fixtures.tarfile.TarFile, "extractall", interrupted):
                    with self.assertRaisesRegex(OSError, "interrupted"):
                        fixtures.swift_compiler(root, environment)
                compiler = fixtures.swift_compiler(root, environment)
                self.assertTrue(compiler.is_relative_to(root / "tools"))
                self.assertEqual(download.call_count, 2, "partial compiler must never count as a cache hit")

    def test_swift_corrupt_download_fails_before_extracting(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(fixtures.platform, "system", return_value="Linux"), \
                    patch.object(fixtures.platform, "machine", return_value="x86_64"), \
                    patch.object(fixtures.urllib.request, "urlretrieve",
                                 side_effect=lambda url, path: Path(path).write_bytes(b"corrupt")), \
                    patch.object(fixtures.tarfile, "open") as extract:
                with self.assertRaisesRegex(ValueError, "Swift toolchain checksum mismatch"):
                    fixtures.swift_compiler(root, {"RUNNER_TOOL_CACHE": str(root / "tools")})
                extract.assert_not_called()

    def test_concurrent_swift_installs_publish_one_complete_toolchain(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payload = self.swift_archive()
            digest = hashlib.sha256(payload).hexdigest()
            ready = threading.Barrier(2)

            def download(url, path):
                ready.wait(3)
                Path(path).write_bytes(payload)

            environment = {"RUNNER_TOOL_CACHE": str(root / "tools")}
            with patch.object(fixtures.platform, "system", return_value="Linux"), \
                    patch.object(fixtures.platform, "machine", return_value="x86_64"), \
                    patch.object(fixtures, "SWIFT_SHA256", digest), \
                    patch.object(fixtures.urllib.request, "urlretrieve", side_effect=download), \
                    ThreadPoolExecutor(max_workers=2) as pool:
                results = list(pool.map(lambda name: fixtures.swift_compiler(root / name, environment), ("a", "b")))
            self.assertEqual(results[0], results[1])
            self.assertEqual(results[0].read_bytes(), b"pinned swift compiler")
            self.assertEqual([path.name for path in results[0].parents[4].iterdir()], [digest])

    def test_swift_cache_can_appear_during_the_initial_probe(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            cache = root / "tools/obs-darwin-swift" / fixtures.SWIFT_SHA256
            compiler = cache / fixtures.SWIFT_RELEASE / "usr/bin/swiftc"
            exists = Path.exists

            def published(path):
                if path == cache:
                    compiler.parent.mkdir(parents=True)
                    compiler.write_bytes(b"compiler published by another producer")
                    return True
                return exists(path)

            with patch.object(fixtures.platform, "system", return_value="Linux"), \
                    patch.object(fixtures.platform, "machine", return_value="x86_64"), \
                    patch.object(Path, "exists", published), \
                    patch.object(fixtures.urllib.request, "urlretrieve") as download:
                self.assertEqual(fixtures.swift_compiler(root, {"RUNNER_TOOL_CACHE": str(root / "tools")}), compiler)
                download.assert_not_called()

    def test_swift_tools_provision_dsymutil_without_ambient_host_tool(self):
        import macos_native_fixtures as fixtures
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            llvm = root / "llvm"
            llvm.mkdir()
            (llvm / "ld64.lld").write_text("linker")
            dsymutil = root / "managed-dsymutil"
            dsymutil.write_text("managed debug tool")
            with patch.object(fixtures.shutil, "which", return_value=None), \
                    patch.object(fixtures, "provision_dsymutil", return_value=dsymutil) as provision:
                tools = fixtures.swift_tools(root, llvm, {})
            self.assertEqual((tools / "dsymutil").resolve(), dsymutil)
            self.assertEqual((tools / "ld64.lld").resolve(), llvm / "ld64.lld")
            provision.assert_called_once_with(root, {})

    def test_missing_dsymutil_cannot_silently_defer_failure_to_swift(self):
        import macos_native_fixtures as fixtures
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(fixtures.shutil, "which", return_value=None), \
                    patch.object(fixtures, "provision_dsymutil", side_effect=FileNotFoundError("dsymutil")):
                with self.assertRaisesRegex(FileNotFoundError, "dsymutil"):
                    fixtures.swift_tools(root, root, {})

    def test_dsymutil_rejects_corrupt_download_before_extracting(self):
        import macos_native_fixtures as fixtures
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(fixtures.platform, "system", return_value="Linux"), \
                    patch.object(fixtures.platform, "machine", return_value="x86_64"), \
                    patch.object(Path, "home", return_value=root), \
                    patch.object(fixtures.urllib.request, "urlretrieve",
                                 side_effect=lambda url, path: Path(path).write_bytes(b"corrupt archive")), \
                    patch.object(fixtures.tarfile, "open") as extract:
                with self.assertRaisesRegex(ValueError, "archive checksum mismatch"):
                    fixtures.provision_dsymutil(root, {})
                extract.assert_not_called()

    def test_preparation_rejects_macos_before_starting_any_tool(self):
        import macos_native_fixtures as fixtures
        with patch.object(fixtures.platform, "system", return_value="Darwin"), patch.object(fixtures.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "Linux producer"):
                fixtures.prepare(Path("unused"), Path("unused"), {})
            run.assert_not_called()

    def test_cross_tool_settings_replace_both_target_spellings(self):
        import macos_native_fixtures as fixtures
        for name in ("CC", "CXX", "AR", "SDKROOT", "SWIFTC"):
            environment = {f"{name}_aarch64-apple-darwin": "inherited-hyphen",
                           f"{name}_aarch64_apple_darwin": "inherited-underscore"}
            fixtures.target_setting(environment, name, "managed")
            self.assertEqual(set(environment.values()), {"managed"})
        environment = {"SWIFTC_aarch64-apple-darwin": "/hyphen/swiftc",
                       "SWIFTC_aarch64_apple_darwin": "/underscore/swiftc"}
        self.assertEqual(fixtures.swift_compiler(Path("unused"), environment), Path("/hyphen/swiftc"))

    def test_every_default_darwin_compile_contract_uses_transport(self):
        import re
        from macos_native_fixtures import TESTS
        root = Path(__file__).resolve().parents[1]
        source = (root / "rust/obs-build/tests/native.rs").read_text()
        definitions = list(re.finditer(r"(?m)^fn (\w+)\(", source))
        blocks = {match[1]: source[match.start():definitions[index + 1].start() if index + 1 < len(definitions) else len(source)]
                  for index, match in enumerate(definitions)}
        for name in TESTS:
            if "::" in name:
                continue
            with self.subTest(name=name):
                block = blocks[name]
                self.assertIn("prebuilt::Scope::enter()", block)
                self.assertNotRegex(block, r"\.(shared_library|swift_shared_library|static_library|executable|compile)\(")
                self.assertNotIn("target: obs_build::BUILD_TARGET.into()", block)

    def test_build_bundles_fixtures_before_final_checksums(self):
        import inspect
        source = inspect.getsource(artifacts.build)
        self.assertLess(source.index("prepare(ROOT, output, env)"), source.index("finalize(output, manifest)"))
        self.assertIn('entry["env"]["OBS_NATIVE_FIXTURES"]', source)


if __name__ == "__main__":
    unittest.main()
