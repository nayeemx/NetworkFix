#!/usr/bin/env bash
set -euo pipefail

# Builds .deb + .AppImage via cargo-packager and .rpm via rpmbuild.
# Requires: Linux (or WSL2), rustup toolchain, network for cargo-packager
# linuxdeploy downloads and `cargo install cargo-packager` (if missing).
# Optional: rpm (apt install rpm) for .rpm; libfuse2 or libfuse2t64 if
# linuxdeploy AppImages fail to run without APPIMAGE_EXTRACT_AND_RUN.

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "error: Linux packaging must run on Linux (or WSL)" >&2
  exit 1
fi

# Repo root (script may be invoked from anywhere)
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

# linuxdeploy tools are AppImages; extract-and-run avoids FUSE requirements
export APPIMAGE_EXTRACT_AND_RUN=1

cargo build --release -p networkfix -p networkfix-gui

if ! command -v cargo-packager >/dev/null 2>&1; then
  cargo install cargo-packager --locked
fi

# rpm format is not supported by cargo-packager (deb + appimage only)
cargo packager -p networkfix-gui --release --formats appimage,deb

# --- .rpm via rpmbuild -------------------------------------------------
if command -v rpmbuild >/dev/null 2>&1; then
  version="$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2)"
  rpm_top="$(mktemp -d)"
  mkdir -p "$rpm_top"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

  cat > "$rpm_top/SPECS/networkfix.spec" <<EOF
Name:           networkfix
Version:        $version
Release:        1%{?dist}
Summary:        Hotspot and cellular network repair
License:        MIT
URL:            https://example.invalid/networkfix
AutoReqProv:    no
# Fedora package name; on other distros adjust manually:
# Requires:      webkit2gtk4.1

%description
NetworkFix repairs hotspot and cellular network connectivity issues.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/applications
mkdir -p %{buildroot}/usr/share/icons/hicolor/256x256/apps
install -m 755 "$repo_root/target/release/networkfix" %{buildroot}/usr/bin/networkfix
install -m 755 "$repo_root/target/release/networkfix-gui" %{buildroot}/usr/bin/networkfix-gui
sed -e 's/{{exec}}/networkfix-gui/g' -e 's/{{icon}}/networkfix-gui/g' \
  "$repo_root/packaging/linux/networkfix.desktop" > %{_topdir}/networkfix-gui.desktop
install -m 644 %{_topdir}/networkfix-gui.desktop %{buildroot}/usr/share/applications/networkfix-gui.desktop
install -m 644 "$repo_root/packaging/linux/icons/networkfix-256.png" %{buildroot}/usr/share/icons/hicolor/256x256/apps/networkfix-gui.png

%files
/usr/bin/networkfix
/usr/bin/networkfix-gui
/usr/share/applications/networkfix-gui.desktop
/usr/share/icons/hicolor/256x256/apps/networkfix-gui.png
EOF

  rpmbuild -bb --define "_topdir $rpm_top" "$rpm_top/SPECS/networkfix.spec"
  mkdir -p dist/linux
  find "$rpm_top/RPMS" -type f -name '*.rpm' -exec cp -f {} dist/linux/ \;
  rm -rf "$rpm_top"
else
  echo "warning: rpmbuild not found; skipping .rpm (apt install rpm)" >&2
fi

# --- collect artifacts ---------------------------------------------------
mkdir -p dist/linux
cp -f target/release/networkfix target/release/networkfix-gui dist/linux/
find target -maxdepth 4 \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -exec cp -f {} dist/linux/ \;
ls -la dist/linux
