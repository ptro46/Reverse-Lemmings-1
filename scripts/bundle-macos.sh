#!/bin/bash
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_root"
cargo build --release --locked
bundle_path="${1:-$project_root/dist/Lemmings Rust.app}"
if [[ -e "$bundle_path" ]]; then
  echo "Destination déjà présente : $bundle_path" >&2
  exit 1
fi
mkdir -p "$bundle_path/Contents/MacOS" "$bundle_path/Contents/Resources/assets"
cp target/release/lemmings-amiga "$bundle_path/Contents/MacOS/lemmings-amiga"
for part in images audio music levels styles sprites; do
  cp -R "assets/$part" "$bundle_path/Contents/Resources/assets/$part"
done
cp assets/levels.json "$bundle_path/Contents/Resources/assets/"
cat > "$bundle_path/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>lemmings-amiga</string>
<key>CFBundleIdentifier</key><string>local.ptro.lemmings-rust</string>
<key>CFBundleName</key><string>Lemmings Rust</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.2</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>NSHumanReadableCopyright</key><string>Experimental Rust engine; original game assets from user's ADF disks.</string>
</dict></plist>
PLIST
codesign --force --deep --sign - "$bundle_path"
echo "$bundle_path"
