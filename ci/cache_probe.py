#!/usr/bin/env python3
"""Measure the cache paths this repo actually has, in one job.

Phases (each independent — one failing does not stop the others):

  gha-stream   the Actions cache *service*, driven directly: reserve, upload,
               then query by key, take the signed archiveLocation, and stream
               `zstd -d | tar -x` straight from the blob. No archive file is
               written to disk. This is the path actions/cache hides.
  gha-actions  the same payload through actions/cache's own save/restore, as a
               baseline for the direct client (run by the workflow, not here).
  s3           PUT and GET of the same payload against the S3-compatible bucket
               the workflows already configure (DigitalOcean Spaces), so the
               faster tier is chosen on evidence and not on geography.

Everything is scoped to a per-run key, so a probe never reads or deletes a real
cache entry. Run it from a workflow_dispatch job; ACTIONS_CACHE_URL and
ACTIONS_RUNTIME_TOKEN only exist inside a job.
"""

from __future__ import annotations

import argparse
import binascii
import hashlib
import hmac
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path


# --- timing ------------------------------------------------------------------


@dataclass
class Phase:
    name: str
    seconds: float = 0.0
    bytes: int = 0
    note: str = ""
    ok: bool = True

    @property
    def mb_per_s(self) -> float:
        if self.seconds <= 0 or self.bytes <= 0:
            return 0.0
        return (self.bytes / (1024 * 1024)) / self.seconds


@dataclass
class Report:
    phases: list[Phase] = field(default_factory=list)
    payload_bytes: int = 0

    def add(self, phase: Phase) -> Phase:
        self.phases.append(phase)
        print(
            f"[probe] {phase.name}: {'ok' if phase.ok else 'FAILED'} "
            f"{phase.seconds:.2f}s {phase.bytes / (1024 * 1024):.1f}MiB "
            f"{phase.mb_per_s:.1f}MiB/s {phase.note}",
            flush=True,
        )
        return phase

    def as_dict(self) -> dict:
        return {
            "payload_bytes": self.payload_bytes,
            "phases": [
                {
                    "name": p.name,
                    "ok": p.ok,
                    "seconds": round(p.seconds, 3),
                    "bytes": p.bytes,
                    "mib_per_s": round(p.mb_per_s, 2),
                    "note": p.note,
                }
                for p in self.phases
            ],
        }


# --- payload -----------------------------------------------------------------


def build_payload(root: Path, size_bytes: int) -> Path:
    """A tar.zst of about ``size_bytes`` with realistically compressible text.

    Real cook layers are dependency artifacts: mostly source and rlib bytes that
    zstd roughly halves. Random bytes would make streaming look artificially slow
    on the decompression side and unrealistically fast on the upload side.
    """
    # Fresh random bytes encoded as hex on every write: genuinely incompressible
    # content that still compresses about 2:1, which is the ratio real dependency
    # artifacts show. A repeated blob would collapse to nothing; pure random bytes
    # would sit at 1:1 and understate the compression cost of a real layer.
    raw = root / "payload.bin"
    written = 0
    with raw.open("wb") as handle:
        while written < size_bytes:
            piece = binascii.hexlify(os.urandom(64 * 1024))
            handle.write(piece)
            written += len(piece)
    staged = root / "stage"
    staged.mkdir(exist_ok=True)
    # Many small-ish files, like a target/ tree, so tar entry cost is realistic.
    chunk = 4 * 1024 * 1024
    with raw.open("rb") as handle:
        index = 0
        while True:
            piece = handle.read(chunk)
            if not piece:
                break
            (staged / f"obj-{index:05d}.bin").write_bytes(piece)
            index += 1
    raw.unlink()
    archive = root / "payload.tar.zst"
    subprocess.run(
        ["tar", "--zstd", "-cf", str(archive), "-C", str(staged), "."],
        check=True,
    )
    shutil.rmtree(staged, ignore_errors=True)
    return archive


# --- Actions cache service ---------------------------------------------------


def _cache_headers(token: str) -> dict:
    return {
        "authorization": f"Bearer {token}",
        "accept": "application/json",
        "content-type": "application/octet-stream",
    }


def _http(method: str, url: str, headers: dict, body=None, timeout: int = 600):
    request = urllib.request.Request(url, data=body, method=method, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.status, dict(response.headers), response.read()
    except urllib.error.HTTPError as err:
        return err.code, dict(err.headers or {}), err.read() or b""


def probe_gha_stream(archive: Path, report: Report, key: str) -> None:
    base = os.environ.get("ACTIONS_CACHE_URL", "").rstrip("/")
    token = os.environ.get("ACTIONS_RUNTIME_TOKEN", "")
    if not (base and token):
        report.add(
            Phase("gha-stream", note="not a job: no ACTIONS_CACHE_URL/RUNTIME_TOKEN", ok=False)
        )
        return
    # The probe picks its own version string; a real client uses
    # @actions/cache's getCacheVersion over the restore path list, which is the
    # whole reason a later transfer between tiers is otherwise impossible.
    version = hashlib.sha256(f"probe-{key}".encode()).hexdigest()
    size = archive.stat().st_size
    headers = _cache_headers(token)
    api = f"{base}/_apis/artifactcache"

    started = time.monotonic()
    status, _hdrs, body = _http(
        "POST",
        f"{api}/caches",
        headers,
        json.dumps({"key": key, "version": version, "cacheSize": size}).encode(),
    )
    if status not in (200, 201):
        report.add(
            Phase("gha-reserve", time.monotonic() - started, note=f"HTTP {status} {body[:160]!r}", ok=False)
        )
        return
    cache_id = json.loads(body or b"{}").get("cacheId")
    report.add(Phase("gha-reserve", time.monotonic() - started, note=f"cacheId={cache_id}"))

    started = time.monotonic()
    offset = 0
    chunk = 32 * 1024 * 1024
    with archive.open("rb") as handle:
        while True:
            piece = handle.read(chunk)
            if not piece:
                break
            upload_headers = dict(headers)
            upload_headers["content-range"] = f"bytes {offset}-{offset + len(piece) - 1}/*"
            status, _hdrs, _body = _http(
                "PATCH", f"{api}/caches/{cache_id}", upload_headers, piece
            )
            if status not in (200, 204):
                report.add(
                    Phase("gha-upload", time.monotonic() - started, offset, note=f"HTTP {status}", ok=False)
                )
                return
            offset += len(piece)
    status, _hdrs, _body = _http(
        "POST", f"{api}/caches/{cache_id}?size={size}", headers, b""
    )
    report.add(
        Phase(
            "gha-upload",
            time.monotonic() - started,
            size,
            note=f"commit HTTP {status}",
            ok=status in (200, 204),
        )
    )

    started = time.monotonic()
    query = urllib.parse.urlencode({"keys": key, "version": version})
    status, _hdrs, body = _http("GET", f"{api}/cache?{query}", headers)
    if status != 200:
        report.add(Phase("gha-query", time.monotonic() - started, note=f"HTTP {status}", ok=False))
        return
    entry = json.loads(body)
    location = entry.get("archiveLocation")
    matched = entry.get("cacheKey")
    report.add(
        Phase(
            "gha-query",
            time.monotonic() - started,
            note=f"matched={matched}",
            ok=bool(location),
        )
    )
    if not location:
        return

    # The interesting phase: stream the blob through zstd and tar with no
    # intermediate archive on disk.
    dest = Path(tempfile.mkdtemp(prefix="probe-extract-"))
    started = time.monotonic()
    zstd = subprocess.Popen(
        ["zstd", "-dc"], stdin=subprocess.PIPE, stdout=subprocess.PIPE
    )
    assert zstd.stdin is not None and zstd.stdout is not None
    untar = subprocess.Popen(
        ["tar", "-x", "-C", str(dest)],
        stdin=zstd.stdout,
        stdout=subprocess.DEVNULL,
    )
    received = 0
    try:
        with urllib.request.urlopen(location, timeout=600) as response:
            while True:
                piece = response.read(1024 * 1024)
                if not piece:
                    break
                received += len(piece)
                zstd.stdin.write(piece)
        zstd.stdin.close()
        untar.wait(timeout=900)
        zstd.wait(timeout=900)
        ok = untar.returncode == 0 and received == size
        report.add(
            Phase(
                "gha-stream-extract",
                time.monotonic() - started,
                received,
                note=f"files={sum(1 for _ in dest.rglob('*') if _.is_file())}",
                ok=ok,
            )
        )
    except Exception as error:  # noqa: BLE001 - a probe phase must not abort the run
        report.add(
            Phase("gha-stream-extract", time.monotonic() - started, received, note=str(error)[:160], ok=False)
        )
    finally:
        shutil.rmtree(dest, ignore_errors=True)
        try:
            _http("DELETE", f"{base}/_apis/artifactcache/caches/{cache_id}", headers)
        except Exception:  # noqa: BLE001
            pass


# --- S3 ----------------------------------------------------------------------


def _sigv4_headers(
    method: str, url: str, region: str, access: str, secret: str, payload: bytes | None
) -> tuple[dict, str]:
    parsed = urllib.parse.urlsplit(url)
    amz_date = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    date_stamp = amz_date[:8]
    payload_hash = hashlib.sha256(payload or b"").hexdigest()
    canonical_headers = f"host:{parsed.netloc}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
    signed_headers = "host;x-amz-content-sha256;x-amz-date"
    canonical_request = (
        f"{method}\n{parsed.path or '/'}\n{parsed.query}\n{canonical_headers}\n"
        f"{signed_headers}\n{payload_hash}"
    )
    scope = f"{date_stamp}/{region}/s3/aws4_request"
    string_to_sign = (
        "AWS4-HMAC-SHA256\n"
        f"{amz_date}\n{scope}\n"
        f"{hashlib.sha256(canonical_request.encode()).hexdigest()}"
    )
    key = f"AWS4{secret}".encode()
    for part in (date_stamp, region, "s3", "aws4_request"):
        key = hmac.new(key, part.encode(), hashlib.sha256).digest()
    signing = hmac.new(key, string_to_sign.encode(), hashlib.sha256).hexdigest()
    headers = {
        "x-amz-date": amz_date,
        "x-amz-content-sha256": payload_hash,
        "authorization": (
            f"AWS4-HMAC-SHA256 Credential={access}/{scope}, "
            f"SignedHeaders={signed_headers}, Signature={signing}"
        ),
    }
    return headers, payload_hash


def probe_s3(archive: Path, report: Report) -> None:
    bucket = os.environ.get("PROBE_S3_BUCKET", "").strip()
    endpoint = os.environ.get("PROBE_S3_ENDPOINT", "").strip()
    region = os.environ.get("PROBE_S3_REGION", "").strip() or "us-east-1"
    access = os.environ.get("SOLDR_S3_CACHE_ACCESS_KEY", "").strip()
    secret = os.environ.get("SOLDR_S3_CACHE_SECRET_KEY", "").strip()
    if not (bucket and endpoint and access and secret):
        report.add(Phase("s3", note="no bucket/endpoint/credentials", ok=False))
        return
    object_key = f"cache-probe/{uuid.uuid4().hex}.tar.zst"
    url = f"https://{endpoint}/{bucket}/{object_key}"
    size = archive.stat().st_size

    started = time.monotonic()
    payload = archive.read_bytes()
    sign_started = time.monotonic()
    headers, _hash = _sigv4_headers("PUT", url, region, access, secret, payload)
    headers["content-length"] = str(len(payload))
    status, _hdrs, body = _http("PUT", url, headers, payload, timeout=900)
    sign_seconds = time.monotonic() - sign_started
    if status not in (200, 201):
        report.add(Phase("s3-put", time.monotonic() - started, size, note=f"HTTP {status} {body[:120]!r}", ok=False))
        return
    report.add(
        Phase(
            "s3-put",
            time.monotonic() - started,
            size,
            note=f"sigv4={sign_seconds:.2f}s",
        )
    )

    started = time.monotonic()
    headers, _hash = _sigv4_headers("GET", url, region, access, secret, None)
    received = 0
    try:
        request = urllib.request.Request(url, method="GET", headers=headers)
        with urllib.request.urlopen(request, timeout=900) as response:
            while True:
                piece = response.read(1024 * 1024)
                if not piece:
                    break
                received += len(piece)
        report.add(Phase("s3-get", time.monotonic() - started, received, ok=received == size))
    except Exception as error:  # noqa: BLE001
        report.add(Phase("s3-get", time.monotonic() - started, received, note=str(error)[:160], ok=False))

    # Leaving the object behind would pollute the bucket; a probe run deletes it.
    try:
        headers, _hash = _sigv4_headers("DELETE", url, region, access, secret, None)
        _http("DELETE", url, headers, None)
    except Exception:  # noqa: BLE001
        pass


# --- main --------------------------------------------------------------------


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--size-mib", type=int, default=512, help="payload size before compression")
    parser.add_argument("--key", default="", help="cache key for the streaming phase")
    parser.add_argument("--skip-gha", action="store_true")
    parser.add_argument("--skip-s3", action="store_true")
    parser.add_argument("--json-out", default="")
    args = parser.parse_args(argv[1:])

    report = Report()
    work = Path(tempfile.mkdtemp(prefix="cache-probe-"))
    try:
        build_started = time.monotonic()
        archive = build_payload(work, args.size_mib * 1024 * 1024)
        report.payload_bytes = archive.stat().st_size
        report.add(
            Phase(
                "build-payload",
                time.monotonic() - build_started,
                report.payload_bytes,
                note=f"{args.size_mib}MiB raw -> tar.zst",
            )
        )
        key = args.key or f"cache-probe-{uuid.uuid4().hex[:12]}"
        if not args.skip_gha:
            probe_gha_stream(archive, report, key)
        if not args.skip_s3:
            probe_s3(archive, report)
    finally:
        shutil.rmtree(work, ignore_errors=True)

    summary = report.as_dict()
    print(json.dumps(summary, indent=2))
    if args.json_out:
        Path(args.json_out).write_text(json.dumps(summary, indent=2), encoding="utf-8")
    return 0 if all(p["ok"] for p in summary["phases"]) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
