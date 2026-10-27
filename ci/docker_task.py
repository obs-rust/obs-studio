#!/usr/bin/env -S uv run --script
"""Run a task in Docker the way zackees/bosn#666 specifies, until bosn does it
itself. Each run gets an ephemeral container with a deterministic name; its
build state lives in named volumes that the next run reattaches.

`ci/docker-task.sh` is a shim that execs this file through uv; it keeps the path
and the arguments below (GEN-005, #962).

    ci/docker-task.sh run --name NAME [options] -- CMD...
    ci/docker-task.sh gc  --name NAME [--image IMAGE] [--apply] [--apply-legacy]

run:
  --name NAME             task name. Container: NAME-<checkout id>, where the
                          checkout id is the first 12 hex of sha256(repo path).
                          The name is the singleton lock: one container per
                          task and checkout. A running one refuses the run
                          (exit 75), or --wait waits for it to exit (docker
                          wait, not polling); a stopped leftover is removed.
  --repo PATH             checkout (default: the git top level of $PWD)
  --image IMAGE           image repository; with --dockerfile it is built
                          and tagged IMAGE:<content hash>, and only the newest
                          --keep-images (default 3) tags of IMAGE are kept
  --dockerfile FILE --context DIR [--build-arg K=V]...
  --source ro|rw|overlay  how the checkout is mounted (default ro):
                          ro: read-only bind; rw: read-write bind;
                          overlay: read-only lower layer + a throwaway upper
                          layer, so writes succeed and vanish after the run
  --source-dest PATH      where the checkout appears (default /src)
  --stack-volume VOL:DEST[:NAME]
                          per-checkout volume, NAME-VOL-<checkout id> unless
                          NAME given; reattached every run
  --machine-volume VOL:DEST
                          shared volume NAME-VOL, for caches that lock
                          (ccache, cargo registry)
  --mount SRC:DEST[:ro]   extra bind mount
  --env K=V               environment for the task
  --prune-cargo DIR       after every run, keep only the newest artifacts in
                          the cargo target dir DIR (container path) and each
                          DIR/<triple>: soldr cache prune-target
                          --keep-latest. Safe when each target is built one
                          way (one profile, one feature set), as these tasks
                          are. soldr comes from a shared tools volume
                          (docker-task-soldr-<version>), fetched once at the
                          version and SHA256 cmake/common/soldr.cmake pins.
  --budget VOL=SIZE       cap a stack or machine volume (SIZE: bytes, or with
                          K, M or G): after every run, a volume over its budget
                          is removed, so the next run starts it cold. Bounds
                          what would otherwise only ever grow (cargo targets,
                          compiler caches). Measured at most once an hour per
                          volume (DOCKER_TASK_BUDGET_SECS, default 3600), all
                          due volumes at once, so most runs pay nothing.
  --clean                 remove this checkout's stack volumes first
  --clean-all             also remove the machine volumes
  --keep                  keep the container after the run (no --rm)
  --wait                  wait for a running container of the same name

gc: lists, and with --apply removes, what the runs leave behind: stack
volumes labelled with a checkout path that no longer exists, stopped
containers of the task, and IMAGE tags beyond the newest three. Volumes
made before labels existed (NAME-*-<12 hex>, unlabelled) are only listed;
--apply-legacy removes those not belonging to this checkout. Volumes in use
by a container are never removed.

Exit status: 2 for a usage or argument error, 75 when a container of the same
name is already running and --wait was not given, otherwise the status of the
command that ran in the container.
"""

from __future__ import annotations

import hashlib
import os
import re
import shlex
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path
from typing import NoReturn, Sequence

LABEL = "io.github.obs-rust.docker-task"
KEEP_IMAGES = 3
BUDGET_SECS = 3600
SOLDR_MOUNT = "/opt/docker-task-soldr"
SOURCE_MODES = ("ro", "rw", "overlay")
BYTES_UNITS = {"k": 1024, "m": 1024 * 1024, "g": 1024 * 1024 * 1024}

# The prune runs in the same container, after the command, whatever its exit
# status; the command's status is kept.
PRUNE_SCRIPT = r'''status=0; "$@" || status=$?
for dir in $DOCKER_TASK_PRUNE; do
  [ -d "$dir" ] || continue
  for t in "$dir" "$dir"/*-*-*; do
    [ -d "$t" ] || continue
    /opt/docker-task-soldr/soldr cache prune-target --keep-latest --force --allow-unpinned --json "$t" 2>/dev/null |
      sed -n "s/.*\"reclaimed_human\": *\"\([^\"]*\)\".*/docker-task: pruned \1 from ${t##*/}/p" >&2 || true
  done
done
exit $status'''

USAGE = "usage: docker-task.sh run|gc --name NAME ... (see the header)"


def die(message: str) -> NoReturn:
    """The script's die(): the message on stderr, then exit 2."""
    print(f"docker-task: {message}", file=sys.stderr)
    raise SystemExit(2)


def quiet(args: Sequence[str]) -> str:
    """The command's stdout, or "" when it fails or its object is missing."""
    try:
        result = subprocess.run(list(args), capture_output=True, text=True)
    except OSError:
        return ""
    return result.stdout


def succeeds(args: Sequence[str]) -> bool:
    """Whether the command exits 0, with its output discarded."""
    try:
        return subprocess.run(list(args), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    except OSError:
        return False


def checkout_id(path: str) -> str:
    """A checkout key from its repo path: the first 12 hex of the SHA256."""
    return hashlib.sha256(path.encode()).hexdigest()[:12]


def to_bytes(spec: str) -> int:
    """`512`, `300K`, `20M`, `8G` -> bytes, as the shell's arithmetic had it."""
    if spec and spec[-1].lower() in BYTES_UNITS:
        return int(spec[:-1]) * BYTES_UNITS[spec[-1].lower()]
    return int(spec)


def volume_exists(name: str) -> bool:
    return bool(quiet(["docker", "volume", "inspect", "-f", "{{.Name}}", name]).strip())


def volume_in_use(name: str) -> bool:
    return bool(quiet(["docker", "ps", "-aq", "--filter", f"volume={name}"]).strip())


def ensure_volume(name: str, scope: str, repo: str, task: str) -> None:
    """Creates the labelled volume when it is not there yet."""
    if volume_exists(name):
        return
    subprocess.run(
        [
            "docker", "volume", "create",
            "--label", f"{LABEL}.task={task}",
            "--label", f"{LABEL}.scope={scope}",
            "--label", f"{LABEL}.checkout={repo}",
            name,
        ],
        stdout=subprocess.DEVNULL,
        check=False,
    )


def remove_volume(name: str) -> None:
    """`--clean`'s removal: never one a container is still using."""
    if volume_in_use(name):
        print(f"docker-task: {name} is in use, not removed", file=sys.stderr)
        return
    succeeds(["docker", "volume", "rm", name])


def image_hash(dockerfile: Path, context: Path, build_args: Sequence[str]) -> str:
    """Content hash of the Dockerfile, its context and the build arguments: the
    image tag, so an unchanged image is never rebuilt.

    The context is every file under it in LC_ALL=C path order (`find -type f`
    then `sort -z`), concatenated; the build arguments follow, one per line.
    """
    digest = hashlib.sha256()
    digest.update(dockerfile.read_bytes())
    files = [path for path in context.rglob("*") if path.is_file() and not path.is_symlink()]
    for path in sorted(files, key=lambda p: f"./{p.relative_to(context).as_posix()}".encode()):
        digest.update(path.read_bytes())
    # `printf '%s\n' "$@"` prints one line per argument, and a lone newline for
    # none at all.
    for argument in build_args or [""]:
        digest.update(f"{argument}\n".encode())
    return digest.hexdigest()[:16]


def ordered_tags(listing: str) -> list[str]:
    """The tags of `docker image ls --format '{{.CreatedAt}}\\t{{.Tag}}'`,
    newest first.

    Lines sharing a CreatedAt fall to the reverse tag string, exactly as
    `LC_ALL=C sort -r | cut -f2` left them: the image just built can sort past
    KEEP, so its tag must never be pruned (#1218).
    """
    lines = sorted((line for line in listing.splitlines() if line), key=lambda line: line.encode(), reverse=True)
    tags = [line.split("\t")[1] if "\t" in line else line for line in lines]
    return [tag for tag in tags if tag != "<none>"]


def prune_images(image: str, keep: int, apply: bool, in_use: str = "", announce: bool = True) -> None:
    """Keeps the newest KEEP tags of IMAGE.

    IN_USE, a full tag, is never removed and not counted (#1218).
    """
    listing = quiet(["docker", "image", "ls", image, "--format", r"{{.CreatedAt}}\t{{.Tag}}"])
    count = 0
    for tag in ordered_tags(listing):
        if f"{image}:{tag}" == in_use:
            continue
        count += 1
        if count <= keep:
            continue
        if not apply:
            if announce:
                print(f"image {image}:{tag} (beyond the newest {keep})")
        elif succeeds(["docker", "image", "rm", f"{image}:{tag}"]):
            if announce:
                print(f"removed image {image}:{tag}")
        elif announce:
            print(f"kept image {image}:{tag} (in use)")


def soldr_pin(cmake: Path, triple: str) -> tuple[str, str]:
    """The soldr version and this triple's SHA256, from cmake/common/soldr.cmake."""
    text = cmake.read_text(encoding="utf-8")
    version = re.search(r"^set\(OBS_SOLDR_VERSION ([0-9.]+)\)$", text, re.MULTILINE)
    sha = re.search(rf"^set\(_obs_soldr_sha256_{re.escape(triple)} ([0-9a-f]+)\)$", text, re.MULTILINE)
    return (version.group(1) if version else ""), (sha.group(1) if sha else "")


def ensure_soldr_tools(image: str) -> str:
    """A shared volume holding the soldr binary, at the version and SHA256
    cmake/common/soldr.cmake pins, fetched once with the task image's curl."""
    cmake = Path(__file__).resolve().parent.parent / "cmake" / "common" / "soldr.cmake"
    architecture = quiet(["docker", "info", "-f", "{{.Architecture}}"]).strip()
    triple = "aarch64-unknown-linux-gnu" if architecture in ("aarch64", "arm64") else "x86_64-unknown-linux-gnu"
    version, sha = soldr_pin(cmake, triple)
    volume = f"docker-task-soldr-{version}"
    if not volume_exists(volume):
        subprocess.run(
            ["docker", "volume", "create", "--label", f"{LABEL}.scope=tools", volume],
            stdout=subprocess.DEVNULL,
            check=False,
        )
    # Indented as the shell had it; the leading spaces are harmless in sh.
    fetch = (
        "\n    [ -x /t/soldr ] && exit 0\n"
        "    url=https://github.com/zackees/soldr/releases/download/v$0/soldr-v$0-$1.tar.zst\n"
        '    curl -fsSL "$url" -o /tmp/soldr.tar.zst &&\n'
        '      echo "$2  /tmp/soldr.tar.zst" | sha256sum -c - >/dev/null &&\n'
        "      tar --zstd -xf /tmp/soldr.tar.zst -C /t soldr"
    )
    # The fetch's own output belongs on stderr: only the volume name is stdout.
    fetched = subprocess.run(
        ["docker", "run", "--rm", "-v", f"{volume}:/t", "--entrypoint", "sh", image, "-c", fetch, version, triple, sha],
        stdout=sys.stderr,
    )
    if fetched.returncode:
        print(f"docker-task: could not fetch soldr {version} for pruning", file=sys.stderr)
    return volume


def enforce_budget(volume: str, budget: int, image: str) -> None:
    """Removes VOLUME when it holds more than BUDGET bytes (measured with du in
    a throwaway container of IMAGE), unless a container still uses it."""
    measured = quiet(["docker", "run", "--rm", "-v", f"{volume}:/v:ro", "--entrypoint", "du", image, "-sb", "/v"])
    size = measured.split("\t", 1)[0].strip()
    if not size.isdigit():
        return
    if int(size) <= budget:
        return
    if volume_in_use(volume):
        print(f"docker-task: {volume} is over budget ({size} > {budget} bytes) but in use; left", file=sys.stderr)
        return
    if succeeds(["docker", "volume", "rm", volume]):
        print(
            f"docker-task: {volume} was over budget ({size} > {budget} bytes): removed, the next run starts it cold",
            file=sys.stderr,
        )


def budget_seconds() -> int:
    try:
        return int(os.environ.get("DOCKER_TASK_BUDGET_SECS", BUDGET_SECS))
    except ValueError:
        return BUDGET_SECS


def budget_state_dir() -> Path:
    """Where the per-volume budget stamps live."""
    state_home = os.environ.get("XDG_STATE_HOME") or str(Path.home() / ".local" / "state")
    return Path(state_home) / "docker-task"


def budget_limits(specs: Sequence[str]) -> list[tuple[str, int]]:
    """`VOL=SIZE` pairs, SIZE in bytes or with a K, M or G suffix."""
    limits: list[tuple[str, int]] = []
    for spec in specs:
        volume, separator, size = spec.partition("=")
        if not separator or not volume:
            die(f"--budget takes VOL=SIZE, not {spec}")
        try:
            limits.append((volume, to_bytes(size)))
        except ValueError:
            die(f"--budget {volume} has a size that is not bytes or K/M/G: {size}")
    return limits


def checked_recently(stamp: Path, now: int) -> bool:
    """Whether the volume was measured within DOCKER_TASK_BUDGET_SECS."""
    try:
        last = int(stamp.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return False
    return now - last < budget_seconds()


def due_budget_volumes(
    limits: Sequence[tuple[str, int]], names: Sequence[str], state: Path, now: int
) -> list[tuple[str, int]]:
    """The budgeted volumes that may be measured now, each stamping itself."""
    due: list[tuple[str, int]] = []
    for volume, limit in limits:
        for pair in names:
            key, _, name = pair.partition("=")
            if key != volume:
                continue
            stamp = state / f"{name}.budget-checked"
            if checked_recently(stamp, now):
                continue
            stamp.write_text(f"{now}\n", encoding="utf-8")
            due.append((name, limit))
    return due


def check_budgets(limits: Sequence[tuple[str, int]], names: Sequence[str], image: str) -> None:
    """After every run (a failed one grows the volumes too), at most once per
    DOCKER_TASK_BUDGET_SECS per volume; due volumes in parallel."""
    state = budget_state_dir()
    state.mkdir(parents=True, exist_ok=True)
    due = due_budget_volumes(limits, names, state, int(time.time()))
    if not due:
        return
    with ThreadPoolExecutor(max_workers=len(due)) as pool:
        list(pool.map(lambda item: enforce_budget(item[0], item[1], image), due))


@dataclass
class RunOptions:
    task: str = ""
    image: str = ""
    repo: str = ""
    dockerfile: str = ""
    context: str = ""
    keep_images: int = KEEP_IMAGES
    source: str = "ro"
    source_dest: str = "/src"
    clean: int = 0
    keep: bool = False
    wait: bool = False
    build_args: list[str] = field(default_factory=list)
    stack: list[str] = field(default_factory=list)
    machine: list[str] = field(default_factory=list)
    mounts: list[str] = field(default_factory=list)
    envs: list[str] = field(default_factory=list)
    budgets: list[str] = field(default_factory=list)
    prune: list[str] = field(default_factory=list)
    command: list[str] = field(default_factory=list)


# Option -> (attribute, is it a list); the flags are handled separately.
RUN_OPTIONS: dict[str, tuple[str, bool]] = {
    "--name": ("task", False),
    "--repo": ("repo", False),
    "--image": ("image", False),
    "--dockerfile": ("dockerfile", False),
    "--context": ("context", False),
    "--keep-images": ("keep_images", False),
    "--source": ("source", False),
    "--source-dest": ("source_dest", False),
    "--build-arg": ("build_args", True),
    "--stack-volume": ("stack", True),
    "--machine-volume": ("machine", True),
    "--mount": ("mounts", True),
    "--env": ("envs", True),
    "--budget": ("budgets", True),
    "--prune-cargo": ("prune", True),
}


def take(option: str, rest: list[str]) -> str:
    """The option's value; an option at the end of the line is a usage error."""
    if not rest:
        die(f"{option} needs a value")
    return rest.pop(0)


def set_run_value(options: RunOptions, option: str, value: str) -> None:
    attribute, is_list = RUN_OPTIONS[option]
    if is_list:
        getattr(options, attribute).append(value)
    elif attribute == "keep_images":
        try:
            options.keep_images = int(value)
        except ValueError:
            die(f"--keep-images takes a number, not {value}")
    else:
        setattr(options, attribute, value)


def apply_run_option(options: RunOptions, option: str, rest: list[str]) -> bool:
    """Sets one `run` option; False when it is not one."""
    if option == "--clean":
        options.clean = max(options.clean, 1)
    elif option == "--clean-all":
        options.clean = 2
    elif option == "--keep":
        options.keep = True
    elif option == "--wait":
        options.wait = True
    elif option in RUN_OPTIONS:
        set_run_value(options, option, take(option, rest))
    else:
        return False
    return True


def parse_run(argv: Sequence[str]) -> RunOptions:
    """`run`'s arguments: the options in the header, then the command after --."""
    options = RunOptions()
    rest = list(argv)
    while rest:
        option = rest.pop(0)
        if option == "--":
            options.command = rest
            break
        if not apply_run_option(options, option, rest):
            die(f"unknown option {option}")
    if not options.task:
        die("--name is required")
    if not options.image:
        die("--image is required")
    if not options.command:
        die("no command after --")
    if options.source not in SOURCE_MODES:
        die("--source must be ro, rw or overlay")
    return options


def resolve_repo(repo: str) -> str:
    """The checkout to mount: --repo, else the git top level of $PWD, with
    symlinks resolved (`cd ... && pwd -P`)."""
    if not repo:
        repo = quiet(["git", "rev-parse", "--show-toplevel"]).strip()
        if not repo:
            die("--repo is required outside a git checkout")
    return os.path.realpath(repo)


def build_image(options: RunOptions) -> str:
    """The image to run: built only when its content hash is new, capped per
    task to the newest --keep-images tags."""
    if not options.dockerfile:
        return options.image
    dockerfile = Path(options.dockerfile)
    context = Path(options.context) if options.context else dockerfile.parent
    tag = f"{options.image}:{image_hash(dockerfile, context, options.build_args)}"
    if not quiet(["docker", "image", "inspect", "-f", "{{.Id}}", tag]).strip():
        build = ["docker", "build", "-q"]
        for argument in options.build_args:
            build += ["--build-arg", argument]
        build += ["-t", tag, "-f", str(dockerfile), str(context)]
        subprocess.run(build, stdout=subprocess.DEVNULL, check=False)
    prune_images(options.image, options.keep_images, True, tag, announce=False)
    return tag


def release_container(cname: str, wait: bool) -> None:
    """The singleton: one container per task and checkout."""
    state = quiet(["docker", "container", "inspect", "-f", "{{.State.Status}}", cname]).strip()
    if not state:
        return
    if state not in ("running", "restarting", "paused"):
        succeeds(["docker", "rm", "-f", cname])
        return
    if not wait:
        print(f"docker-task: {cname} is already running (one run per task and checkout; --wait to queue)", file=sys.stderr)
        raise SystemExit(75)
    print(f"docker-task: waiting for {cname} to finish", file=sys.stderr)
    quiet(["docker", "wait", cname])


def spec_fields(spec: str, count: int) -> list[str]:
    """`vol:dest[:name]` as the shell's `IFS=: read -r` split it: the last field
    keeps any further colons."""
    fields = spec.split(":", count - 1)
    return fields + [""] * (count - len(fields))


def volume_arguments(options: RunOptions, ident: str, repo: str) -> tuple[list[str], list[str]]:
    """The `-v` arguments for the stack and machine volumes, and the `vol=name`
    pairs the budgets look up."""
    args: list[str] = []
    names: list[str] = []
    for spec in options.stack:
        volume, dest, name = spec_fields(spec, 3)
        name = name or f"{options.task}-{volume}-{ident}"
        names.append(f"{volume}={name}")
        if options.clean >= 1:
            remove_volume(name)
        ensure_volume(name, "stack", repo, options.task)
        args += ["-v", f"{name}:{dest}"]
    for spec in options.machine:
        volume, dest = spec_fields(spec, 2)
        name = f"{options.task}-{volume}"
        names.append(f"{volume}={name}")
        if options.clean >= 2:
            remove_volume(name)
        ensure_volume(name, "machine", "", options.task)
        args += ["-v", f"{name}:{dest}"]
    for spec in options.mounts:
        args += ["-v", spec]
    for spec in options.envs:
        args += ["-e", spec]
    return args, names


def overlay_volume(options: RunOptions, cname: str, repo: str) -> tuple[Path, str]:
    """A read-only lower layer (`repo`) with a throwaway upper layer, so writes
    succeed and vanish after the run."""
    directory = Path(tempfile.mkdtemp(prefix="docker-task-overlay.", dir=os.environ.get("TMPDIR") or "/tmp"))
    for part in ("upper", "work"):
        (directory / part).mkdir()
    volume = f"{cname}-overlay"
    succeeds(["docker", "volume", "rm", volume])
    subprocess.run(
        [
            "docker", "volume", "create", "--driver", "local",
            "--opt", "type=overlay", "--opt", "device=overlay",
            "--opt", f"o=lowerdir={repo},upperdir={directory}/upper,workdir={directory}/work",
            "--label", f"{LABEL}.task={options.task}", "--label", f"{LABEL}.scope=overlay",
            volume,
        ],
        stdout=subprocess.DEVNULL,
        check=False,
    )
    return directory, volume


def cleanup_overlay(directory: Path, volume: str, tag: str) -> None:
    """Removes the overlay volume, then clears its root-owned upper/work dirs
    from a throwaway container, in the background so the caller doesn't wait."""
    succeeds(["docker", "volume", "rm", volume])
    quoted = shlex.quote(str(directory))
    script = (
        f"docker run --rm -v {quoted}:/overlay --entrypoint rm {shlex.quote(tag)} "
        "-rf /overlay/upper /overlay/work >/dev/null 2>&1 || true\n"
        f"rmdir {quoted} 2>/dev/null || true\n"
    )
    # </dev/null >/dev/null 2>&1 &: the caller does not wait for it.
    subprocess.Popen(
        ["sh", "-c", script], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )


def mount_source(options: RunOptions, repo: str, cname: str) -> tuple[list[str], Path | None, str]:
    """The checkout's `-v` arguments, and the overlay left to clean up."""
    if options.source == "ro":
        return ["-v", f"{repo}:{options.source_dest}:ro"], None, ""
    if options.source == "rw":
        return ["-v", f"{repo}:{options.source_dest}"], None, ""
    directory, volume = overlay_volume(options, cname, repo)
    return ["-v", f"{volume}:{options.source_dest}"], directory, volume


def prune_mounts(options: RunOptions, tag: str) -> tuple[list[str], list[str]]:
    """The soldr tools volume and the `sh -c` wrapper that prunes after the
    command; no prune requested leaves the command alone."""
    if not options.prune:
        return [], list(options.command)
    tools = ensure_soldr_tools(tag)
    args = ["-v", f"{tools}:{SOLDR_MOUNT}:ro", "-e", f"DOCKER_TASK_PRUNE={' '.join(options.prune)}"]
    return args, ["sh", "-c", PRUNE_SCRIPT, "docker-task-prune", *options.command]


def run(argv: Sequence[str]) -> int:
    options = parse_run(argv)
    repo = resolve_repo(options.repo)
    ident = checkout_id(repo)
    cname = f"{options.task}-{ident}"
    tag = build_image(options)
    release_container(cname, options.wait)

    args = ["--name", cname]
    if not options.keep:
        args.append("--rm")
    volume_args, names = volume_arguments(options, ident, repo)
    source_args, overlay_dir, overlay = mount_source(options, repo, cname)
    prune_args, command = prune_mounts(options, tag)
    args += [*volume_args, *source_args, *prune_args]

    try:
        status = subprocess.run(["docker", "run", *args, tag, *command]).returncode
        check_budgets(budget_limits(options.budgets), names, tag)
        return status
    finally:
        if overlay_dir is not None:
            cleanup_overlay(overlay_dir, overlay, tag)


@dataclass
class GcOptions:
    task: str = ""
    image: str = ""
    repo: str = ""
    apply: bool = False
    apply_legacy: bool = False


GC_OPTIONS = {"--name": "task", "--image": "image", "--repo": "repo"}
GC_FLAGS = {"--apply": "apply", "--apply-legacy": "apply_legacy"}


def parse_gc(argv: Sequence[str]) -> GcOptions:
    """`gc`'s arguments: --name, --image, --apply, --apply-legacy."""
    options = GcOptions()
    rest = list(argv)
    while rest:
        option = rest.pop(0)
        if option in GC_FLAGS:
            setattr(options, GC_FLAGS[option], True)
        elif option in GC_OPTIONS:
            setattr(options, GC_OPTIONS[option], take(option, rest))
        else:
            die(f"unknown option {option}")
    if not options.task:
        die("--name is required")
    return options


def gc_stack_volumes(options: GcOptions) -> None:
    """Stack volumes whose labelled checkout path no longer exists."""
    listed = quiet([
        "docker", "volume", "ls", "-q",
        "--filter", f"label={LABEL}.task={options.task}",
        "--filter", f"label={LABEL}.scope=stack",
    ])
    for volume in listed.split():
        checkout = quiet([
            "docker", "volume", "inspect", "-f", f'{{{{index .Labels "{LABEL}.checkout"}}}}', volume
        ]).strip()
        if Path(checkout).is_dir():
            continue
        if volume_in_use(volume):
            print(f"volume {volume} (checkout {checkout} gone, but in use)")
        elif options.apply:
            if succeeds(["docker", "volume", "rm", volume]):
                print(f"removed volume {volume} (checkout {checkout} gone)")
        else:
            print(f"volume {volume} (checkout {checkout} gone)")


def gc_legacy_volumes(options: GcOptions, own_id: str) -> None:
    """Volumes from before the labels existed (`NAME-*-<12 hex>`, unlabelled):
    only listed, unless --apply-legacy, and never this checkout's."""
    pattern = re.compile(rf"^{re.escape(options.task)}-.+-[0-9a-f]{{12}}$")
    for volume in quiet(["docker", "volume", "ls", "-q"]).split():
        if not pattern.match(volume):
            continue
        labels = quiet(["docker", "volume", "inspect", "-f", f'{{{{index .Labels "{LABEL}.task"}}}}', volume]).strip()
        if labels or volume.endswith(f"-{own_id}"):
            continue
        if volume_in_use(volume):
            print(f"legacy volume {volume} (in use)")
        elif options.apply_legacy:
            if succeeds(["docker", "volume", "rm", volume]):
                print(f"removed legacy volume {volume}")
        else:
            print(f"legacy volume {volume} (unlabelled, another checkout; --apply-legacy removes)")


def gc_containers(options: GcOptions) -> None:
    """Stopped leftovers of the task: one container per task and checkout."""
    found: list[str] = []
    for status in ("exited", "created"):
        found += quiet([
            "docker", "ps", "-aq", "--filter", f"name=^{options.task}-", "--filter", f"status={status}"
        ]).split()
    for container in found:
        if not options.apply:
            print(f"container {container} (stopped)")
        elif succeeds(["docker", "rm", container]):
            print(f"removed container {container}")


def gc(argv: Sequence[str]) -> int:
    options = parse_gc(argv)
    repo = options.repo or quiet(["git", "rev-parse", "--show-toplevel"]).strip() or os.getcwd()
    own_id = checkout_id(os.path.realpath(repo))
    gc_stack_volumes(options)
    gc_legacy_volumes(options, own_id)
    gc_containers(options)
    if options.image:
        prune_images(options.image, KEEP_IMAGES, options.apply)
    return 0


def main(argv: Sequence[str]) -> int:
    if not argv:
        die(USAGE)
    subcommand, rest = argv[0], list(argv[1:])
    if subcommand == "run":
        return run(rest)
    if subcommand == "gc":
        return gc(rest)
    die(USAGE)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
