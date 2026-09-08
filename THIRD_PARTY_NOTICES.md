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

Downstream publishers must meet the redistribution and corresponding-source
requirements of each bundled component. A ZIP containing WinShell alone uses
no bundled Git for Windows runtime; build it with `-WithoutRuntime`.

Cargo.lock records exact Rust dependency versions. Each Rust crate contains its
own license and notices in its published source package.
