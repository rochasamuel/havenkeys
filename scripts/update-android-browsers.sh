#!/usr/bin/env bash
# Refresh the privileged browser list Android Autofill and passkeys trust to
# report a page's domain (spec 2026-10-01-android-app §7.2, §8.1). This is the
# list Google's Credential Manager uses. Review the diff before committing.
set -euo pipefail
cd "$(dirname "$0")/.."
curl --fail --silent --show-error --proto '=https' \
  https://www.gstatic.com/gpm-passkeys-privileged-apps/apps.json \
  -o crates/havenkeys-core/data/android-browsers.json
