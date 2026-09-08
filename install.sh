#!/bin/sh
# Usage: curl -fsSL https://raw.githubusercontent.com/neko233-com/winshell/main/install.sh | sh
# Pin:   curl -fsSL .../install.sh | sh -s -- 0.2.0
set -eu
test "$(uname -s)" = Darwin || { echo 'This installer supports macOS. On Windows use install.ps1.' >&2; exit 1; }
case "$(uname -m)" in arm64) arch=arm64;; x86_64) arch=x64;; *) echo 'Unsupported CPU architecture' >&2; exit 1;; esac
version=${1:-latest}
if test "$version" = latest; then
    metadata=$(curl --proto '=https' --tlsv1.2 -fsSL https://api.github.com/repos/neko233-com/winshell/releases/latest)
    version=$(printf '%s' "$metadata" | /usr/bin/plutil -extract tag_name raw -o - -)
fi
version=${version#v}
printf '%s\n' "$version" | /usr/bin/grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.-][a-zA-Z0-9.-]+)?$' || { echo 'Invalid release version' >&2; exit 1; }
name="winshell-$version-macos-$arch"
base="https://github.com/neko233-com/winshell/releases/download/v$version"
temporary=$(mktemp -d "${TMPDIR:-/tmp}/winshell-install.XXXXXXXX")
trap 'rm -rf "$temporary"' EXIT HUP INT TERM
echo "Downloading WinShell $version ($arch)..."
curl --proto '=https' --tlsv1.2 -fSL "$base/$name.tar.gz" -o "$temporary/$name.tar.gz"
curl --proto '=https' --tlsv1.2 -fsSL "$base/$name.tar.gz.sha256" -o "$temporary/checksum"
expected=$(awk 'NR == 1 { print $1 }' "$temporary/checksum")
printf '%s\n' "$expected" | grep -Eq '^[a-fA-F0-9]{64}$' || { echo 'Invalid checksum' >&2; exit 1; }
actual=$(shasum -a 256 "$temporary/$name.tar.gz" | awk '{ print $1 }')
test "$expected" = "$actual" || { echo 'SHA256 verification failed. Installation stopped.' >&2; exit 1; }
tar -tzf "$temporary/$name.tar.gz" > "$temporary/entries"
awk -v prefix="$name/" 'index($0, prefix) != 1 || $0 ~ /(^|\/)\.\.(\/|$)/ { exit 1 }' "$temporary/entries" || { echo 'Invalid archive layout' >&2; exit 1; }
tar -xzf "$temporary/$name.tar.gz" -C "$temporary"
source_app="$temporary/$name/WinShell.app"
test -x "$source_app/Contents/MacOS/winshell" || { echo 'Application is missing' >&2; exit 1; }
codesign --verify --strict "$source_app"
applications="$HOME/Applications"
destination="$applications/WinShell.app"
mkdir -p "$applications" "$HOME/.local/bin"
if test -e "$destination"; then
    identifier=$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$destination/Contents/Info.plist")
    test "$identifier" = com.neko233.winshell || { echo 'Destination belongs to another application' >&2; exit 1; }
    if pgrep -f "$destination/Contents/MacOS/winshell" >/dev/null; then
        echo 'Close WinShell before upgrading, then re-run the installer.' >&2; exit 1
    fi
fi
staged=$(mktemp -d "$applications/.winshell-stage.XXXXXXXX")
ditto "$source_app" "$staged/WinShell.app"
backup="$staged/previous.app"
if test -e "$destination"; then mv "$destination" "$backup"; fi
if ! mv "$staged/WinShell.app" "$destination"; then
    test ! -e "$backup" || mv "$backup" "$destination"
    echo 'Installation failed; previous version restored.' >&2; exit 1
fi
rm -rf "$staged"
launcher="$HOME/.local/bin/winshell"
if test -e "$launcher" && ! grep -q '# WinShell launcher' "$launcher"; then
    echo "Preserved existing command at $launcher"
else
    cat > "$launcher" <<'EOF'
#!/bin/sh
# WinShell launcher
exec "$HOME/Applications/WinShell.app/Contents/MacOS/winshell" "$@"
EOF
    chmod +x "$launcher"
fi
echo "Installed: $destination"
echo "Open WinShell in Finder, or run: $launcher"
echo 'This release uses an ad-hoc signature. Gatekeeper may require approval in System Settings > Privacy & Security.'
