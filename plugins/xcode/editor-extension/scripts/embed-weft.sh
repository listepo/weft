#!/bin/sh
# Puts the artifact bundle's `weft` next to the extension's executable and signs it as a child of
# the sandbox. Apple's rule for a helper tool in a sandboxed app is exactly two entitlements,
# app-sandbox and inherit; with any other the system aborts the process. The extension runs the
# tool as its own child, so the tool takes the extension's sandbox instead of asking for its own.
# https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app
set -eu

bundle="$SRCROOT/../Artifacts/weft.artifactbundle"
tool=$(ls "$bundle"/weft-*-macosx/bin/weft 2>/dev/null | head -n 1 || true)
if [ -z "$tool" ]; then
  echo "error: no weft in $bundle; run 'moon run xcode-plugin:bundle'" >&2
  exit 1
fi

destination="$TARGET_BUILD_DIR/$EXECUTABLE_FOLDER_PATH/weft"
cp "$tool" "$destination"
codesign --force --sign "${EXPANDED_CODE_SIGN_IDENTITY:--}" --options runtime \
  --entitlements "$SRCROOT/scripts/helper.entitlements" "$destination"
