fn main() {
    #[cfg(windows)]
    {
        let ico = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packaging/windows/networkfix.ico");
        println!("cargo:rerun-if-changed={}", ico.display());
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico.to_str().expect("ico path utf-8"));
        if let Err(e) = res.compile() {
            // Keep builds working if the Windows SDK rc.exe is missing.
            println!("cargo:warning=winresource icon embed failed: {e}");
        }
    }
}
