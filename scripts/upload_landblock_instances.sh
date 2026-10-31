#!/bin/sh
# upload_landblock_instances.sh — Mirror the ACLI landblock-instances tree to R2
#
# Mirrors export/world/landblock-instances/ to
# s3://<bucket>/world/landblock-instances/ with `rclone sync` (Cloudflare R2
# provider), the rsync --delete equivalent: the bucket prefix becomes an exact
# mirror of the local tree, so objects that no longer exist locally are removed.
# The subdir mirrors the R2 prefix read by the
# `/dats/world/landblock-instances/:block` Worker route (see
# docs/landblock-instances-format.md).
#
# Required environment:
#   BUCKET                  destination R2 bucket name (no default)
#   AWS_ACCESS_KEY_ID       R2 S3 token id
#   AWS_SECRET_ACCESS_KEY   R2 S3 token secret
#   AWS_ENDPOINT_URL        https://<account-id>.r2.cloudflarestorage.com
#   PUBLISHER               rclone (default) or wrangler; wrangler needs only a
#                           `wrangler login` (or CLOUDFLARE_API_TOKEN) and no S3 keys
# The token needs Object Read & Write (ListBucket + DeleteObject).
#
# Requires rclone >= 1.59 (per Cloudflare's R2 docs).
#
# Options:
#   SRC_DIR                 local tree to mirror
#                           (default: <repo>/export/world/landblock-instances)
#   CLIENT_LAYOUT_URL       deployed ac-noclip marker
#                           (default: https://noclip.treestats.net/acli-layout.json)
#   REVISION                optional immutable dataset name, [A-Za-z0-9_-]{1,64}
#                           -> world/landblock-instances-revisions/$REVISION/
#   CONFIRM_LEGACY_FULL_TREE=1 required for legacy mode (no REVISION); never use
#                           a --block/--limit/partial export for the legacy tree
#   DRYRUN=1                validate and preview without making changes
#
# Revision mode reserves _publish.json before uploading and refuses reuse, even
# after an interrupted upload. Single writer only: Wrangler has no conditional
# put. Never manually write into this script-owned namespace. Wrangler also
# checks each destination object before writing; rclone requires an empty prefix.
# A failed revision must not be deployed. Choose a NEW revision for a new attempt.
#
# Before touching R2, scripts/check_acli_publish.mjs verifies that the deployed
# client advertises the current ACLI layout and that every local file uses
# it. This prevents publishing new records while an old client is live.
#
# Usage:
#   BUCKET=treestats-acdats-extracted REVISION=spawns-v1 ./scripts/upload_landblock_instances.sh
#   BUCKET=treestats-acdats-extracted REVISION=spawns-v1 DRYRUN=1 ./scripts/upload_landblock_instances.sh

set -eu
# POSIX bracket ranges are locale-sensitive (some locales put é inside a-z).
LC_ALL=C
export LC_ALL

BUCKET="${BUCKET:-}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="${SRC_DIR:-$ROOT_DIR/export/world/landblock-instances}"
# rclone needs R2 S3 keys; wrangler needs `wrangler login` (or an API token) and
# works when the S3 token has been rotated out.
PUBLISHER="${PUBLISHER:-rclone}"
WRANGLER="${WRANGLER:-npx wrangler}"
PARALLEL="${PARALLEL:-12}"
CLIENT_LAYOUT_URL="${CLIENT_LAYOUT_URL:-https://noclip.treestats.net/acli-layout.json}"
PREFIX="world/landblock-instances"
if [ "${REVISION+x}" = x ]; then
  if [ -z "$REVISION" ]; then
    echo "ERROR: REVISION must not be empty; unset it for legacy mode." >&2; exit 1
  fi
  case "$REVISION" in
    *[!a-zA-Z0-9_-]*) echo "ERROR: invalid REVISION (use 1-64 ASCII letters, digits, hyphens or underscores)." >&2; exit 1 ;;
  esac
  if [ "${#REVISION}" -gt 64 ]; then
    echo "ERROR: REVISION exceeds 64 characters." >&2; exit 1
  fi
  PREFIX="world/landblock-instances-revisions/$REVISION"
else
  REVISION=""
  if [ "${CONFIRM_LEGACY_FULL_TREE:-0}" != 1 ]; then
    echo "ERROR: legacy mode requires CONFIRM_LEGACY_FULL_TREE=1; never publish a partial tree here. Prefer REVISION=spawns-v1." >&2
    exit 1
  fi
fi
case "$PUBLISHER" in
  rclone|wrangler) ;;
  *) echo "ERROR: PUBLISHER must be rclone or wrangler." >&2; exit 1 ;;
esac

# ── Environment ─────────────────────────────────────────────────────────

check_env() {
  _required="BUCKET"
  if [ "$PUBLISHER" = rclone ]; then
    _required="BUCKET AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_ENDPOINT_URL"
  fi
  _missing=false
  for _var in $_required; do
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

  if [ "$PUBLISHER" = rclone ] && ! command -v rclone >/dev/null 2>&1; then
    echo "ERROR: rclone not found. Install it from https://rclone.org/install/" >&2
    exit 1
  fi

  if ! command -v node >/dev/null 2>&1; then
    echo "ERROR: node not found (required for the ACLI publish guard)." >&2
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

# Require a flat, canonical tree: both publishers must upload exactly the files
# checked by the guard, not nested files or auxiliary files rclone might mirror.
if find "$SRC_DIR" -mindepth 1 ! -type f | grep -q . ||
   find "$SRC_DIR" -type f | grep -Ev '/0x[0-9A-F]{4}\.bin$' | grep -q .; then
  echo "ERROR: expected a flat tree of canonical 0xNNNN.bin files only." >&2
  exit 1
fi

# Hard guard: do not publish a newer-layout tree until the deployed client says
# it can decode it, and refuse a mixed/legacy local tree.
node "$ROOT_DIR/scripts/check_acli_publish.mjs" \
  --src "$SRC_DIR" \
  --client-url "$CLIENT_LAYOUT_URL"

echo "--- $_count files -> s3://$BUCKET/$PREFIX/ (publisher: $PUBLISHER) ---"

# A reservation is deliberately permanent, even if the upload fails. Reusing a
# URL after a partial publication is unsafe with immutable caches.
_tmp="$(mktemp -d)"
trap 'rm -rf "$_tmp"' EXIT HUP INT TERM
if [ -n "$REVISION" ]; then
  printf '{"revision":"%s","files":%s}\n' "$REVISION" "$_count" >"$_tmp/reservation.json"
  if [ "$PUBLISHER" = wrangler ]; then
    if $WRANGLER r2 object get "$BUCKET/$PREFIX/_publish.json" --remote --file "$_tmp/existing" >"$_tmp/probe.log" 2>&1; then
      echo "ERROR: revision $REVISION is already reserved; use a new revision." >&2; exit 1
    elif ! grep -Fq 'The specified key does not exist.' "$_tmp/probe.log"; then
      echo "ERROR: cannot establish that revision is unused; refusing publication." >&2
      cat "$_tmp/probe.log" >&2; exit 1
    fi
    if [ "${DRYRUN:-0}" != 1 ]; then
      $WRANGLER r2 object put "$BUCKET/$PREFIX/_publish.json" --remote \
        --file "$_tmp/reservation.json" --content-type application/json
    fi
  else
    rclone lsf ":s3:$BUCKET/$PREFIX" --recursive \
      --s3-provider Cloudflare --s3-endpoint "${AWS_ENDPOINT_URL%/}" \
      --s3-env-auth --s3-region auto --s3-no-check-bucket >"$_tmp/list"
    if [ -s "$_tmp/list" ]; then
      echo "ERROR: revision prefix is not empty; use a new revision." >&2; exit 1
    fi
    if [ "${DRYRUN:-0}" != 1 ]; then
      rclone copyto "$_tmp/reservation.json" ":s3:$BUCKET/$PREFIX/_publish.json" \
        --immutable --s3-provider Cloudflare --s3-endpoint "${AWS_ENDPOINT_URL%/}" \
        --s3-env-auth --s3-region auto --s3-no-check-bucket
    fi
  fi
fi

if [ "$PUBLISHER" = wrangler ]; then
  # No R2 S3 token needed: uses the wrangler login (or CLOUDFLARE_API_TOKEN).
  # There is no --delete equivalent, so a block that disappears from the source
  # tree stays in R2; the ACLI block set is stable, so that is acceptable.
  export BUCKET PREFIX WRANGLER REVISION
  if [ "${DRYRUN:-0}" = 1 ]; then
    find "$SRC_DIR" -type f -name '*.bin' | head -5 | while read -r _file; do
      echo "  (dry run) put $PREFIX/$(basename "$_file")"
    done
    echo "  (dry run) $_count file(s) total"
  else
    find "$SRC_DIR" -type f -name '*.bin' -print0 |
      xargs -0 -n 1 -P "${PARALLEL:-12}" sh -c '
        if [ -n "$REVISION" ]; then
          _probe="$(mktemp -d)" || exit 255
          trap '\''rm -rf "$_probe"'\'' EXIT HUP INT TERM
          if $WRANGLER r2 object get "$BUCKET/$PREFIX/$(basename "$1")" \
               --remote --file "$_probe/existing" >"$_probe/log" 2>&1; then
            echo "ERROR: refusing to overwrite $PREFIX/$(basename "$1")" >&2; exit 255
          elif ! grep -Fq "The specified key does not exist." "$_probe/log"; then
            cat "$_probe/log" >&2; exit 255
          fi
        fi
        $WRANGLER r2 object put "$BUCKET/$PREFIX/$(basename "$1")" \
          --file "$1" --remote \
          --content-type application/octet-stream \
          --cache-control "public, max-age=31536000, immutable" >/dev/null || exit 255
      ' _
  fi
else
  _extra="--progress"
  if [ "${DRYRUN:-0}" = 1 ]; then
    _extra="--dry-run --progress"
  fi
  # rclone wants the endpoint without a trailing slash.
  _endpoint="${AWS_ENDPOINT_URL%/}"
  _operation=sync
  if [ -n "$REVISION" ]; then
    _operation=copy
    _extra="$_extra --immutable"
  fi
  rclone "$_operation" "$SRC_DIR" ":s3:$BUCKET/$PREFIX" \
    --s3-provider Cloudflare \
    --s3-endpoint "$_endpoint" \
    --s3-env-auth \
    --s3-region auto \
    --s3-no-check-bucket \
    --transfers 32 \
    --checkers 64 \
    --fast-list \
    $_extra
fi
echo ""

if [ "${DRYRUN:-0}" = 1 ]; then
  echo "Dry run complete. No objects written."
else
  echo "Publication complete at s3://$BUCKET/$PREFIX/"
  echo "Verify the complete revision before deploying service support, then the client."
fi
