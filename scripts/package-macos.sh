#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
case "$(uname -m)" in arm64) arch=arm64;; x86_64) arch=x64;; *) echo 'Unsupported architecture' >&2; exit 1;; esac
export MACOSX_DEPLOYMENT_TARGET=12.0
cargo build --release --locked
name="winshell-$version-macos-$arch"
package="dist/$name"
test ! -e "$package" || { echo "Package already exists: $package" >&2; exit 1; }
app="$package/WinShell.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/winshell "$app/Contents/MacOS/winshell"
cp README.md LICENSE THIRD_PARTY_NOTICES.md config.example.toml "$app/Contents/Resources/"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>WinShell</string>
<key>CFBundleDisplayName</key><string>WinShell</string>
<key>CFBundleIdentifier</key><string>com.neko233.winshell</string>
<key>CFBundleExecutable</key><string>winshell</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleLocalizations</key><array><string>en</string><string>zh_CN</string><string>zh_TW</string><string>ja</string><string>ko</string><string>de</string><string>fr</string><string>es</string></array>
</dict></plist>
EOF
# Ad-hoc signature ensures a consistent executable seal on Apple Silicon.
# This does not replace Developer ID signing or notarization.
codesign --force --sign - "$app"
codesign --verify --strict "$app"
"$app/Contents/MacOS/winshell" --doctor
"$app/Contents/MacOS/winshell" --smoke-test bash
tar -czf "dist/$name.tar.gz" -C dist "$name"
(cd dist && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
