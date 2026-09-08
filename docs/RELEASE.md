# Release procedure

1. Update the package version in `Cargo.toml` and regenerate `Cargo.lock`.
2. Run `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
   `cargo test --locked`, and `scripts/validate.ps1 -UI` on Windows.
3. Push the exact commit. `Build and validate` must pass on Windows x64, macOS
   Intel, and macOS Apple Silicon. The workflow exercises native windows and
   real pseudoterminals and tests the Windows installation and uninstallation.
4. Collect upstream source packages with `python scripts/collect-runtime-sources.py`.
   Resolve missing archives before distributing the bundled runtime. Preserve
   the manifest, package list, notices, and corresponding sources with the release.
5. Download the three successful build artifacts. Verify every SHA256 and tag
   the tested commit. Create a draft GitHub release with the portable ZIP,
   Windows setup EXE, two macOS application archives, all `.sha256` files, and
   corresponding sources. Publish only after the asset set is complete.
6. Test `install.ps1 -Version VERSION` on Windows and `sh install.sh VERSION` on
   macOS against the public release, including an upgrade run.

Windows builds are currently unsigned. macOS builds have an ad-hoc signature;
Developer ID signing and notarization require release credentials. Do not
disable Smart App Control, Gatekeeper, or execution policy in installers.
Use those credentials when available; do not describe an ad-hoc signature as
a notarized distribution.

The installer keeps settings outside the application directory. Uninstalling
does not delete the user's config, shell profiles, histories, or projects.
