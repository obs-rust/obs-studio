#!/usr/bin/env -S uv run --script
"""Unit tests for the Cargo feature sets of ci/local_gate.py (#2056).

    uv run python -m unittest ci/test_feature_sets.py
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import unittest
from pathlib import Path
from unittest import mock

_SPEC = importlib.util.spec_from_file_location("local_gate", Path(__file__).with_name("local_gate.py"))
local_gate = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(local_gate)


def crate(features=None, deps=None, dev=None, build=None):
    manifest = {"package": {"name": "x"}}
    if features is not None:
        manifest["features"] = features
    for kind, table in (("dependencies", deps), ("dev-dependencies", dev), ("build-dependencies", build)):
        if table:
            manifest[kind] = table
    return manifest


class FeatureCoverageTest(unittest.TestCase):
    """Every workspace feature is built on and off by some clippy run: the
    --workspace run (features unified) or one of FEATURE_SETS."""

    def coverage(self, manifests, sets=(), exempt=None):
        return local_gate.feature_coverage(manifests, tuple(sets), exempt or {})

    def test_default_feature_needs_a_run_without_it(self):
        # libobs-rust: default = ["gio"]; CMake builds it with
        # NO_DEFAULT_FEATURES when Gio is missing.
        manifests = {"lib": crate({"default": ["gio"], "gio": []})}
        errors = self.coverage(manifests)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("lib/gio", errors[0])
        self.assertIn("off", errors[0])
        self.assertEqual(self.coverage(manifests, [("-p", "lib", "--no-default-features")]), [])

    def test_opt_in_feature_needs_a_run_with_it(self):
        manifests = {"app": crate({"extra": []})}
        errors = self.coverage(manifests)
        self.assertEqual(len(errors), 1, errors)
        self.assertIn("app/extra", errors[0])
        self.assertIn("on", errors[0])
        self.assertEqual(self.coverage(manifests, [("-p", "app", "--features", "extra")]), [])

    def test_feature_enabled_by_a_dependent_is_on_in_the_workspace_run(self):
        # obs-util/gio is not a default of obs-util, but libobs-rust's default
        # turns it on in every --workspace run; only a run that builds
        # obs-util without libobs-rust's default turns it off.
        manifests = {
            "lib": crate({"default": ["gio"], "gio": ["util/gio"]}, deps={"util": {"path": "../util"}}),
            "util": crate({"gio": []}),
        }
        errors = self.coverage(manifests, [("-p", "lib", "--no-default-features")])
        self.assertEqual(errors, [])
        errors = self.coverage(manifests)
        self.assertEqual(sorted(e.split(":")[0] for e in errors), ["lib/gio", "util/gio"])

    def test_dependency_features_and_defaults_follow_the_dependency_table(self):
        manifests = {
            "app": crate(deps={"core": {"path": "../core", "features": ["fast"], "default-features": False}}),
            "core": crate({"default": ["std"], "std": [], "fast": []}),
        }
        on = local_gate.enabled_features(manifests, "app", (), default=True)
        self.assertIn(("core", "fast"), on)
        self.assertNotIn(("core", "std"), on)

    def test_optional_dependencies_only_with_their_feature(self):
        manifests = {
            "app": crate(
                {"native": ["dep:sys", "sys/bundled"], "probe": ["sys?/probe"]},
                deps={"sys": {"path": "../sys", "optional": True}},
            ),
            "sys": crate({"bundled": [], "probe": []}),
        }
        self.assertEqual(local_gate.active_crates(manifests, "app", (), default=True), {"app"})
        on = local_gate.enabled_features(manifests, "app", ("probe",), default=True)
        self.assertNotIn(("sys", "probe"), on)  # `?/` does not activate the dependency
        on = local_gate.enabled_features(manifests, "app", ("native", "probe"), default=True)
        self.assertIn(("sys", "bundled"), on)
        self.assertIn(("sys", "probe"), on)
        self.assertEqual(local_gate.active_crates(manifests, "app", ("native",), default=True), {"app", "sys"})

    def test_dev_and_build_dependencies_count(self):
        # Every run is --all-targets: dev-dependencies are built too.
        manifests = {
            "app": crate(dev={"helper": {"path": "../helper", "features": ["trace"]}}, build={"gen": {"path": "../gen"}}),
            "helper": crate({"trace": []}),
            "gen": crate(),
        }
        self.assertEqual(local_gate.active_crates(manifests, "app", (), default=True), {"app", "helper", "gen"})
        self.assertIn(("helper", "trace"), local_gate.enabled_features(manifests, "app", (), default=True))

    def test_off_only_counts_where_the_crate_is_built(self):
        # A run that does not build `lib` at all does not turn lib/gio off.
        manifests = {"lib": crate({"default": ["gio"], "gio": []}), "other": crate()}
        errors = self.coverage(manifests, [("-p", "other",)])
        self.assertEqual(len(errors), 1, errors)

    def test_exemption_needs_a_reason_and_a_real_feature(self):
        manifests = {"app": crate({"bundled": []})}
        self.assertEqual(self.coverage(manifests, exempt={("app", "bundled"): "needs the release staticlib"}), [])
        errors = self.coverage(manifests, exempt={("app", "bundled"): ""})
        self.assertTrue(any("reason" in e for e in errors), errors)
        errors = self.coverage(manifests, exempt={("app", "bundled"): "x", ("app", "gone"): "stale"})
        self.assertTrue(any("app/gone" in e for e in errors), errors)

    def test_unknown_crate_or_feature_in_a_set_is_an_error(self):
        manifests = {"app": crate({"extra": []})}
        errors = self.coverage(manifests, [("-p", "app", "--features", "extra,typo")])
        self.assertTrue(any("typo" in e for e in errors), errors)
        errors = self.coverage(manifests, [("-p", "nope", "--features", "extra")])
        self.assertTrue(any("nope" in e for e in errors), errors)


class WorkspaceFeatureSetsTest(unittest.TestCase):
    """The real workspace: what CMake and the release builds select."""

    def test_every_workspace_feature_is_built_on_and_off(self):
        errors = local_gate.feature_coverage(
            local_gate.workspace_manifests(), local_gate.FEATURE_SETS, local_gate.FEATURE_SET_EXEMPT
        )
        self.assertEqual(errors, [])

    def test_manifests_are_the_workspace_members(self):
        names = set(local_gate.workspace_manifests())
        self.assertIn("libobs-rust", names)
        self.assertIn("obs-util", names)
        self.assertNotIn("tools", names)

    def test_gio_off_is_linted_and_tested(self):
        # CMake: NO_DEFAULT_FEATURES, then gio only if Gio is found
        # (libobs/cmake/os-linux.cmake, os-freebsd.cmake).
        self.assertIn(("-p", "libobs-rust", "--no-default-features"), local_gate.FEATURE_SETS)
        self.assertIn(("-p", "obs-util"), local_gate.FEATURE_TEST_SETS)


class FeatureSetLanesTest(unittest.TestCase):
    """The lanes run one cargo command per set, through soldr_cargo."""

    def cargo_calls(self, lane):
        calls = []

        def fake_run(cmd, **kw):
            calls.append(cmd)
            return subprocess.CompletedProcess(cmd, 0)

        with (
            mock.patch.dict(os.environ, {}, clear=False),
            mock.patch.object(local_gate.subprocess, "run", side_effect=fake_run),
            mock.patch.object(local_gate, "run", side_effect=fake_run),
            mock.patch.object(local_gate, "tracked_files", return_value=[]),
            mock.patch.object(local_gate, "ensure_nextest", return_value=None),
            mock.patch.object(local_gate, "clang_format"),
            mock.patch.object(local_gate, "gersemi"),
            mock.patch.object(local_gate, "self_test"),
            mock.patch.object(local_gate, "check_test_binary_inventory"),
            mock.patch.object(local_gate, "check_feature_sets") as check,
            mock.patch.object(local_gate, "toolchain_gcc_ld", return_value="/sysroot/gcc-ld"),
        ):
            lane(False)
        return calls, check

    def test_lint_checks_coverage_and_lints_each_set(self):
        calls, check = self.cargo_calls(local_gate.lane_lint)
        check.assert_called_once_with()
        self.assertTrue(local_gate.FEATURE_SETS)
        for args in local_gate.FEATURE_SETS:
            with self.subTest(args=args):
                self.assertIn(["soldr", "cargo", "clippy", *args, "--all-targets", "--locked", "--", "-D", "warnings"], calls)

    def test_rust_test_tests_each_set(self):
        calls, _ = self.cargo_calls(local_gate.lane_rust_test)
        self.assertTrue(local_gate.FEATURE_TEST_SETS)
        for args in local_gate.FEATURE_TEST_SETS:
            with self.subTest(args=args):
                self.assertIn(["soldr", "cargo", "test", *args, "--locked"], calls)


if __name__ == "__main__":
    unittest.main()
