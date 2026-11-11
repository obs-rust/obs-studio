#!/usr/bin/env -S uv run --script
"""Exercise the COFF import-thunk/static-object collision from #3603 without an SDK.

This is a link regression, not a substitute for native Windows Tier 2.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from native_test_graph import load_graph

ROOT = Path(__file__).resolve().parent.parent
REGISTRATION = ROOT / "test/cmocka/build/test-graph.d"
BRIDGE = ROOT / "test/cmocka/hosts/windows/effect-parser-lookup-import.c"
SYMBOL = "gs_effect_get_param_by_name"
ALIAS = "obs_test_effect_get_param_by_name"


class EffectParserWindowsLinkTest(unittest.TestCase):
    def test_public_import_and_private_static_shim_link_together(self):
        programs = {name: shutil.which(name) for name in ("clang", "lld-link", "llvm-lib", "llvm-readobj")}
        if not all(programs.values()):
            self.skipTest("COFF tools unavailable; native Windows Tier 2 remains required")
        with tempfile.TemporaryDirectory(prefix="obs-effect-coff-") as directory:
            root = Path(directory)

            def run(name, *args):
                result = subprocess.run([programs[name], *map(str, args)], cwd=root,
                                        capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                return result

            def compile_c(name, source, *args):
                (root / f"{name}.c").write_text(source)
                run("clang", "--target=x86_64-pc-windows-msvc", "-c", f"{name}.c",
                    "-o", f"{name}.obj", *args)

            # The public declaration is deliberately not dllimport: this is
            # libobs's real EXPORT convention. Only pointer types are needed.
            header = ("typedef struct gs_effect gs_effect_t;\n"
                      "typedef struct gs_effect_param gs_eparam_t;\n"
                      f"__declspec(dllexport) gs_eparam_t *{SYMBOL}(const gs_effect_t *, const char *);\n")
            (root / "graphics").mkdir()
            (root / "graphics/graphics.h").write_text(header)
            implementation = f"gs_eparam_t *{SYMBOL}(const gs_effect_t *e, const char *n) {{ return (gs_eparam_t *)e; }}\n"
            compile_c("dll", header + implementation)
            run("lld-link", "/dll", "/noentry", "/nodefaultlib", "/out:obs.dll",
                "/implib:obs.lib", "dll.obj")
            # One Rust codegen object has both the private shim and public
            # lookup. Modelling it in C keeps the reproduction SDK/toolchain free.
            compile_c("rust", header + implementation + "void ep_parse_struct(void) {}\n")
            run("llvm-lib", "/out:libobs_rust.lib", "rust.obj")
            registrations = load_graph(REGISTRATION)["targets"]
            bridge_source = {
                "path": "test/cmocka/hosts/windows/effect-parser-lookup-import.c",
                "requires": "MSVC",
            }
            bridge_define = {"name": f"/D{SYMBOL}={ALIAS}", "requires": "MSVC"}
            for suffix, output in (
                ("blocks", "effect-parser-lookup-import.lib"),
                ("params", "effect-parser-params-lookup-import.lib"),
                ("passes", "effect-parser-passes-lookup-import.lib"),
                ("shader_call", "effect-parser-shader-call-lookup-import.lib"),
            ):
                target = registrations[f"test_effect_parser_{suffix}_rust"]
                with self.subTest(fixture=suffix):
                    self.assertIn(bridge_source, target["sources"])
                    self.assertIn(bridge_define, target["compile_options"])
                    self.assertEqual(target["msvc_import_library"], {
                        "definition": "test/cmocka/hosts/windows/effect-parser-lookup-import.def",
                        "output": output,
                    })
                    self.assertEqual(target["libraries"][:5], [
                        "OBS::libobs", "OBS::caption", "Uthash::Uthash",
                        "OBS::native-test-cmocka", "libobs_rust-static",
                    ])
            registration = registrations["test_effect_parser_blocks_rust"]
            bridged = bridge_define in registration["compile_options"]
            extra_objects = []
            defines = []
            if bridged:
                self.assertIn(bridge_source, registration["sources"])
                defines = [f"-D{SYMBOL}={ALIAS}"]
                run("clang", "--target=x86_64-pc-windows-msvc", "-c", BRIDGE,
                    "-I", root, *defines, "-o", "bridge.obj")
                extra_objects = ["bridge.obj"]
                definition = ROOT / registration["msvc_import_library"]["definition"]
                run("llvm-lib", f"/def:{definition}",
                    "/machine:x64", "/out:lookup-import.lib")
            compile_c("parser", header + "void ep_parse_struct(void);\n"
                      f"void mainCRTStartup(void) {{ {SYMBOL}(0, 0); ep_parse_struct(); }}\n", *defines)
            # obs.lib first, exactly as in the failing Tier 2 target. No /FORCE.
            run("lld-link", "/entry:mainCRTStartup", "/subsystem:console", "/nodefaultlib",
                "/out:test.exe", "parser.obj", *extra_objects,
                *(["lookup-import.lib"] if bridged else []), "obs.lib", "libobs_rust.lib")
            # LLD can prefer the static definition where link.exe reports
            # LNK2005. Verify ownership too: the public call must still go
            # through obs.dll, even with a static definition in the link.
            imports = run("llvm-readobj", "--coff-imports", "test.exe").stdout
            self.assertIn("Name: obs.dll", imports)
            self.assertIn(f"Symbol: {SYMBOL} ", imports)


if __name__ == "__main__":
    unittest.main()
