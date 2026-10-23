//! The languages the Editor colours (Review and editing): Zed's bundled tree-sitter grammars,
//! with their configs and highlight queries from Zed's `grammars` crate, and no language
//! servers. A file's language is picked from its name as Zed picks it, and its colours come
//! from the theme, so they follow cedian Dark and cedian Light.

use std::{path::Path, pin::Pin, sync::Arc};

use futures::FutureExt as _;
use gpui::{App, Global};
use language::{Language, LanguageRegistry, LoadedLanguage};
use theme::ActiveTheme as _;

/// Every language Zed bundles a grammar for, by the name of its directory in `grammars`.
const NAMES: [&str; 23] = [
    "bash",
    "c",
    "cpp",
    "css",
    "diff",
    "env",
    "gitcommit",
    "go",
    "gomod",
    "gowork",
    "javascript",
    "jsdoc",
    "json",
    "jsonc",
    "markdown",
    "markdown-inline",
    "python",
    "regex",
    "rust",
    "tsx",
    "typescript",
    "yaml",
    "zed-keybind-context",
];

struct Languages(Arc<LanguageRegistry>);

impl Global for Languages {}

/// Builds the registry once, at start, and keeps it in step with the theme.
pub fn init(cx: &mut App) {
    let registry = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
    registry.register_native_grammars(grammars::native_grammars());
    for name in NAMES {
        let config = grammars::load_config(name);
        registry.register_language(
            config.name.clone(),
            config.grammar.clone(),
            config.matcher.clone(),
            config.hidden,
            None,
            Arc::new(move || {
                let config = config.clone();
                async move {
                    Ok(LoadedLanguage {
                        config,
                        queries: grammars::load_queries(name),
                        context_provider: None,
                        toolchain_provider: None,
                        manifest_name: None,
                    })
                }
                .boxed()
            }),
        );
    }
    registry.set_theme(cx.theme().clone());
    cx.observe_global::<theme::GlobalTheme>({
        let registry = registry.clone();
        move |cx| registry.set_theme(cx.theme().clone())
    })
    .detach();
    cx.set_global(Languages(registry));
}

/// The registry, for a buffer's injected languages (Markdown's code blocks).
pub fn registry(cx: &App) -> Option<Arc<LanguageRegistry>> {
    cx.try_global::<Languages>().map(|l| l.0.clone())
}

/// A language on its way from the registry.
pub type Loading = Pin<Box<dyn Future<Output = anyhow::Result<Arc<Language>>>>>;

/// Loads the language for `path` by its name, when there is one.
pub fn for_path(path: &Path, cx: &App) -> Option<Loading> {
    let registry = registry(cx)?;
    let path = path.to_owned();
    Some(Box::pin(async move {
        registry.load_language_for_file_path(&path).await
    }))
}

/// Loads a language by its name (`Diff` for Git's diffs).
pub fn named(name: &str, cx: &App) -> Option<Loading> {
    Some(Box::pin(registry(cx)?.language_for_name(name)))
}
