#!/bin/sh
# Run a task in Docker the way zackees/bosn#666 specifies, until bosn does it
# itself (ci/docker_task.py). The logic lives in Python (GEN-005, #962).
exec uv run --script "$(dirname "$0")/docker_task.py" "$@"
