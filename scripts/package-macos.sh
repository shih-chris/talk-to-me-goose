#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/dist/Talk To Me Goose.app"
CONTENTS="$APP/Contents"

cargo build --release --manifest-path "$ROOT/Cargo.toml"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"
cp "$ROOT/target/release/talk-to-me-goose" "$CONTENTS/MacOS/talk-to-me-goose"
cp "$ROOT/Info.plist" "$CONTENTS/Info.plist"

if [ -n "${CODESIGN_IDENTITY:-}" ]; then
    codesign --force --options runtime --sign "$CODESIGN_IDENTITY" "$APP"
else
    codesign --force --deep --sign - "$APP"
fi

printf 'Packaged %s\n' "$APP"
