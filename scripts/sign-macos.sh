#!/usr/bin/env -S bash -e

ENTITLEMENTS_PATH="assets/macos/entitlements.plist"

APP_BUNDLE_PATH="${APP_BUNDLE_PATH:?APP_BUNDLE_PATH not set}"

# 1. Create a temporary keychain and import certificate
KEYCHAIN=build.keychain-db

if security list-keychains | grep -q "$KEYCHAIN"; then
  echo "Keychain $KEYCHAIN already exists, using existing keychain."
else
  security create-keychain -p "$MACOS_CI_KEYCHAIN_PWD" "$KEYCHAIN"
fi

security default-keychain -s "$KEYCHAIN"
security unlock-keychain -p "$MACOS_CI_KEYCHAIN_PWD" "$KEYCHAIN"
security set-keychain-settings "$KEYCHAIN"
security default-keychain -s "$KEYCHAIN"
security unlock-keychain -p "$MACOS_CI_KEYCHAIN_PWD" "$KEYCHAIN"
security set-keychain-settings "$KEYCHAIN"

echo "$MACOS_CERTIFICATE" | base64 --decode > certificate.p12
security import certificate.p12 \
  -k "$KEYCHAIN" \
  -P "$MACOS_CERTIFICATE_PWD" \
  -T /usr/bin/codesign

security set-key-partition-list -S apple-tool:,apple:,codesign: \
  -s -k "$MACOS_CI_KEYCHAIN_PWD" "$KEYCHAIN"

# 2. Sign app bundle
codesign --deep --force --options runtime --timestamp \
  --entitlements $ENTITLEMENTS_PATH \
  --sign "$MACOS_CERTIFICATE_NAME" \
  "$APP_BUNDLE_PATH"

codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE_PATH"
echo "Signed app at $APP_BUNDLE_PATH"
