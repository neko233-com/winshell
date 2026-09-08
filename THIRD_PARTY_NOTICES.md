# Third-party software

WinShell is a terminal host. Its independent runtime distribution contains
separately licensed programs; the WinShell license does not relicense them.

| Component | Project | License |
| --- | --- | --- |
| GPUI | https://github.com/zed-industries/zed/tree/main/crates/gpui | Apache-2.0 |
| Alacritty terminal library | https://github.com/alacritty/alacritty | Apache-2.0 |
| portable-pty | https://github.com/wezterm/wezterm | MIT |
| Git for Windows portable runtime | https://gitforwindows.org/ | Multiple, including GPL-2.0 and GPL-3.0 |

The portable runtime is the unmodified upstream PortableGit 2.55.0.5
distribution, with two additional text files identifying provenance and hash.
All upstream license, copyright, and source-reference files are kept in
`runtime/git`; do not remove them when redistributing the package.

Pinned upstream release and binary/source references:
https://github.com/git-for-windows/git/releases/tag/v2.55.0.windows.5

Git for Windows component source/build repositories:
https://github.com/git-for-windows

Exact package versions and corresponding source archive URLs and SHA256 values
are recorded in `runtime-package-versions.txt` and `runtime-sources.json` inside
Windows packages (under `docs/` in the repository). The complete set of source
packages, including upstream sources, patches, and build instructions, is also
provided as `winshell-runtime-sources-2.55.0.5.zip` in the same GitHub release.
These source archives are retained without modification and are independently
licensed. Extract each `.src.tar.gz` or `.src.tar.zst` to inspect its PKGBUILD,
source files, and patches; they are not needed to run WinShell.

Source package mirrors used by upstream:
https://repo.msys2.org/msys/sources/
https://repo.msys2.org/mingw/sources/
https://github.com/git-for-windows/pacman-repo

A ZIP containing WinShell alone uses no bundled Git for Windows runtime; build
it with `-WithoutRuntime`. macOS packages use the operating system's tools and
do not redistribute the Windows runtime.

Cargo.lock records exact Rust dependency versions. `RUST_LICENSES.txt` and
`RUST_DEPENDENCIES.json` accompany application packages and retain dependency
licenses and source references. Each Rust crate's complete source is available
at https://crates.io/crates/NAME/VERSION/download for its recorded name/version.

The installer includes Inno Setup's runtime and translations under their own
terms: https://github.com/jrsoftware/issrc/blob/is-6_6_1/license.txt
Translator credits are retained in `installer/languages` in the repository.
