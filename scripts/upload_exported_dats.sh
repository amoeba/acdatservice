#!/bin/sh
# upload_exported_dats.sh — Mirror exported DATs to R2
#
# Wraps `rclone sync` (Cloudflare R2 provider) with concurrency. rclone sync
# makes the destination match the source and deletes objects that no longer
# exist locally (the rsync --delete equivalent).
#
# Required environment:
#   BUCKET                  destination R2 bucket name (no default)
#   AWS_ACCESS_KEY_ID       R2 S3 token id
#   AWS_SECRET_ACCESS_KEY   R2 S3 token secret
#   AWS_ENDPOINT_URL        https://<account-id>.r2.cloudflarestorage.com
# The token needs Object Read & Write (ListBucket + DeleteObject).
#
# Requires rclone >= 1.59 (per Cloudflare's R2 docs).
#
# Usage:
#   BUCKET=treestats-acdats-extracted ./scripts/upload_exported_dats.sh
#   DRYRUN=1 ./scripts/upload_exported_dats.sh   # preview without changes

set -eu

BUCKET="${BUCKET:-}"
DATS="portal cell highres local_english"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# ── Environment ─────────────────────────────────────────────────────────

check_env() {
  _missing=false
  for _var in BUCKET AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_ENDPOINT_URL; do
    eval "_val=\${$_var:-}"
    if [ -z "$_val" ]; then
      echo "ERROR: $_var is not set." >&2
      _missing=true
    fi
  done

  if $_missing; then
    cat >&2 <<'EOF'

Create an R2 API token with Object Read & Write, then export:
  BUCKET=<bucket-name>
  AWS_ACCESS_KEY_ID=<token-id>
  AWS_SECRET_ACCESS_KEY=<token-secret>
  AWS_ENDPOINT_URL=https://<account-id>.r2.cloudflarestorage.com
EOF
    exit 1
  fi

  if ! command -v rclone >/dev/null 2>&1; then
    echo "ERROR: rclone not found. Install it from https://rclone.org/install/" >&2
    exit 1
  fi
}

# ── Mirror ──────────────────────────────────────────────────────────────

check_env

# rclone wants the endpoint without a trailing slash.
_endpoint="${AWS_ENDPOINT_URL%/}"

for _dat in $DATS; do
  _src="$ROOT_DIR/export/$_dat"

  if [ ! -d "$_src" ]; then
    echo "SKIP: $_dat -- $_src not found"
    continue
  fi

  # Never sync --delete from an empty source: it would wipe the prefix.
  _count="$(find "$_src" -type f -name '*.bin' | wc -l | tr -d ' ')"
  if [ "$_count" -eq 0 ]; then
    echo "SKIP: $_dat -- no .bin files in $_src"
    continue
  fi

  _extra="--progress"
  if [ "${DRYRUN:-0}" = 1 ]; then
    _extra="--dry-run --progress"
  fi

  echo "--- $_dat ($_count files) -> s3://$BUCKET/$_dat/ ---"
  rclone sync "$_src" ":s3:$BUCKET/$_dat" \
    --s3-provider Cloudflare \
    --s3-endpoint "$_endpoint" \
    --s3-env-auth \
    --s3-region auto \
    --s3-no-check-bucket \
    --transfers 32 \
    --checkers 64 \
    --fast-list \
    $_extra
  echo ""
done

echo "Done. Deploy the worker with: npx wrangler deploy"
