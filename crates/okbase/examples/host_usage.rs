//! The library calls shown in docs/usage.md ("Applications").
//! `cargo run -p okbase --example host_usage -- <bundle>`

use std::path::Path;

use okbase::{Bundle, CatalogOptions, GrepRequest, OpenOptions, Scope};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    // Open once at startup; Bundle is cheap to clone and safe to share between threads.
    let bundle = Bundle::open(Path::new(&dir), OpenOptions::default())?;
    bundle.sync()?; // incremental: call again after files change

    // The host decides what each user may read; okbase only enforces it.
    let scope = Scope::all().deny("internal/**")?;

    // 1. Catalog for the system prompt.
    let catalog = bundle.catalog(&CatalogOptions::default(), &scope)?;
    println!("catalog: {} docs, ~{} tokens", catalog.docs, catalog.tokens);

    // 2. A tool call from the model.
    let hits = bundle.grep(
        &GrepRequest {
            pattern: "refund|đổi trả".into(),
            files_only: true,
            ..Default::default()
        },
        &scope,
    )?;
    println!("grep: {} documents", hits.total_docs);

    // 3. Which setup fits this bundle.
    let advice = bundle.advise(&okbase::AdviseOptions::default(), &scope)?;
    print!("{}", advice.to_text());
    Ok(())
}
