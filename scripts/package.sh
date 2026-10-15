#!/usr/bin/env bash
# Azeroth Together's shareable build: `dist/AzerothTogether/` and `dist/AzerothTogether.zip`, to send
# a friend. It holds the game and its content files and NO Blizzard data: each player copies the
# `Data` folder from their own copy of AzerothSurvivors into the folder (see PLAYING.txt).
#
#   scripts/package.sh            # build (profile `share`, no `dev`) and package
#
# On Windows the build needs MinGW on PATH, as every build here does (CLAUDE.md).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --profile share -p azeroth-together --no-default-features

out=dist/AzerothTogether
rm -rf -- "${out:?}"
mkdir -p "$out"
exe=target/share/azeroth-together.exe
[ -f "$exe" ] || exe=target/share/azeroth-together
cp "$exe" "$out/"
cp -r content "$out/content"
cp scripts/PLAYING.txt "$out/PLAYING.txt"

rm -f dist/AzerothTogether.zip
if command -v powershell >/dev/null; then
    powershell -NoProfile -Command \
        "Compress-Archive -Path 'dist/AzerothTogether' -DestinationPath 'dist/AzerothTogether.zip'"
else
    (cd dist && zip -qr AzerothTogether.zip AzerothTogether)
fi
echo "packaged: $out and dist/AzerothTogether.zip"
