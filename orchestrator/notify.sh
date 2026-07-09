#!/usr/bin/env bash
# Telegram notifier. Reads TELEGRAM_TOKEN and TELEGRAM_CHAT_ID from environment.
# Usage: notify.sh <level> <message>
#   level: info | milestone | alert
# Only "alert" is meant to interrupt. Everything else batches into digests.

set -uo pipefail

LEVEL="${1:?usage: notify.sh <info|milestone|alert> <message>}"
shift
MSG="$*"

: "${TELEGRAM_TOKEN:?TELEGRAM_TOKEN not set}"
: "${TELEGRAM_CHAT_ID:?TELEGRAM_CHAT_ID not set}"

case "$LEVEL" in
  info)      PREFIX="[CERULEA]" ; SILENT=true  ;;
  milestone) PREFIX="[CERULEA MILESTONE]" ; SILENT=true  ;;
  alert)     PREFIX="[CERULEA ACTION NEEDED]" ; SILENT=false ;;
  *)         PREFIX="[CERULEA]" ; SILENT=true ;;
esac

curl -sS -X POST "https://api.telegram.org/bot${TELEGRAM_TOKEN}/sendMessage" \
  --data-urlencode "chat_id=${TELEGRAM_CHAT_ID}" \
  --data-urlencode "text=${PREFIX}
${MSG}" \
  -d "disable_notification=${SILENT}" \
  -d "disable_web_page_preview=true" >/dev/null

exit 0
