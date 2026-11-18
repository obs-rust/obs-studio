"""Real archive/PE/source-receipt tests; target execution is mocked on Linux."""
import hashlib
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_test_artifact as shared
import macos_test_workspace as archive
import windows_native_fixtures as native
import windows_qt_artifact as qt
import windows_test_proof as proof
from test_windows_qt_artifact import dll
from test_windows_test_artifact import executable

OWNERS = ("obs-util", "obs-data", "obs-graphics", "obs-media-io", "obs-nvenc",
          "obs-gpu-status", "obs-preflight", "obs-studio-rust")
LOGICAL = "/opt/obs-win-42-3/src"


def write(root, name, contents):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(contents)
    return path


def fixture(root):
    source, bundle = root / "source", root / "bundle"
    source.mkdir()
    (source / "Cargo.toml").write_text("source bytes\n")
    for command in (["git", "init", "-q"], ["git", "add", "Cargo.toml"],
                    ["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"]):
        subprocess.run(command, cwd=source, capture_output=True, check=True)
    tests = []
    for owner in OWNERS:
        name = f"target/{proof.windows.TARGET}/debug/deps/{owner}.exe"
        write(bundle, name, executable())
        tests.append({"package": owner, "binary": name, "cwd": ".", "args": [], "env": {}})
    for name in ("obs-build", "obs-preflight", "obs-studio-rust"):
        write(bundle, f"target/{proof.windows.TARGET}/debug/{name}.exe", executable())
    native_root = bundle / "native-fixtures"
    snapshots = []
    work = LOGICAL + "/target/cross-native-fixture-work/fixture"
    for i in range(55):
        image = dll() + bytes([i])
        digest = hashlib.sha256(image).hexdigest()
        write(native_root, "blobs/" + digest, image)
        snapshots.append({"file": "blobs/" + digest, "sha256": digest, "destination": f"{work}/image{i}.dll"})
    names = ["native_build::" + name for name in native.TESTS]
    for i, name in enumerate(names):
        events = [{"files": snapshots if i == 0 and j == 0 else [],
                   "directories": [work + "/runtime root/Release"]} for j in range(140 if i == 0 else 1)]
        write(native_root, name.replace("::", "--") + "/manifest.json", json.dumps({"version": 1,
            "complete": True, "target": proof.windows.TARGET, "host": "x86_64-unknown-linux-gnu",
            "directories": [work], "events": events}).encode())
    write(native_root, "index.json", json.dumps({"version": 1, "complete": True,
        "target": proof.windows.TARGET, "tests": names, "images": native.fixture_images(native_root)}).encode())
    runtime = bundle / qt.RELATIVE
    write(runtime, "contracts/answer.exe", executable(["Qt6Core.dll"]))
    write(runtime, "contracts/compile-helper.exe", executable())
    write(runtime, "qt/bin/Qt6Core.dll", dll())
    write(runtime, "qt/plugins/platforms/qminimal.dll", dll())
    for contract, count in zip(qt.CONTRACTS, (3, 11, 4, 30), strict=True):
        tests.extend({"package": "Qt/" + contract, "kind": "qt-contract", "binary": qt.RELATIVE + "/contracts/answer.exe",
                      "cwd": ".", "args": [], "env": {},
                      "env_path_prepend": {"PATH": ["{root}/" + qt.RELATIVE + "/qt/bin"]}} for _ in range(count))
    images = {p.relative_to(bundle).as_posix(): proof.windows.pe_imports(p.read_bytes(), staged_dlls={"qt6core.dll"})
              for p in bundle.rglob("*.exe")}
    data = {"version": 1, "target": proof.windows.TARGET, "build_root": LOGICAL,
            "coverage": proof.windows.PREPARED_COVERAGE, "doctests_complete": True, "native_fixtures": True,
            "tests": tests, "images": images, "qt_images": qt.image_inventory(runtime),
            "qt_contracts_complete": list(qt.CONTRACTS), "runtime_dlls": ["vcruntime140.dll"]}
    receipt = {"run_id": "42", "run_attempt": "3", "source_sha": proof.git(source, "rev-parse", "HEAD"),
               "source_tree": proof.git(source, "rev-parse", "HEAD^{tree}"), "sources": proof.source_inventory(source),
               "logical_root": LOGICAL}
    write(bundle, "producer.json", json.dumps(receipt).encode())
    shared.finalize(bundle, data)
    return source, bundle, data, receipt


class PayloadTest(unittest.TestCase):
    def test_complete_source_and_payload_are_verified_before_target_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, bundle, data, receipt = fixture(root)
            proof.verify_payload(bundle, require_qt=True)
            proof.verify_complete(data)
            packet = root / "packet.tar.gz"
            archive.pack(bundle, packet)
            with patch.object(proof, "validate_runtime_root"), patch.object(proof, "qualify_compiled_root"), \
                    patch.object(proof, "run_verified", return_value=7) as run, patch.dict(os.environ):
                self.assertEqual(proof.consume(packet, source, root / "stage", archive.checksum(packet), receipt, execute=True), 7)
                run.assert_called_once_with(source / "manifest.json", source)
                self.assertTrue((source / "target/windows-proof-temp").is_dir())

    def test_changed_source_bytes_fail_before_install_or_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, bundle, _, receipt = fixture(root)
            packet = root / "packet.tar.gz"
            archive.pack(bundle, packet)
            (source / "Cargo.toml").write_text("wrong source\n")
            with patch.object(proof, "run_verified") as run:
                with self.assertRaisesRegex(ValueError, "receipt mismatch: sources"):
                    proof.consume(packet, source, root / "stage", archive.checksum(packet), receipt, execute=True)
                run.assert_not_called()
                self.assertFalse((source / "target").exists())

    def test_missing_category_helper_or_qt_observation_is_incomplete(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, _, data, _ = fixture(Path(temporary))
            for package in ("obs-util", "Qt/" + qt.CONTRACTS[0]):
                changed = json.loads(json.dumps(data))
                changed["tests"].remove(next(e for e in changed["tests"] if e["package"] == package))
                with self.subTest(package=package), self.assertRaisesRegex(ValueError, "categories|Qt"):
                    proof.verify_complete(changed)
            changed = json.loads(json.dumps(data))
            changed["images"].pop(f"target/{proof.windows.TARGET}/debug/obs-build.exe")
            with self.assertRaisesRegex(ValueError, "helpers"):
                proof.verify_complete(changed)

    def test_native_history_count_and_compiled_roots_are_bound_before_execution(self):
        for change in ("event", "root", "snapshot digest", "version", "snapshot image"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                _, bundle, data, _ = fixture(Path(temporary))
                path = bundle / "native-fixtures" / ("native_build::" + native.TESTS[0]).replace("::", "--") / "manifest.json"
                history = json.loads(path.read_text())
                if change == "event":
                    history["events"].pop()
                elif change == "root":
                    history = json.loads(json.dumps(history).replace(LOGICAL, "/opt/obs-win-99-1/src"))
                elif change == "snapshot digest":
                    history["events"][0]["files"][0]["sha256"] = "0" * 64
                elif change == "version":
                    history["version"] = 0
                else:
                    removed = history["events"][0]["files"].pop()["file"]
                    index_path = bundle / "native-fixtures/index.json"
                    index = json.loads(index_path.read_text())
                    index["images"].pop(removed)
                    index_path.write_text(json.dumps(index))
                path.write_text(json.dumps(history))
                shared.finalize(bundle, data)
                with self.assertRaisesRegex(ValueError, "history|compiled root|snapshot"):
                    proof.verify_payload(bundle, require_qt=True)


if __name__ == "__main__":
    unittest.main()
