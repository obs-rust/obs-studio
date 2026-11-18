#!/usr/bin/env -S uv run --script
"""The cross check fetches its host Qt as an authenticated client (#4209).

On 2026-11-09 GitHub answered anonymous release downloads with 504 for hours
while authenticated requests worked, and the Cross Compile job failed on
`file(DOWNLOAD)` of the host Qt for uic. With a token in GITHUB_TOKEN (or
GH_TOKEN), cmake/cross/release-download.cmake asks the REST API for the
asset; without one (a local run) it keeps the anonymous URL. The hash check
stays either way, and the token never goes into a URL or a message.

These cases run the helper under `cmake -P` against a local HTTP server that
plays both github.com (anonymous downloads down) and the API.

    uv run --no-project -m unittest ci/test_cross_release_download.py
"""

from __future__ import annotations

import hashlib
import http.server
import json
import os
import shutil
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "cmake/cross/release-download.cmake"
CMAKE = shutil.which("cmake")
TOKEN = "ghs_cross-test-token"
PAYLOAD = b"host qt archive bytes"
SHA256 = hashlib.sha256(PAYLOAD).hexdigest()
REPO, TAG, ASSET = "obs-rust/obs-studio-libs", "qt6-6.11.1", "qt6-host.tar.zst"


class Server(http.server.ThreadingHTTPServer):
    def __init__(self, anonymous_status: int):
        super().__init__(("127.0.0.1", 0), Handler)
        self.anonymous_status = anonymous_status
        self.seen: list[tuple[str, str, dict[str, str]]] = []


class Handler(http.server.BaseHTTPRequestHandler):
    server: Server

    def log_message(self, *args):  # keep test output quiet
        pass

    def reply(self, status: int, body: bytes = b"", headers: dict[str, str] | None = None):
        self.send_response(status)
        for key, value in (headers or {}).items():
            self.send_header(key, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        host = self.headers.get("Host", "")
        self.server.seen.append((host, self.path, {k.lower(): v for k, v in self.headers.items()}))
        port = self.server.server_address[1]
        if self.path == f"/{REPO}/releases/download/{TAG}/{ASSET}":
            if self.server.anonymous_status != 200:
                return self.reply(self.server.anonymous_status)
            return self.reply(200, PAYLOAD)
        if self.path == f"/repos/{REPO}/releases/tags/{TAG}":
            assets = [{"name": "other.tar.zst", "url": f"http://127.0.0.1:{port}/repos/{REPO}/releases/assets/1"},
                      {"name": ASSET, "url": f"http://127.0.0.1:{port}/repos/{REPO}/releases/assets/4209"}]
            return self.reply(200, json.dumps({"tag_name": TAG, "assets": assets}).encode())
        if self.path == f"/repos/{REPO}/releases/assets/4209":
            # The API redirects to a signed URL on another host.
            return self.reply(302, headers={"Location": f"http://localhost:{port}/signed/{ASSET}?sig=x"})
        if self.path == f"/signed/{ASSET}?sig=x":
            return self.reply(200, PAYLOAD)
        return self.reply(404)


@unittest.skipUnless(CMAKE, "cmake is not installed")
class CrossReleaseDownloadTest(unittest.TestCase):
    def run_helper(self, env_token: dict[str, str], anonymous_status: int, sha256: str = SHA256):
        server = Server(anonymous_status)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        base = f"http://127.0.0.1:{server.server_address[1]}"
        tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, tmp)
        out = tmp / ASSET
        script = tmp / "drive.cmake"
        script.write_text(
            f'set(OBS_CROSS_GITHUB_WEB "{base}")\n'
            f'set(OBS_CROSS_GITHUB_API "{base}")\n'
            f'include("{HELPER.as_posix()}")\n'
            f'obs_cross_release_download("host Qt for uic" {REPO} {TAG} {ASSET} "{out.as_posix()}" {sha256})\n',
            encoding="utf-8")
        env = {k: v for k, v in os.environ.items() if k not in ("GITHUB_TOKEN", "GH_TOKEN")}
        env.update(env_token)
        env.update({"no_proxy": "*", "NO_PROXY": "*"})
        result = subprocess.run([CMAKE, "-P", str(script)], env=env, capture_output=True, text=True,
                                timeout=60, check=False)
        return result, out, server.seen

    def test_token_downloads_through_the_api_when_anonymous_downloads_fail(self):
        result, out, seen = self.run_helper({"GITHUB_TOKEN": TOKEN}, anonymous_status=504)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(out.read_bytes(), PAYLOAD)
        paths = [path for _, path, _ in seen]
        self.assertNotIn(f"/{REPO}/releases/download/{TAG}/{ASSET}", paths)
        self.assertEqual(paths, [f"/repos/{REPO}/releases/tags/{TAG}", f"/repos/{REPO}/releases/assets/4209",
                                 f"/signed/{ASSET}?sig=x"])
        tag, asset, signed = (headers for _, _, headers in seen)
        self.assertEqual(tag.get("authorization"), f"Bearer {TOKEN}")
        self.assertEqual(asset.get("authorization"), f"Bearer {TOKEN}")
        self.assertEqual(asset.get("accept"), "application/octet-stream")
        # The token stays with the API host; the signed URL needs none.
        self.assertNotIn("authorization", signed)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_gh_token_is_used_when_github_token_is_absent(self):
        result, out, seen = self.run_helper({"GH_TOKEN": "gh-cli-token"}, anonymous_status=504)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(seen[0][2].get("authorization"), "Bearer gh-cli-token")

    def test_without_a_token_the_anonymous_url_is_kept(self):
        result, out, seen = self.run_helper({}, anonymous_status=200)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(out.read_bytes(), PAYLOAD)
        self.assertEqual([path for _, path, _ in seen], [f"/{REPO}/releases/download/{TAG}/{ASSET}"])
        self.assertNotIn("authorization", seen[0][2])

    def test_wrong_bytes_fail_with_the_hash_and_no_token(self):
        result, out, _ = self.run_helper({"GITHUB_TOKEN": TOKEN}, anonymous_status=504, sha256="0" * 64)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("HASH mismatch", result.stderr)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    def test_cross_lists_fetch_the_host_qt_through_the_helper(self):
        lists = (ROOT / "cmake/cross/CMakeLists.txt").read_text(encoding="utf-8")
        self.assertTrue("include(${CMAKE_CURRENT_LIST_DIR}/release-download.cmake)" in lists,
                        "cmake/cross/CMakeLists.txt does not include release-download.cmake")
        self.assertTrue("obs_cross_release_download(" in lists,
                        "the host Qt is not fetched with obs_cross_release_download()")
        self.assertFalse("releases/download/${OBS_CROSS_HOST_QT_TAG}" in lists,
                         "the host Qt is still fetched from the anonymous URL")


if __name__ == "__main__":
    unittest.main()
