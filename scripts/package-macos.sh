#!/usr/bin/env -S bash -e

APP_BUNDLE_PATH="${APP_BUNDLE_PATH:?APP_BUNDLE_PATH not set}"
DMG_NAME="${DMG_NAME:?DMG_NAME not set}"
DMG_DIR="${DMG_DIR:?DMG_DIR not set}"

VOLUME_NAME="Rustcast"
STAGING_DIR="$DMG_DIR/dmg-staging"

rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"

cp -R "$APP_BUNDLE_PATH" "$STAGING_DIR/"
ln -s /Applications "$STAGING_DIR/Applications"

rm -f "$DMG_DIR/$DMG_NAME"

hdiutil create -volname "$VOLUME_NAME" \
  -srcfolder "$STAGING_DIR" \
  -ov -format UDZO \
  "$DMG_DIR/$DMG_NAME"

DMG_PATH="$DMG_DIR/$DMG_NAME"
echo "Created DMG at $DMG_PATH"
echo "DMG_PATH=$DMG_PATH" >> "$GITHUB_ENV"

# Notarize DMG (recommended: App Store Connect API key)
if [[ -n "$MACOS_NOTARY_KEY_ID" ]]; then
  echo "$MACOS_NOTARY_KEY" | base64 --decode > notary.key

  xcrun notarytool store-credentials "notarytool-profile" --apple-id "$MACOS_NOTARISATION_APPLE_ID" --team-id "$MACOS_NOTARY_TEAM_ID" --password "$MACOS_NOTARIZATION_PWD"

  xcrun notarytool submit "$DMG_PATH" \
    --key notary.key \
    --key-id "$MACOS_NOTARY_KEY_ID" \
    --issuer "$MACOS_NOTARY_ISSUER_ID" \
    --team-id "$MACOS_NOTARY_TEAM_ID" \
    --wait 

  echo "Waiting for ticket propagation..."
#   sleep 30

  xcrun stapler staple "$DMG_PATH"

  echo "Notarized and stapled DMG."
fi
