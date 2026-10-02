//! Spike S10-lite: convert every sample with anydoc (office/PDF) or htmd (HTML), time it, write out/.
use std::time::Instant;

fn main() {
    std::fs::create_dir_all("out").unwrap();
    let mut files: Vec<_> = std::fs::read_dir("samples").unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let t = Instant::now();
        let r: Result<String, String> = if name.ends_with(".html") {
            let html = std::fs::read_to_string(&p).unwrap();
            htmd::HtmlToMarkdown::builder()
                .skip_tags(vec!["script", "style", "nav", "head"])
                .build()
                .convert(&html)
                .map_err(|e| e.to_string())
        } else {
            anydoc::to_markdown(&p).map_err(|e| format!("{e:?}"))
        };
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        match r {
            Ok(md) => {
                std::fs::write(format!("out/{name}.md"), &md).unwrap();
                println!("{name:<24} ok   {ms:>8.1} ms  {:>6} chars", md.chars().count());
            }
            Err(e) => println!("{name:<24} ERR  {ms:>8.1} ms  {}", &e[..e.len().min(120)]),
        }
    }
}
