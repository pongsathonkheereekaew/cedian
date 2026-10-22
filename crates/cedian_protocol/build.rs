//! Generates cedian's Rust types from cedia's protocol schema at the pinned revision.

use std::{env, fs, path::PathBuf};

use anyhow::Context as _;

fn main() -> anyhow::Result<()> {
    let schema_path = "cedia/protocol.schema.json";
    println!("cargo:rerun-if-changed={schema_path}");
    let text = fs::read_to_string(schema_path).context("reading cedia's protocol schema")?;
    let mut schema: serde_json::Value =
        serde_json::from_str(&text).context("parsing cedia's protocol schema")?;
    inline_union_refs(&mut schema);
    let schema: schemars::schema::RootSchema =
        serde_json::from_value(schema).context("reading cedia's protocol schema")?;

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

/// Replaces each `$ref` member of a definition's `anyOf` with the schema it names. typify
/// turns a union whose members all fix `type` to a constant into a tagged enum, but it cannot
/// see the constant behind a `$ref` and falls back to untagged `Variant0..n`.
fn inline_union_refs(schema: &mut serde_json::Value) {
    let Some(definitions) = schema.get("definitions").cloned() else {
        return;
    };
    let Some(defs) = schema
        .get_mut("definitions")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return;
    };
    for def in defs.values_mut() {
        let Some(members) = def
            .get_mut("anyOf")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        for member in members {
            let target = member
                .get("$ref")
                .and_then(serde_json::Value::as_str)
                .and_then(|r| r.strip_prefix("#/definitions/"))
                .and_then(|name| definitions.get(name));
            if let Some(target) = target {
                *member = target.clone();
            }
        }
    }
}
