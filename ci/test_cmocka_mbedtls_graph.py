#!/usr/bin/env -S uv run --script
"""Native tests built with USE_MBEDTLS list MbedTLS::mbedtls in the test graph.

    uv run python -m unittest ci/test_cmocka_mbedtls_graph.py

USE_MBEDTLS with CRYPTO makes plugins/obs-outputs/librtmp/rtmp.h include
<mbedtls/version.h>. A test that defines them but does not list
MbedTLS::mbedtls gets no mbedtls include directory, in CMake or in the Cargo
native test build. Where mbedtls 3 headers are on the default include path
(Ubuntu CI) it still compiles, so only this check sees the gap there; where they
are not (e.g. /usr/include/mbedtls3 on Arch Linux) the build fails.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GRAPH = ROOT / "test" / "cmocka" / "build" / "test-graph.json"
LIBRARY = "MbedTLS::mbedtls"


def missing_mbedtls(graph: dict) -> list[str]:
    """Targets that define USE_MBEDTLS but do not list MbedTLS::mbedtls."""
    found = []
    for name, target in sorted(graph.get("targets", {}).items()):
        definitions = {d.split("=", 1)[0] for d in target.get("definitions", [])}
        if "USE_MBEDTLS" not in definitions:
            continue
        libraries = {lib if isinstance(lib, str) else lib.get("name") for lib in target.get("libraries", [])}
        if LIBRARY not in libraries:
            found.append(name)
    return found


class CmockaMbedtlsGraphTest(unittest.TestCase):
    def test_use_mbedtls_targets_link_mbedtls(self):
        self.assertEqual(missing_mbedtls(json.loads(GRAPH.read_text())), [])

    def test_checker_sees_a_missing_library(self):
        graph = {
            "targets": {
                "a": {"definitions": ["USE_MBEDTLS", "CRYPTO"], "libraries": ["OBS::libobs"]},
                "b": {"definitions": ["USE_MBEDTLS"], "libraries": [LIBRARY]},
                "c": {"definitions": ["USE_MBEDTLS"], "libraries": [{"name": LIBRARY, "requires": "OS_LINUX"}]},
                "d": {"definitions": ["NO_CRYPTO"], "libraries": ["OBS::libobs"]},
            }
        }
        self.assertEqual(missing_mbedtls(graph), ["a"])


if __name__ == "__main__":
    unittest.main()
