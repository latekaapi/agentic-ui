//! Writes the generated gpui-kit theme set to `crates/aui-tokens/themes/agentic-ui.json`.
//!
//! ```bash
//! cargo run -p aui-tokens --example dump-theme
//! ```

use std::{fs, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("themes");
    fs::create_dir_all(&dir)?;
    let path = dir.join("agentic-ui.json");
    fs::write(&path, aui_tokens::theme_set_json())?;
    println!("wrote {}", path.display());
    Ok(())
}
