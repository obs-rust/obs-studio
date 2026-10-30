"""Cook cache namespaces must reflect the measured consumer policy (#1286)."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def jobs(path):
    text = path.read_text().split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([a-z0-9-]+):\s*$", text, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


class CookProfilesTest(unittest.TestCase):
    def test_linux_runner_layouts_have_distinct_cook_namespaces(self):
        workflow = jobs(ROOT / ".github/workflows/build-project.yaml")

        def namespace(name):
            match = re.search(r"^\s+cache-key-suffix: (\S+)$", workflow[name], re.M)
            self.assertIsNotNone(match, name)
            return match[1]

        host = {namespace(name) for name in ("rust-tests-linux", "rust-cross-lint", "rust-cross-lint-release")}
        self.assertEqual(len(host), 1, "Compatible host jobs must share the same namespace")
        self.assertNotIn(namespace("rust-linux-tier2-build"), host,
                         "Different archive-path versions must not share janitor identity")
        for name in ("rust-tests-macos", "rust-tests-windows", "rust-tier2-macos", "rust-tier2-windows"):
            self.assertNotIn("cache-key-suffix:", workflow[name],
                             "Do not invalidate unrelated Windows/macOS caches")

    def test_build_consumers_declare_the_profile_policy(self):
        expected = {
            "rust-tests-linux": "${{ needs.check-event.outputs.core == 'true' && '--release' || '--profile dev -- --tests' }}",
            "rust-cross-lint": "--profile dev",
            "rust-cross-lint-release": "--profile dev",
            "rust-tests-macos": "--profile dev -- --tests",
            "rust-tests-windows": "--profile dev -- --tests",
            # Corrosion-only build, its own target dir: no cook (#3027).
            "rust-linux-tier2-build": None,
            # validate.sh/.ps1 build with --target <triple> --target-dir
            # build/cargo-native-* and Corrosion: no cook (#3034).
            "rust-tier2-macos": None,
            "rust-tier2-windows": None,
        }
        workflow = jobs(ROOT / ".github/workflows/build-project.yaml")
        for name, flags in expected.items():
            with self.subTest(job=name):
                if flags is None:
                    self.assertIn("prebuild-deps: none", workflow[name])
                    self.assertNotIn("prebuild-deps-flags", workflow[name])
                    continue
                self.assertIn("prebuild-deps-flags: " + flags, workflow[name])
                self.assertNotIn("prebuild-deps: none", workflow[name])

    def test_release_static_library_uses_release_cook(self):
        found = 0
        for name, job in jobs(ROOT / ".github/workflows/release-rust.yaml").items():
            if "zackees/setup-soldr@" in job:
                found += 1
                self.assertIn("prebuild-deps-flags: --release", job, name)
        self.assertEqual(found, 1)

    def test_release_linux_host_shares_host_namespace_only_on_linux(self):
        release = jobs(ROOT / ".github/workflows/release-rust.yaml")["build"]
        match = re.search(r"^\s+cache-key-suffix: (.+)$", release, re.M)
        self.assertIsNotNone(match, "The release Linux host must share the host archive identity")
        expression = re.fullmatch(
            r"\$\{\{ matrix.os == '([^']+)' && '([^']+)' \|\| '([^']*)' \}\}", match[1])
        self.assertIsNotNone(expression)
        host = jobs(ROOT / ".github/workflows/build-project.yaml")["rust-tests-linux"]
        host_suffix = re.search(r"^\s+cache-key-suffix: (\S+)$", host, re.M)[1]
        for platform, expected in [("linux", host_suffix), ("macos", ""), ("windows", "")]:
            with self.subTest(platform=platform):
                actual = expression[2] if platform == expression[1] else expression[3]
                self.assertEqual(actual, expected)

    def test_cook_dev_dependencies_declare_explicit_features(self):
        """Re-enabled defaults use explicit features, not `default-features` (#2661).

        The cook with dev-dependencies (`-- --tests`) drops a member's
        `default-features = true` override for a `[workspace.dependencies]`
        entry declared with `default-features = false` (the #2603/#2604
        defect), so the recipe either fails or serves the wrong feature set.
        The fix is the #2604 pattern: an explicit list equal to the crate's
        own `default`.
        """
        crate_defaults = {
            "proptest": {"std", "fork", "timeout", "bit-set"},
            "base64": {"std", "simd-unsafe"},
            "memchr": {"std"},
            "sha2": {"alloc", "oid"},
        }
        seen: set[str] = set()
        for manifest in sorted((ROOT / "rust").glob("*/Cargo.toml")):
            for line_no, line in enumerate(manifest.read_text().splitlines(), 1):
                crate = line.split(" = ", 1)[0]
                if crate not in crate_defaults:
                    continue
                seen.add(crate)
                match = re.search(r"features = \[([^\]]*)\]", line)
                declared = {feature.strip().strip('"') for feature in match[1].split(",")} if match else set()
                with self.subTest(manifest=manifest.name, line=line_no, crate=crate):
                    self.assertNotIn("default-features = true", line)
                    self.assertEqual(declared, crate_defaults[crate])
        self.assertEqual(seen, set(crate_defaults), "Every re-enabled dependency must be covered")


if __name__ == "__main__":
    unittest.main()
