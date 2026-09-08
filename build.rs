fn main() {
    println!("cargo:rerun-if-changed=assets/winshell.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/winshell.ico")
            .set("ProductName", "WinShell")
            .set("FileDescription", "WinShell — Native Windows Terminal")
            .set("OriginalFilename", "winshell.exe")
            .set("LegalCopyright", "Copyright 2026 WinShell contributors")
            .compile()
            .expect("Windows icon and version resource compilation failed");
    }
}
