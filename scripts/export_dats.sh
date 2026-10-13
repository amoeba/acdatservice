#!/bin/sh
# export_dats.sh — Re-export all DATs from source to flat raw .bin files
#
# Requires the `dat` CLI from asheron-rs on PATH:
#   cargo install -q worker-build  # if needed
#   cargo +stable install asheron-rs --features dat-export,dat-tokio
#   # or: cargo build --bin dat --features dat-export,dat-tokio --release
#   #     and copy target/release/dat somewhere on your PATH
#
# Source DAT files are read from DATS_DIR.
# Exports land in DEST_DIR (default: the project root).
#
# Selection (any combination; defaults to all when run with no vars):
#   EXPORT_PORTAL=1               # client_portal.dat
#   EXPORT_CELL=1                 # client_cell_1.dat
#   EXPORT_HIGHRES=1              # client_highres.dat
#   EXPORT_LOCAL_ENGLISH=1        # client_local_English.dat

set -eu

# ── Paths (override via env) ────────────────────────────────────────────
: "${DATS_DIR:=""
: "${DEST_DIR:="$(cd "$(dirname "$0")/.." && pwd)"}"

# Source DAT filenames per type
dat_source_file() {
  case "$1" in
    portal)         echo "client_portal.dat" ;;
    cell)           echo "client_cell_1.dat" ;;
    highres)        echo "client_highres.dat" ;;
    local_english)  echo "client_local_English.dat" ;;
  esac
}

dat_export_dir() {
  echo "$DEST_DIR/export/-$1"
}

# ── Selection ───────────────────────────────────────────────────────────

dat_upper() {
  echo "EXPORT_$(echo "$1" | tr a-z A-Z)"
}

dat_is_selected() {
  _name="$1"

  # Check for individual EXPORT_* flags
  _found_individual=false
  for _d in portal cell highres local_english; do
    _uvarname="$(dat_upper "$_d")"
    eval "_val=\${$_uvarname:-}"
    if [ "$_val" = 1 ]; then
      _found_individual=true
      break
    fi
  done

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

# ── Single DAT export ───────────────────────────────────────────────────

get_expected_count() {
  _name="$1"
  _dat_file="$DATS_DIR/$(dat_source_file "$_name")"
  dat list --count "$_dat_file" 2>/dev/null || echo "0"
}

verify_dat() {
  _name="$1"
  _export_dir="$(dat_export_dir "$_name")"

  if [ ! -d "$_export_dir" ]; then
    echo "[ ] $_name -- no export directory at $_export_dir"
    return
  fi

  _got="$(find "$_export_dir" -name '*.bin' | wc -l | tr -d ' ')"

  # Expected count from the DAT itself (may fail if CLI is busy)
  _expected=$(dat list --count "$DATS_DIR/$(dat_source_file "$_name")" 2>/dev/null)
  if [ -z "$_expected" ] || [ "$_expected" -le 0 ] 2>/dev/null; then
    echo "[~] $_name -- $_got files on disk (expected count unavailable)"
  elif [ "$_expected" -eq "$_got" ]; then
    echo "[v] $_name -- $_got/$_expected files match"
  else
    echo "[!] $_name -- expected $_expected files, got $_got"
  fi
}

export_dat() {
  _name="$1"
  _dat_file="$DATS_DIR/$(dat_source_file "$_name")"
  _export_dir="$(dat_export_dir "$_name")"

  if [ ! -f "$_dat_file" ]; then
    echo "SKIP: $_name -- source not found at $_dat_file"
    return
  fi

  echo "--- Exporting $_name ---"
  echo "  Source: $_dat_file"
  echo "  Output: $_export_dir"

  _expected="$(get_expected_count "$_name")"
  echo "  Expected files: $_expected"

  rm -rf "$_export_dir"
  dat export --raw --file "$_dat_file" --output "$_export_dir"
  _export_exit=$?

  # Clean up empty type subdirectories the CLI may have left
  for _subdir in Texture CharGen SpellTable; do
    _path="$_export_dir/$_subdir"
    if [ -d "$_path" ]; then
      rmdir "$_path" 2>/dev/null || rm -rf "$_path"
    fi
  done

  _got="$(find "$_export_dir" -name '*.bin' | wc -l | tr -d ' ')"
  if [ "$_export_exit" -ne 0 ]; then
    echo "[!] $_name -- export command failed (exit $_export_exit), got $_got/$_expected files"
  elif [ "$_expected" -eq "$_got" ]; then
    echo "[v] $_name export complete ($_got/$_expected files)"
  else
    echo "[!] $_name -- count mismatch: expected $_expected, got $_got files"
  fi
  echo ""
}

# ── Main ────────────────────────────────────────────────────────────────

# Guard -- skip main when sourced for testing
if [ -z "${_EXPORT_DATS_SOURCED:-}" ]; then
  if ! command -v dat >/dev/null 2>&1; then
    echo "ERROR: 'dat' CLI not found on PATH."
    echo ""
    echo "Install from asheron-rs:"
    echo "  git clone https://github.com/amoeba/asheron-rs"
    echo "  cd asheron-rs"
    echo "  cargo build --bin dat --features dat-export,dat-tokio --release"
    echo "  cp target/release/dat ~/.local/bin/  # or somewhere on your PATH"
    exit 1
  fi

  echo "================================================"
  echo "  asheron-rs DAT Exporter"
  echo "  Source: $DATS_DIR"
  echo "  Dest:   $DEST_DIR"
  echo "================================================"
  echo ""

  # Determine which DATs to operate on
  _selected_dats=""
  for _name in portal cell highres local_english; do
    if dat_is_selected "$_name"; then
      _selected_dats="$_selected_dats $_name"
    fi
  done

  if [ "${VERIFY:-0}" = 1 ]; then
    echo "Mode: VERIFY (checking existing exports)"
    echo "DATs to verify:$_selected_dats"
    echo ""
    for _name in $_selected_dats; do
      verify_dat "$_name"
    done
  elif [ "${EXPORT_DEFAULT:-1}" = 0 ]; then
    echo "EXPORT_DEFAULT=0: no exports requested."
  else
    echo "DATs to export:$_selected_dats"
    echo ""
    for _name in $_selected_dats; do
      export_dat "$_name"
    done
  fi

  echo "================================================"
  if [ "${VERIFY:-0}" = 1 ]; then
    echo "  Verification complete."
  else
    echo "  All exports complete."
  fi
  echo ""
  echo "  Next step: upload to R2"
  echo "    UPLOAD_DEFAULT=1 ./scripts/upload_exported_dats.sh"
  echo "================================================"
fi
