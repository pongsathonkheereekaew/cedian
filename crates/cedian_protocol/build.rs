//! Generates cedian's Rust types from cedia's protocol schema at the pinned revision.

use std::{env, fs, path::PathBuf};

use anyhow::Context as _;

fn main() -> anyhow::Result<()> {
    let schema_path = "cedia/protocol.schema.json";
    println!("cargo:rerun-if-changed={schema_path}");
    let text = fs::read_to_string(schema_path).context("reading cedia's protocol schema")?;
    let schema: schemars::schema::RootSchema =
        serde_json::from_str(&text).context("parsing cedia's protocol schema")?;

    let mut settings = typify::TypeSpaceSettings::default();
    settings.with_struct_builder(false);
    let mut space = typify::TypeSpace::new(&settings);
    space
        .add_root_schema(schema)
        .context("turning cedia's schema into Rust types")?;
    let file = syn::parse2::<syn::File>(space.to_stream()).context("parsing generated types")?;

    let out = PathBuf::from(env::var("OUT_DIR")?).join("protocol.rs");
    fs::write(&out, prettyplease::unparse(&file)).context("writing generated types")?;
    Ok(())
}
