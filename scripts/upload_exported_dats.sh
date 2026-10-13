#!/bin/sh
# upload_exported_dats.sh — Upload pre-extracted DAT files to R2
#
# Uploads flattened .bin files (exported by asheron-rs `dat export --raw`)
# to the treestats-acdats-extracted R2 bucket. Files land at keys like
#   portal/0x06006957.bin   cell/0x01000001.bin   local_english/0x21000001.bin
#
# Prerequisites
# --------------
#   1. R2 S3-compatible API Token (Cloudflare Dashboard -> R2 -> API Tokens)
#   2. aws CLI installed (https://aws.amazon.com/cli/)
#   3. Exported .bin files in the expected directories
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
# Default: uploads all DATs when run with no env vars.

set -eu

BUCKET="treestats-acdats-extracted"
SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# All known DATs — name mapped to export directory basename
KNOWN_DATS="portal cell highres local_english"

# ── Helpers ─────────────────────────────────────────────────────────────

# Return the uppercased env-var name for a DAT (e.g. portal -> UPLOAD_PORTAL)
dat_upper() {
  echo "UPLOAD_$(echo "$1" | tr a-z A-Z)"
}

# Check whether a single DAT name is selected by the user's env vars.
# Selects when UPLOAD_DEFAULT=1 OR when the per-DAT flag (UPLOAD_PORTAL, etc.) is 1.
# If UPLOAD_DEFAULT=1 and any per-DAT flag is also set, per-DAT wins (default is
# suppressed) — meaning only the explicitly flagged DATs are selected.
# shellcheck disable=SC3043
dat_is_selected() {
  _name="$1"

  # Gather all per-DAT toggle states into a single pass so we can determine
  # whether any per-DAT flag is set at all.
  _found_individual=false
  for _d in $KNOWN_DATS; do
    _uvarname="$(dat_upper "$_d")"
    eval "_val=\${$_uvarname:-}"
    if [ "$_val" = 1 ]; then
      _found_individual=true
      break
    fi
  done

  # Now test the specific DAT
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

# ── Upload ──────────────────────────────────────────────────────────────

upload_dat() {
  _name="$1"
  _export_dir="$SCRIPT_DIR/export-$_name"

  if [ ! -d "$_export_dir" ]; then
    echo "SKIP: $_name -- export directory not found at $_export_dir"
    echo "      Run the asheron-rs export first:"
    echo "        cargo run --bin dat --features dat-export,dat-tokio -- export \\"
    echo "          --file client_${_name}.dat --output $_export_dir --raw"
    return
  fi

  _file_count="$(find "$_export_dir" -name '*.bin' | wc -l | tr -d ' ')"

  if [ "$_file_count" -eq 0 ]; then
    echo "SKIP: $_name -- no .bin files found in $_export_dir"
    return
  fi

  echo "--- Uploading $_name ($_file_count files) ---"
  echo "  Source: $_export_dir"
  echo "  Target: s3://$BUCKET/$_name/"

  _upload_aws() {
    _src="$1"
    _dest="s3://$BUCKET/$_name/"
    aws s3 cp "$_src" "$_dest" \
      --endpoint-url "$AWS_ENDPOINT_URL" \
      --content-type application/octet-stream \
      --quiet
  }

  # BATCH 1: files at the export root (setups, envcells, graphics objects, etc.)
  _root_files="$(find "$_export_dir" -maxdepth 1 -name '*.bin' | wc -l | tr -d ' ')"
  if [ "$_root_files" -gt 0 ]; then
    echo "  -> Uploading $_root_files root files"
    find "$_export_dir" -maxdepth 1 -name '*.bin' -exec aws s3 cp {} \
      "s3://$BUCKET/$_name/" \
      --endpoint-url "$AWS_ENDPOINT_URL" \
      --content-type application/octet-stream \
      --quiet \;
    echo "  [v] Root files done"
  fi

  # BATCH 2: files in type subdirectories (Texture/, CharGen/, SpellTable/)
  for _subdir in Texture CharGen SpellTable; do
    _subdir_path="$_export_dir/$_subdir"
    if [ -d "$_subdir_path" ]; then
      _sub_count="$(find "$_subdir_path" -name '*.bin' | wc -l | tr -d ' ')"
      if [ "$_sub_count" -gt 0 ]; then
        echo "  -> Uploading $_sub_count $_subdir files"
        find "$_subdir_path" -name '*.bin' -exec aws s3 cp {} \
          "s3://$BUCKET/$_name/" \
          --endpoint-url "$AWS_ENDPOINT_URL" \
          --content-type application/octet-stream \
          --quiet \;
        echo "  [v] $_subdir done"
      fi
    fi
  done

  echo "[v] $_name upload complete"
  echo ""
}

# ── Main ────────────────────────────────────────────────────────────────

if [ -z "${_UPLOAD_SOURCED:-}" ]; then
  echo "========================================"
  echo "  R2 Extracted File Uploader"
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

  for _name in $_selected_dats; do
    upload_dat "$_name"
  done

  echo "========================================"
  echo "  All uploads complete."
  echo ""
  echo "  Verify a file:"
  echo "    curl -sI https://pub-c15f23d2578040129a6aca9f4f1a3a47.r2.dev/<dat>/<file>.bin"
  echo ""
  echo "  Then deploy the worker:"
  echo "    npx wrangler deploy"
  echo "========================================"
fi