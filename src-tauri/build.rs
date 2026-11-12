fn main() {
    // Icons are embedded when the crate compiles (generate_context!), and
    // cargo cannot track that read — without this, `pnpm tauri icon` output
    // is ignored until an unrelated recompile. Fold icon metadata into a
    // rustc-env: any change re-runs this script AND recompiles the crate,
    // and `rerun-if-changed` is what makes the script re-run at all.
    let mut meta = String::new();
    if let Ok(entries) = std::fs::read_dir("icons") {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                meta.push_str(&format!(
                    "{:?}:{:?}:{:?};",
                    entry.file_name(),
                    m.len(),
                    m.modified()
                ));
            }
        }
    }
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rustc-env=VEYLORA_ICON_META={}", meta);
    tauri_build::build();
}
