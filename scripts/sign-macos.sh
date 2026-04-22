#!/bin/bash
set -euo pipefail

RELEASE_DIR="target/release"
APP_DIR="$RELEASE_DIR/macos"
APP_NAME="Rustcast.app"
APP_PATH="$APP_DIR/$APP_NAME"

# --- Required env vars ---
environment=(
  "MACOS_CERTIFICATE"
  "MACOS_CERTIFICATE_PWD"
  "MACOS_CI_KEYCHAIN_PWD"
  "MACOS_CERTIFICATE_NAME"
  "MACOS_NOTARY_TEAM_ID"
  "MACOS_NOTARY_KEY_ID"
  "MACOS_NOTARY_KEY"
  "MACOS_NOTARY_ISSUER_ID"
)

for var in "${environment[@]}"; do
  if [[ -z "${!var:-}" ]]; then
    echo "Error: $var is not set"
    exit 1
  fi
done

# --- Step 1: Decode the notarization API key FIRST ---
echo "Preparing notarization API key..."
NOTARY_KEY_FILE="AuthKey.p8"
if printf '%s' "$MACOS_NOTARY_KEY" | grep -q "BEGIN PRIVATE KEY"; then
  printf '%s' "$MACOS_NOTARY_KEY" > "$NOTARY_KEY_FILE"
else
  printf '%s' "$MACOS_NOTARY_KEY" | base64 --decode > "$NOTARY_KEY_FILE"
fi

# --- Step 2: Decode and install the signing certificate ---
echo "Decoding certificate..."
echo "$MACOS_CERTIFICATE" | base64 --decode > certificate.p12

echo "Installing cert in a new keychain..."
security create-keychain -p "$MACOS_CI_KEYCHAIN_PWD" build.keychain
security default-keychain -s build.keychain
security unlock-keychain -p "$MACOS_CI_KEYCHAIN_PWD" build.keychain
security import certificate.p12 -k build.keychain -P "$MACOS_CERTIFICATE_PWD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$MACOS_CI_KEYCHAIN_PWD" build.keychain

# --- Step 3: Sign the app ---
echo "Signing app..."
/usr/bin/codesign \
  --force \
  --deep \
  --options runtime \
  --timestamp \
  -s "$MACOS_CERTIFICATE_NAME" \
  -v \
  "$APP_PATH"

# --- Step 4: Verify the signature (not notarization yet) ---
echo "Verifying signature..."
/usr/bin/codesign --verify --deep --strict --verbose=2 "$APP_PATH"

# --- Step 5: Create notarization zip ---
echo "Creating notarization archive..."
ditto -c -k --keepParent "$APP_PATH" "notarization.zip"

# --- Step 6: Submit for notarization ---
echo "Submitting for notarization..."
SUBMIT_JSON=$(xcrun notarytool submit "notarization.zip" \
  --key "$NOTARY_KEY_FILE" \
  --key-id "$MACOS_NOTARY_KEY_ID" \
  --issuer "$MACOS_NOTARY_ISSUER_ID" \
  --output-format json)

echo "$SUBMIT_JSON"
SUBMIT_ID=$(echo "$SUBMIT_JSON" | jq -r .id)

if [[ -z "$SUBMIT_ID" || "$SUBMIT_ID" == "null" ]]; then
  echo "Error: Failed to get submission ID from notarytool"
  exit 1
fi

echo "Submission ID: $SUBMIT_ID"

# --- Step 7: Wait for notarization to complete ---
echo "Waiting for notarization result..."
WAIT_STATUS=0
xcrun notarytool wait "$SUBMIT_ID" \
  --key "$NOTARY_KEY_FILE" \
  --key-id "$MACOS_NOTARY_KEY_ID" \
  --issuer "$MACOS_NOTARY_ISSUER_ID" \
  --timeout 30m || WAIT_STATUS=$?

# --- Step 8: Fetch and print the notarization log ---
echo "Fetching notarization log..."
xcrun notarytool log "$SUBMIT_ID" \
  --key "$NOTARY_KEY_FILE" \
  --key-id "$MACOS_NOTARY_KEY_ID" \
  --issuer "$MACOS_NOTARY_ISSUER_ID" \
  notarization-log.json || true
cat notarization-log.json || true

if [[ $WAIT_STATUS -ne 0 ]]; then
  echo "Notarization did not succeed (wait exit code: $WAIT_STATUS)"
  exit $WAIT_STATUS
fi

# --- Step 9: Staple the notarization ticket ---
echo "Stapling notarization ticket..."
xcrun stapler staple "$APP_PATH"

# --- Step 10: Final Gatekeeper check (AFTER stapling) ---
echo "Running Gatekeeper assessment..."
spctl --assess --type execute --verbose "$APP_PATH"

echo "Done! App is signed, notarized, and stapled."
