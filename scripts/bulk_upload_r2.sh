#!/usr/bin/env bash
# bulk_upload_r2.sh — Fast parallel upload of extracted files to R2
#
# Prerequisites:
#   1. R2 S3-compatible credentials in environment or ~/.aws/credentials
#      (Get these from your Cloudflare dashboard → R2 → API Tokens)
#   2. aws CLI installed
#      https://aws.amazon.com/cli/
#
# Usage:
#   export AWS_ACCESS_KEY_ID="<your-r2-access-key>"
#   export AWS_SECRET_ACCESS_KEY="<your-r2-secret-key>"
#   export AWS_ENDPOINT_URL="https://<account-id>.r2.cloudflarestorage.com"
#
#   ./scripts/bulk_upload_r2.sh export-portal   portal
#   ./scripts/bulk_upload_r2.sh export-cell     cell
#   ./scripts/bulk_upload_r2.sh export-local-english local_english

set -euo pipefail

EXPORT_DIR="$1"
DAT_NAME="$2"
BUCKET="treestats-acdats-extracted"

if [ -z "$EXPORT_DIR" ] || [ -z "$DAT_NAME" ]; then
  echo "Usage: $0 <export-dir> <dat-name>"
  exit 1
fi

if [ ! -d "$EXPORT_DIR" ]; then
  echo "ERROR: Export directory $EXPORT_DIR not found"
  exit 1
fi

if [ -z "${AWS_ENDPOINT_URL:-}" ]; then
  echo "ERROR: AWS_ENDPOINT_URL not set"
  echo "Get R2 S3 credentials from Cloudflare dashboard → R2 → API Tokens"
  exit 1
fi

echo "Uploading files from $EXPORT_DIR to s3://$BUCKET/$DAT_NAME/"

# Copy all .bin files (recursive flatten — strips Texture/, CharGen/, subdirs)
find "$EXPORT_DIR" -name '*.bin' -print0 | \
  xargs -0 -P 16 -I {} sh -c '
    base=$(basename "$1")
    aws s3 cp "$1" "s3://'"$BUCKET"'/'"$DAT_NAME"'/$base" \
      --endpoint-url "'"$AWS_ENDPOINT_URL"'" \
      --content-type application/octet-stream \
      --quiet
    echo "OK: $base"
  ' _ {}

echo "Done: $DAT_NAME uploaded"