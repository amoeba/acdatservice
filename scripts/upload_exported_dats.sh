#!/bin/sh
# upload_exported_dats.sh — Mirror pre-extracted DAT files to R2
#
# Mirrors local export/<dat>/ directories into the treestats-acdats-extracted
# R2 bucket with `aws s3 sync --delete` — the rsync --delete equivalent. The
# bucket prefix becomes an exact mirror of the local export, so objects that no
# longer exist locally are removed. Files land at keys like
#   portal/0x06006957.bin   cell/0x01000001.bin   highres/0x060043BE.bin
#
# Prerequisites
# --------------
#   1. R2 S3-compatible API Token with Object Read & Write
#      (--delete needs ListBucket + DeleteObject, not just read)
#   2. aws CLI installed (https://aws.amazon.com/cli/)
#   3. Exported .bin files under export/<dat>/ (see scripts/export_dats.sh)
#
# Selection (set any combination; default is all):
#   UPLOAD_PORTAL=1               # client_portal.dat
#   UPLOAD_CELL=1                 # client_cell_1.dat
#   UPLOAD_HIGHRES=1              # client_highres.dat
#   UPLOAD_LOCAL_ENGLISH=1        # client_local_English.dat
#
# Example: upload just portal and highres
#   UPLOAD_PORTAL=1 UPLOAD_HIGHRES=1 ./scripts/upload_exported_dats.sh
#
# Preview what would change without touching the bucket:
#   DRYRUN=1 ./scripts/upload_exported_dats.sh
#
# Default: uploads all DATs when run with no env vars.

set -eu

BUCKET="treestats-acdats-extracted"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# All known DATs — also the export directory basename and R2 prefix
KNOWN_DATS="portal cell highres local_english"

# ── Helpers ─────────────────────────────────────────────────────────────

# Return the uppercased env-var name for a DAT (e.g. portal -> UPLOAD_PORTAL)
dat_upper() {
  echo "UPLOAD_$(echo "$1" | tr a-z A-Z)"
}

# Check whether a single DAT name is selected by the user's env vars.
# Selects when the per-DAT flag (UPLOAD_PORTAL, etc.) is 1; when no per-DAT
# flag is set at all, defaults to all DATs.
dat_is_selected() {
  _name="$1"

  # Gather all per-DAT toggle states so we know whether any flag was set.
  _found_individual=false
  for _d in $KNOWN_DATS; do
    _uvarname="$(dat_upper "$_d")"
    eval "_val=\${$_uvarname:-}"
    if [ "$_val" = 1 ]; then
      _found_individual=true
      break
    fi
  done

  # Test the specific DAT
  _uvarname="$(dat_upper "$_name")"
  eval "_val=\${$_uvarname:-}"
  if [ "$_val" = 1 ]; then
    return 0
  fi

  # No individual flags -> default to all DATs
  if ! $_found_individual; then
    return 0
  fi

  return 1
}

# ── Validation ──────────────────────────────────────────────────────────

check_env() {
  _missing=false

  if [ -z "${AWS_ACCESS_KEY_ID:-}" ]; then
    echo "ERROR: AWS_ACCESS_KEY_ID is not set."
    _missing=true
  fi
  if [ -z "${AWS_SECRET_ACCESS_KEY:-}" ]; then
    echo "ERROR: AWS_SECRET_ACCESS_KEY is not set."
    _missing=true
  fi
  if [ -z "${AWS_ENDPOINT_URL:-}" ]; then
    echo "ERROR: AWS_ENDPOINT_URL is not set."
    _missing=true
  fi

  if $_missing; then
    echo ""
    echo "To get R2 S3 credentials:"
    echo "  1. Go to https://dash.cloudflare.com/ -> R2 -> treestats-acdats-extracted"
    echo "  2. Click 'API Tokens' and create a token with at least Object Read & Write"
    echo "  3. Export the three values:"
    echo ""
    echo "       export AWS_ACCESS_KEY_ID=\"<token-id>\""
    echo "       export AWS_SECRET_ACCESS_KEY=\"<token-secret>\""
    echo "       export AWS_ENDPOINT_URL=\"https://<account-id>.r2.cloudflarestorage.com\""
    echo ""
    echo "Then run this script again."
    exit 1
  fi

  if ! command -v aws >/dev/null 2>&1; then
    echo "ERROR: aws CLI not found. Install it from https://aws.amazon.com/cli/"
    exit 1
  fi
}

# ── Upload (mirror) ─────────────────────────────────────────────────────

sync_dat() {
  _name="$1"
  _export_dir="$SCRIPT_DIR/export/$_name"

  if [ ! -d "$_export_dir" ]; then
    echo "ERROR: $_name -- export directory not found at $_export_dir" >&2
    echo "       Run the export first:" >&2
    echo "         EXPORT_$(echo "$_name" | tr a-z A-Z)=1 ./scripts/export_dats.sh" >&2
    return 1
  fi

  # Never `sync --delete` from an empty source: it would wipe the prefix.
  _file_count="$(find "$_export_dir" -type f -name '*.bin' | wc -l | tr -d ' ')"
  if [ "$_file_count" -eq 0 ]; then
    echo "ERROR: $_name -- no .bin files found in $_export_dir" >&2
    return 1
  fi

  _dryrun=""
  _output_flag="--only-show-errors"
  if [ "${DRYRUN:-0}" = 1 ]; then
    _dryrun="--dryrun"
    # Keep the planned upload/delete lines visible in a preview.
    _output_flag=""
  fi

  echo "--- Mirroring $_name ($_file_count files) ---"
  echo "  Source: $_export_dir/"
  echo "  Target: s3://$BUCKET/$_name/  (--delete)"
  if [ -n "$_dryrun" ]; then
    echo "  Mode:   DRYRUN (no changes will be made)"
  fi
  echo ""

  # `if` keeps `set -e` from aborting so we can report the failing DAT and let
  # the remaining selected DATs continue, then exit non-zero at the end.
  if aws s3 sync "$_export_dir/" "s3://$BUCKET/$_name/" \
    --delete \
    --endpoint-url "$AWS_ENDPOINT_URL" \
    --region "${AWS_DEFAULT_REGION:-auto}" \
    --content-type application/octet-stream \
    $_output_flag \
    $_dryrun; then
    echo ""
    echo "[v] $_name mirror complete"
    return 0
  else
    _sync_exit=$?
    echo "" >&2
    echo "[!] $_name -- aws s3 sync failed (exit $_sync_exit)" >&2
    return 1
  fi
}

# ── Main ────────────────────────────────────────────────────────────────

if [ -z "${_UPLOAD_SOURCED:-}" ]; then
  echo "========================================"
  echo "  R2 Extracted File Mirror"
  echo "  Bucket: $BUCKET"
  echo "========================================"
  echo ""

  check_env
  echo "Using R2 endpoint: $AWS_ENDPOINT_URL"
  echo ""

  # Collect the list of selected DATs
  _selected_dats=""
  _selected_count=0

  for _name in $KNOWN_DATS; do
    if dat_is_selected "$_name"; then
      _selected_dats="$_selected_dats $_name"
      _selected_count=$((_selected_count + 1))
    fi
  done

  if [ "$_selected_count" -eq 0 ]; then
    echo "ERROR: No DATs selected. Set individual flags:" >&2
    echo "       UPLOAD_PORTAL=1  UPLOAD_CELL=1  UPLOAD_HIGHRES=1  UPLOAD_LOCAL_ENGLISH=1" >&2
    exit 1
  fi

  _failed=false
  for _name in $_selected_dats; do
    if ! sync_dat "$_name"; then
      _failed=true
    fi
  done

  echo "========================================"
  if $_failed; then
    echo "  Some uploads failed. See the errors above."
  elif [ "${DRYRUN:-0}" = 1 ]; then
    echo "  DRYRUN complete. No changes were made."
  else
    echo "  All uploads complete."
    echo ""
    echo "  Verify a file:"
    echo "    curl -sI https://pub-c15f23d2578040129a6aca9f4f1a3a47.r2.dev/<dat>/<file>.bin"
    echo ""
    echo "  Then deploy the worker:"
    echo "    npx wrangler deploy"
  fi
  echo "========================================"

  if $_failed; then
    exit 1
  fi
fi
