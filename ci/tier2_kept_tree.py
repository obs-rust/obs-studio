#!/usr/bin/env -S uv run --script
"""Kept Tier 2 build tree and local tier2-tree handoff on the Dell (#4954 P5, P7).

    uv run --no-project ci/tier2_kept_tree.py restore --kept DIR --build BUILD --src SRC
    uv run --no-project ci/tier2_kept_tree.py save    --kept DIR --build BUILD --src SRC
    uv run --no-project ci/tier2_kept_tree.py handoff-put --root DIR --key RUN --pack PACK --labels JSON --marker FILE
    uv run --no-project ci/tier2_kept_tree.py handoff-get --root DIR --key RUN --dest PACK

Kept tree. A trusted merge-queue build moves its tree (`BUILD/tree` plus the
`tree.rustc` stamp validate.sh's stale_build.py keeps) into a per-runner
directory on the host's tmpfs. Before saving, `zccache snapshot` records the
size, mtime and BLAKE3 of every source file. The next run moves the tree back
to the same absolute path and runs `zccache replay`: unchanged sources get
their recorded mtimes back, so ninja and cargo rebuild only what changed.
Changed, missing or unverifiable files keep their fresh checkout mtime and
rebuild. The tree is trusted only behind `.clean`. restore removes the marker
before touching anything and save writes it last (atomic rename after
`sync -f`), so a job that dies in between leaves a tree the next run throws
away.

Handoff. The build job copies its pack (tier2-tree.tar.zst and
runtime-packages.txt) to `<root>/<run id>/` with a SHA256SUMS file. The test
jobs verify it and use it instead of downloading the artifact. The upload is
skipped only when the test jobs' runner labels name this host (the marker
file's content), so they cannot land anywhere without the handoff. Entries
older than a day, or beyond a total byte budget, are pruned on every put.

Every subcommand prints one summary line, writes `key=value` outputs to
$GITHUB_OUTPUT, and exits 0 on its own failures: a cache must never fail a
job. Tested by ci/test_tier2_kept_tree.py.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Callable

KEPT = ("tree", "tree.rustc")
MANIFEST = "sources.json"
CLEAN = ".clean"
SUMS = "SHA256SUMS"
HANDOFF_MAX_AGE_S = 86400
HANDOFF_MAX_BYTES = 8 * 1024**3

Runner = Callable[..., subprocess.CompletedProcess]


def _fsync_dir(path: Path) -> None:
    try:
        fd = os.open(path, os.O_RDONLY)
    except OSError:
        return
    try:
        os.fsync(fd)
    except OSError:
        pass
    finally:
        os.close(fd)


def _write_atomic(path: Path, text: str) -> None:
    tmp = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    with open(tmp, "w") as f:
        f.write(text)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    _fsync_dir(path.parent)


def _unlink_durable(path: Path) -> None:
    try:
        path.unlink()
    except FileNotFoundError:
        return
    _fsync_dir(path.parent)


def _remove(path: Path) -> None:
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path, ignore_errors=True)
    elif path.exists() or path.is_symlink():
        path.unlink()


def _sync(path: Path) -> None:
    if subprocess.run(["sync", "-f", str(path)], check=False).returncode != 0:
        os.sync()


def restore(kept: Path, build: Path, src: Path, zccache: str = "zccache", run: Runner = subprocess.run) -> dict:
    """Move a clean kept tree to BUILD and replay source mtimes."""
    if not (kept / CLEAN).is_file():
        for name in (*KEPT, MANIFEST):
            _remove(kept / name)
        return {"restored": False, "reason": "no clean kept tree", "replay": None}
    _unlink_durable(kept / CLEAN)
    build.mkdir(parents=True, exist_ok=True)
    for name in KEPT:
        if (kept / name).exists():
            _remove(build / name)
            shutil.move(str(kept / name), str(build / name))
    replay = None
    manifest = kept / MANIFEST
    if manifest.is_file():
        result = run([zccache, "replay", "--workspace", str(src), "--manifest", str(manifest), "--json"],
                     capture_output=True, text=True, check=False)
        if result.returncode == 0:
            try:
                replay = json.loads(result.stdout)
            except ValueError:
                replay = None
    return {"restored": True, "reason": "clean kept tree", "replay": replay}


def save(kept: Path, build: Path, src: Path, zccache: str = "zccache", run: Runner = subprocess.run) -> dict:
    """Snapshot sources, move BUILD's tree into KEPT, then mark it clean."""
    if not (build / "tree").is_dir():
        return {"saved": False, "reason": f"no tree at {build / 'tree'}"}
    kept.mkdir(parents=True, exist_ok=True)
    _unlink_durable(kept / CLEAN)
    for name in (*KEPT, MANIFEST):
        _remove(kept / name)
    tmp = kept / f".{MANIFEST}.tmp"
    argv = [zccache, "snapshot", "--workspace", str(src), "--out", str(tmp)]
    # Workspace caches are not build inputs; hashing them is wasted time.
    for entry in sorted(src.glob(".tier2-*")):
        argv += ["--exclude", str(entry)]
    try:
        build.resolve().relative_to(src.resolve())
        argv += ["--exclude", str(build)]
    except ValueError:
        pass
    result = run(argv, capture_output=True, text=True, check=False)
    if result.returncode != 0 or not tmp.is_file():
        _remove(tmp)
        return {"saved": False, "reason": f"zccache snapshot failed: {(result.stderr or '').strip()[:200]}"}
    os.replace(tmp, kept / MANIFEST)
    for name in KEPT:
        if (build / name).exists():
            shutil.move(str(build / name), str(kept / name))
    _sync(kept)
    _write_atomic(kept / CLEAN, json.dumps({"saved_at": time.time()}) + "\n")
    return {"saved": True, "reason": "kept"}


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _host(marker: Path) -> str:
    try:
        return marker.read_text().strip()
    except OSError:
        return ""


def _labels(raw: str) -> list[str]:
    try:
        value = json.loads(raw) if raw.strip() else []
    except ValueError:
        return []
    return [v for v in value if isinstance(v, str)] if isinstance(value, list) else []


def handoff_is_local(labels: str, marker: Path) -> bool:
    """True only when the consumer labels pin this persistent-cache host."""
    host = _host(marker)
    return bool(host) and host in _labels(labels)


def _tree_bytes(path: Path) -> int:
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


def prune(root: Path, max_age_s: int = HANDOFF_MAX_AGE_S, max_bytes: int = HANDOFF_MAX_BYTES, now: float | None = None) -> None:
    now = time.time() if now is None else now
    entries = []
    for entry in root.iterdir():
        age = now - entry.stat().st_mtime
        if entry.name.startswith("."):
            if age > 3600:
                _remove(entry)  # an interrupted put
            continue
        if age > max_age_s:
            _remove(entry)
        else:
            entries.append((entry.stat().st_mtime, entry, _tree_bytes(entry)))
    total = sum(size for _, _, size in entries)
    for _, entry, size in sorted(entries, key=lambda e: e[0]):
        if total <= max_bytes:
            break
        _remove(entry)
        total -= size


def handoff_put(root: Path, key: str, pack: Path, labels: str, marker: Path) -> dict:
    """Publish PACK under ROOT/KEY; local=True when the test jobs must land on this host."""
    tmp = root / f".{key}.{os.getpid()}.tmp"
    _remove(tmp)
    tmp.mkdir(parents=True)
    sums = []
    for f in sorted(pack.iterdir()):
        if f.is_file():
            shutil.copy2(f, tmp / f.name)
            sums.append(f"{_sha256(tmp / f.name)}  {f.name}\n")
    (tmp / SUMS).write_text("".join(sums))
    dest = root / key
    _remove(dest)
    os.replace(tmp, dest)
    os.utime(dest)
    prune(root)
    host = _host(marker)
    local = handoff_is_local(labels, marker)
    return {"local": local, "reason": f"host {host or '?'} {'in' if local else 'not in'} the test jobs' labels"}


def handoff_get(root: Path, key: str, dest: Path) -> dict:
    """Copy a verified handoff to DEST; hit=False means download the artifact."""
    entry = root / key
    sums = entry / SUMS
    if not sums.is_file():
        return {"hit": False, "reason": f"no handoff {entry}"}
    wanted = []
    for line in sums.read_text().splitlines():
        digest, _, name = line.partition("  ")
        path = entry / name
        if not name or "/" in name or not path.is_file() or _sha256(path) != digest:
            return {"hit": False, "reason": f"handoff {entry} failed verification ({name or line})"}
        wanted.append(name)
    dest.mkdir(parents=True, exist_ok=True)
    for name in wanted:
        shutil.copy2(entry / name, dest / name)
    return {"hit": True, "reason": f"verified {len(wanted)} files from {entry}"}


def _outputs(values: dict) -> None:
    out = os.environ.get("GITHUB_OUTPUT")
    if not out:
        return
    with open(out, "a") as f:
        for k, v in values.items():
            if isinstance(v, bool):
                f.write(f"{k}={'true' if v else 'false'}\n")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("restore", "save"):
        p = sub.add_parser(name)
        p.add_argument("--kept", type=Path, required=True)
        p.add_argument("--build", type=Path, required=True)
        p.add_argument("--src", type=Path, required=True)
        p.add_argument("--zccache", default="zccache")
    p = sub.add_parser("handoff-put")
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--key", required=True)
    p.add_argument("--pack", type=Path, required=True)
    p.add_argument("--labels", default="")
    p.add_argument("--marker", type=Path, required=True)
    p = sub.add_parser("handoff-get")
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--key", required=True)
    p.add_argument("--dest", type=Path, required=True)
    args = parser.parse_args(argv)
    t0 = time.monotonic()
    try:
        if args.cmd == "restore":
            result = restore(args.kept, args.build, args.src, args.zccache)
        elif args.cmd == "save":
            result = save(args.kept, args.build, args.src, args.zccache)
        elif args.cmd == "handoff-put":
            result = handoff_put(args.root, args.key, args.pack, args.labels, args.marker)
        else:
            result = handoff_get(args.root, args.key, args.dest)
    except OSError as err:
        result = {"restored": False, "saved": False, "local": False, "hit": False, "reason": f"error: {err}"}
    _outputs(result)
    print(f"tier2-kept-tree {args.cmd}: {json.dumps(result)} in {time.monotonic() - t0:.1f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
