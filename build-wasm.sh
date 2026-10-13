#!/usr/bin/env bash
# Builds the game for the browser into web/: the wasm, its JS glue and its
# assets, for a page with its canvas (`CANVAS` in src/lib.rs).
# `FAST=1` for quick builds while iterating.
set -euo pipefail

cd "$(dirname "$0")"

TARGET=wasm32-unknown-unknown
OUT_DIR=web
NAME=dostihy

PROFILE=${FAST:+wasm-fast}
PROFILE=${PROFILE:-wasm-release}

# wasm-bindgen has to be the very version the game was built with.
WANTED="$(tomlq -r '.package[] | select(.name == "wasm-bindgen") | .version' Cargo.lock)"
if [[ "$(wasm-bindgen --version)" != "wasm-bindgen $WANTED" ]]; then
    echo "needs wasm-bindgen $WANTED: cargo binstall wasm-bindgen-cli@$WANTED" >&2
    exit 1
fi

cargo build --profile "$PROFILE" --target "$TARGET" --bin "$NAME"

RAW="target/$TARGET/$PROFILE/$NAME.wasm"

echo ">>> wasm-bindgen"
BINDGEN_ARGS=(--target web --no-typescript --remove-producers-section)
if [[ -n ${FAST:-} ]]; then
    BINDGEN_ARGS+=(--keep-debug)
else
    BINDGEN_ARGS+=(--remove-name-section)
fi
wasm-bindgen "${BINDGEN_ARGS[@]}" \
    --out-dir "$OUT_DIR" \
    --out-name "$NAME" \
    "$RAW"

if [[ -z ${FAST:-} ]]; then
    echo ">>> wasm-opt"
    wasm-opt -Oz \
        --enable-bulk-memory \
        --enable-bulk-memory-opt \
        --enable-sign-ext \
        --enable-mutable-globals \
        --enable-nontrapping-float-to-int \
        --enable-multivalue \
        --enable-reference-types \
        --enable-extended-const \
        --strip-debug \
        --strip-dwarf \
        --strip-producers \
        --vacuum \
        -o "$OUT_DIR/${NAME}_bg.wasm" "$OUT_DIR/${NAME}_bg.wasm"
fi

# A browser has no system fonts to find, so the game brings Noto Sans (SIL OFL).
# Downloaded rather than kept in git; pinned and checked.
echo ">>> fonts"
FONTS="https://raw.githubusercontent.com/notofonts/notofonts.github.io/4e15001b9fb484cc871d7b2ad930cd2226edf260/fonts/NotoSans/hinted/ttf"
mkdir -p "$OUT_DIR/assets/fonts"
while read -r sum file; do
    path="$OUT_DIR/assets/fonts/$file"
    if ! echo "$sum  $path" | sha256sum --check --status 2>/dev/null; then
        curl -sSfL -o "$path" "$FONTS/$file"
        echo "$sum  $path" | sha256sum --check --quiet
    fi
done <<'EOF'
478c558ea716033cd60c03438f628dfa75694dcf6b5f6d505a2f05fd2b4f3823 NotoSans-Regular.ttf
1df075a380fc7cb898acf64c1f7b3b4dd780de3caa860178bf929de35817a913 NotoSans-Bold.ttf
EOF

# In a browser the original game's installer has to lie in the game's assets
# too, so the local copy goes along. Whatever hosts web/ then redistributes it.
if [[ -f assets/Dostihy3000setup.exe ]]; then
    cp assets/Dostihy3000setup.exe "$OUT_DIR/assets/"
    echo ">>> $OUT_DIR/assets/Dostihy3000setup.exe (from assets/)"
fi

echo ">>> $OUT_DIR/: $(du -h "$OUT_DIR/${NAME}_bg.wasm" | cut -f1) wasm"
