use anyhow::*;
use std::env;

fn main() -> Result<()> {
    // rerun this script if something in the assets folder changes
    println!("cargo:rerun-if-changed=assets/*");

    update_assets()?;

    Ok(())
}

/// Copy assets from assets directory to build output directory
fn update_assets() -> Result<()> {
    let out_dir = env::var("OUT_DIR")?;

    let path_contents_to_copy = ["assets/"];

    let mut copy_options = fs_extra::dir::CopyOptions::new();
    copy_options.overwrite = true;

    fs_extra::copy_items(&path_contents_to_copy, out_dir, &copy_options)?;

    Ok(())
}
