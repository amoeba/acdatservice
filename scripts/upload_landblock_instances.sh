#!/bin/sh
# upload_landblock_instances.sh — Mirror the ACLI landblock-instances tree to R2
#
# Mirrors export/world/landblock-instances/ to
# s3://<bucket>/world/landblock-instances/ with `aws s3 sync --delete`, the
# rsync --delete equivalent: the bucket prefix becomes an exact mirror of the
# local tree, so objects that no longer exist locally are removed. The subdir
# mirrors the R2 prefix read by the `/dats/world/landblock-instances/:block`
# Worker route (see docs/landblock-instances-format.md).
#
# Required environment:
#   BUCKET                  destination R2 bucket name (no default)
#   AWS_ACCESS_KEY_ID       R2 S3 token id
#   AWS_SECRET_ACCESS_KEY   R2 S3 token secret
#   AWS_ENDPOINT_URL        https://<account-id>.r2.cloudflarestorage.com
# The token needs Object Read & Write (ListBucket + DeleteObject).
#
# Options:
#   SRC_DIR                 local tree to mirror
#                           (default: <repo>/export/world/landblock-instances)
#   AWS_DEFAULT_REGION      R2 region (default: auto)
#   DRYRUN=1                preview the sync without making changes (adds
#                           --dryrun to the aws command)
#
# Usage:
#   BUCKET=treestats-acdats-extracted ./scripts/upload_landblock_instances.sh
#   DRYRUN=1 ./scripts/upload_landblock_instances.sh   # preview without changes

set -eu

BUCKET="${BUCKET:-}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="${SRC_DIR:-$ROOT_DIR/export/world/landblock-instances}"
PREFIX="world/landblock-instances"

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

  if ! command -v aws >/dev/null 2>&1; then
    echo "ERROR: aws CLI not found. Install it from https://aws.amazon.com/cli/" >&2
    exit 1
  fi
}

# ── Mirror ──────────────────────────────────────────────────────────────

check_env

if [ ! -d "$SRC_DIR" ]; then
  echo "ERROR: source directory not found: $SRC_DIR" >&2
  echo "Generate it first with the ac-noclip tool:" >&2
  echo "  npx tsx scripts/gen-landblock-instances.ts --out=export" >&2
  exit 1
fi

# Never sync --delete from an empty source: it would wipe the prefix.
_count="$(find "$SRC_DIR" -type f -name '*.bin' | wc -l | tr -d ' ')"
if [ "$_count" -eq 0 ]; then
  echo "ERROR: no .bin files in $SRC_DIR" >&2
  echo "Refusing to sync --delete from an empty source (it would wipe the prefix)." >&2
  exit 1
fi

_extra=""
if [ "${DRYRUN:-0}" = 1 ]; then
  _extra="--dryrun"
fi

echo "--- $_count files -> s3://$BUCKET/$PREFIX/ ---"
aws s3 sync "$SRC_DIR" "s3://$BUCKET/$PREFIX/" \
  --delete \
  --content-type application/octet-stream \
  --only-show-errors \
  --endpoint-url "$AWS_ENDPOINT_URL" \
  --region "${AWS_DEFAULT_REGION:-auto}" \
  $_extra
echo ""

echo "Done. Deploy the worker with: npx wrangler deploy"
