#!/usr/bin/env -S uv run --script
"""Check native graph snapshots; regenerate explicitly with --fix.

    uv run ci/native_expectations.py --fix

Only these two mechanical graph slices are regenerated. Their case sets remain
fixed; missing targets/tests are errors, and ordered arrays are never sorted.
The Rust registration tests retain their independent host/count assertions.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIRECTORY = "rust/obs-build/tests/native/registration/"
CASES = {
    DIRECTORY + "headless_characterizations_expected.json": (
        "test_wayland_hotkeys", "test_service_state", "test_lua_graphics_lock",
        "test_lua_graphics_lock_bounded", "test_vcam_audio_config", "test_virtual_cam_shutdown",
    ),
    DIRECTORY + "portal_output_expected.json": (
        "test_encoder_counter_getters", "test_config_close_fault", "test_inhibit_portal",
        "test_inhibit_sleep_dbus", "test_portal_shortcuts", "test_output_stop_while_reconnecting",
    ),
}


def rendered(root: Path, paths: list[str]) -> dict[str, str]:
    """Validate every selected slice before writing any file."""
    graph_path = root / "test/cmocka/build/test-graph.json"
    graph = json.loads(graph_path.read_text(encoding="utf-8"))
    outputs = {}
    for path in paths:
        names = CASES[path]
        value = {"schema": graph["schema"]}
        for section in ("targets", "tests"):
            value[section] = {name: graph[section][name] for name in names}
        outputs[path] = json.dumps(value, indent=4 if "headless_characterizations" in path else 2, ensure_ascii=False) + "\n"
    return outputs


def regenerate(root: Path, paths: list[str]) -> None:
    for path, text in rendered(root, paths).items():
        (root / path).write_text(text, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fix", action="store_true")
    args = parser.parse_args()
    outputs = rendered(ROOT, list(CASES))
    stale = [path for path, text in outputs.items()
             if (ROOT / path).read_text(encoding="utf-8") != text]
    if args.fix:
        for path in stale:
            (ROOT / path).write_text(outputs[path], encoding="utf-8")
        return 0
    if stale:
        print("Regenerate with `uv run ci/native_expectations.py --fix`: " + ", ".join(stale))
    return bool(stale)


if __name__ == "__main__":
    raise SystemExit(main())
