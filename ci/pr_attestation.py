#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["prooft @ git+https://github.com/forge-ci/prooft@683bcf0b3fcf77548054c85991f2b03fdaca77bd"]
# ///
"""Base-owned PR attestation verifier (RED decision stub)."""


def evaluate(root, *, base, head, number, event, graphql, rest, now):
    return {"trusted": False, "jobs": {}}
