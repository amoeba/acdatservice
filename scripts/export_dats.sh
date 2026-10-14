#!/bin/sh
# export_dats.sh — Re-export all DATs from source to flat raw .bin files
#
# Requires the `dat` CLI from asheron-rs on PATH:
#   cargo install -q worker-build  # if needed
#   cargo +stable install asheron-rs --features dat-export,dat-tokio
#   # or: cargo build --bin dat --features dat-export,dat-tokio --release
#   #     and copy target/release/dat somewhere on your PATH
#
# Source DAT files are read from DATS_DIR (default: ~/Downloads/ac-updates).
# Exports land in DEST_DIR/export/<dat> (DEST_DIR defaults to the project root),
# e.g. export/highres/0x060043BE.bin. The script refuses to overwrite an
# existing export directory; remove it first to force a fresh export.
#
# Selection (any combination; defaults to all when run with no vars):
#   EXPORT_PORTAL=1               # client_portal.dat
#   EXPORT_CELL=1                 # client_cell_1.dat
#   EXPORT_HIGHRES=1              # client_highres.dat
#   EXPORT_LOCAL_ENGLISH=1        # client_local_English.dat
#
# Options:
#   EXPORT_DEFAULT=0              # don't export anything unless an explicit
#                                 # EXPORT_* flag is set
#   VERIFY=1                      # also compare exported .bin counts against
#                                 # the source DAT. With explicit EXPORT_* flags
#                                 # this runs after the export; on its own it
#                                 # only verifies existing exports.

set -eu

# ── Paths (override via env) ────────────────────────────────────────────
: "${DATS_DIR:="$HOME/Downloads/ac-updates"}"
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
  echo "$DEST_DIR/export/$1"
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

  # No individual flags -> default to all DATs. A bare VERIFY=1 checks every
  # existing export, regardless of EXPORT_DEFAULT (which only governs whether
  # we re-export).
  if ! $_found_individual; then
    if [ "${VERIFY:-0}" = 1 ] || [ "${EXPORT_DEFAULT:-1}" != 0 ]; then
      return 0
    fi
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
  _expected="$(dat list --count "$DATS_DIR/$(dat_source_file "$_name")" 2>/dev/null)" || _expected=""
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
    return 0
  fi

  # Never overwrite an existing export. This is a safety net; main reports the
  # conflict up front and exits before any export starts.
  if [ -e "$_export_dir" ]; then
    echo "ERROR: $_name -- output directory already exists: $_export_dir" >&2
    echo ""
    return 1
  fi

  echo "--- Exporting $_name ---"
  echo "  Source: $_dat_file"
  echo "  Output: $_export_dir"

  _expected="$(get_expected_count "$_name")"
  echo "  Expected files: $_expected"

  # `if ...; then` keeps `set -e` from aborting on a failed export so we can
  # report the exit code and file count below.
  if dat export --raw --file "$_dat_file" --output "$_export_dir"; then
    _export_exit=0
  else
    _export_exit=$?
  fi

  # Clean up empty type subdirectories the CLI may have left
  for _subdir in Texture CharGen SpellTable; do
    _path="$_export_dir/$_subdir"
    if [ -d "$_path" ]; then
      rmdir "$_path" 2>/dev/null || true
    fi
  done

  _got="$(find "$_export_dir" -name '*.bin' | wc -l | tr -d ' ')"
  if [ "$_export_exit" -ne 0 ]; then
    echo "[!] $_name -- export command failed (exit $_export_exit), got $_got/$_expected files"
    echo ""
    return 1
  elif [ "$_expected" -eq "$_got" ]; then
    echo "[v] $_name export complete ($_got/$_expected files)"
    echo ""
    return 0
  else
    echo "[!] $_name -- count mismatch: expected $_expected, got $_got files"
    echo ""
    return 1
  fi
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

  # Determine which DATs to operate on. `_found_individual` records whether
  # the user asked for specific DATs (via EXPORT_* flags).
  _selected_dats=""
  _found_individual=false
  for _name in portal cell highres local_english; do
    _uvarname="$(dat_upper "$_name")"
    eval "_val=\${$_uvarname:-}"
    if [ "$_val" = 1 ]; then
      _found_individual=true
    fi
    if dat_is_selected "$_name"; then
      _selected_dats="$_selected_dats $_name"
    fi
  done

  if [ -z "$_selected_dats" ]; then
    echo "No DATs selected. Set an EXPORT_* flag (e.g. EXPORT_HIGHRES=1)"
    echo "or leave EXPORT_DEFAULT unset/1 to export all."
    exit 1
  fi

  # VERIFY on its own (no explicit EXPORT_* flags) means verify-only; do not
  # re-export everything just to check it. With explicit flags, export first
  # and then verify.
  _do_export=1
  if [ "${VERIFY:-0}" = 1 ] && ! $_found_individual; then
    _do_export=0
  fi

  _export_failed=false
  if [ "$_do_export" = 1 ]; then
    # Pre-flight: refuse to start if any target directory already exists, so a
    # run can't partially export some DATs and leave stale ones behind.
    _conflict=false
    for _name in $_selected_dats; do
      _export_dir="$(dat_export_dir "$_name")"
      if [ -e "$_export_dir" ]; then
        echo "ERROR: $_name -- output directory already exists:" >&2
        echo "       $_export_dir" >&2
        _conflict=true
      fi
    done
    if $_conflict; then
      echo "" >&2
      echo "Refusing to overwrite existing exports. Remove them, then run again:" >&2
      for _name in $_selected_dats; do
        _export_dir="$(dat_export_dir "$_name")"
        if [ -e "$_export_dir" ]; then
          echo "  rm -r \"$_export_dir\"" >&2
        fi
      done
      exit 1
    fi

    echo "DATs to export:$_selected_dats"
    echo ""
    for _name in $_selected_dats; do
      if ! export_dat "$_name"; then
        _export_failed=true
      fi
    done
  fi

  if [ "${VERIFY:-0}" = 1 ]; then
    echo "Mode: VERIFY (checking exports)"
    echo "DATs to verify:$_selected_dats"
    echo ""
    for _name in $_selected_dats; do
      verify_dat "$_name"
    done
  fi

  echo "================================================"
  if [ "$_do_export" = 1 ]; then
    if $_export_failed; then
      echo "  Some exports failed. Remove the existing output directories"
      echo "  (see the errors above) and run again."
    else
      echo "  Exports complete."
    fi
  fi
  if [ "${VERIFY:-0}" = 1 ]; then
    echo "  Verification complete."
  fi
  echo ""
  if ! $_export_failed; then
    echo "  Next step: upload to R2"
    echo "    ./scripts/upload_exported_dats.sh"
  fi
  echo "================================================"

  if $_export_failed; then
    exit 1
  fi
fi
