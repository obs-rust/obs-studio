#!/usr/bin/env bash
# Prove the soldr S3 cache bucket is reachable: write a canary, read it back,
# and report the bucket's size against the 50G watermark.
#
# Lives in ci/ rather than inline in the workflow: scheduled.yaml may not carry
# a multi-line run: block (ci/test_workflow_duplicate_keys.py), and this is
# easier to run by hand.
#
#   AWS_ACCESS_KEY_ID=... AWS_SECRET_ACCESS_KEY=... ci/s3_cache_canary.sh
set -euo pipefail

BUCKET="${BUCKET:-forge-ci}"
ENDPOINT="${ENDPOINT:-https://nyc3.digitaloceanspaces.com}"
SUMMARY="${GITHUB_STEP_SUMMARY:-/dev/null}"

if [ -z "${AWS_ACCESS_KEY_ID:-}" ] || [ -z "${AWS_SECRET_ACCESS_KEY:-}" ]; then
  echo "::warning::S3 cache credentials are not configured; the remote cache is off"
  echo "S3 cache: credentials missing" >>"$SUMMARY"
  exit 0
fi

key="ci-canary/$(date -u +%Y%m%dT%H%M%SZ).txt"
echo "canary $(date -u)" >/tmp/canary.txt

if ! aws s3 cp /tmp/canary.txt "s3://$BUCKET/$key" --endpoint-url "$ENDPOINT" >/dev/null; then
  echo "::error::S3 cache write failed (bucket $BUCKET at $ENDPOINT)"
  echo "S3 cache: write FAILED" >>"$SUMMARY"
  exit 1
fi

aws s3 cp "s3://$BUCKET/$key" /tmp/canary.read --endpoint-url "$ENDPOINT" >/dev/null
if cmp -s /tmp/canary.txt /tmp/canary.read; then
  readback=ok
else
  readback=MISMATCH
fi

totals=$(aws s3 ls "s3://$BUCKET" --recursive --summarize --endpoint-url "$ENDPOINT" 2>/dev/null || true)
objects=$(printf '%s\n' "$totals" | awk '/Total Objects:/ {print $3}')
bytes=$(printf '%s\n' "$totals" | awk '/Total Size:/ {print $3}')
gib=$(( (${bytes:-0}) / 1024 / 1024 / 1024 ))

{
  echo "### S3 cache canary"
  echo "- write/read: **$readback** (object $key)"
  echo "- bucket: $BUCKET @ nyc3"
  echo "- objects: ${objects:-?}"
  echo "- size: ${bytes:-?} bytes (${gib} GiB of 50 GiB)"
} >>"$SUMMARY"

aws s3 rm "s3://$BUCKET/$key" --endpoint-url "$ENDPOINT" >/dev/null || true
[ "$readback" = ok ]
