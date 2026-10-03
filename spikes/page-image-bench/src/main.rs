//! S14: times `okbase_convert::page_image` on each sample (median of N runs) and writes the
//! exported image to `out/<sample>.okbase.<ext>`. Output: one JSON line per sample.
use std::time::Instant;

fn main() {
    let runs: usize = std::env::var("RUNS").ok().and_then(|r| r.parse().ok()).unwrap_or(20);
    let mut files: Vec<_> = std::fs::read_dir("samples")
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "pdf"))
        .collect();
    files.sort();
    std::fs::create_dir_all("out").unwrap();
    for f in files {
        let bytes = std::fs::read(&f).unwrap();
        let name = f.file_stem().unwrap().to_string_lossy().into_owned();
        let page = if name.contains("50pages") { 50 } else { 1 };
        let mut times = Vec::new();
        let mut img = None;
        for _ in 0..runs {
            let t = Instant::now();
            img = okbase_convert::page_image(&bytes, page);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        let (ext, size) = match &img {
            Some(i) => {
                std::fs::write(format!("out/{name}.okbase.{}", i.ext), &i.bytes).unwrap();
                (i.ext, i.bytes.len())
            }
            None => ("none", 0),
        };
        println!(
            "{{\"sample\":\"{name}\",\"tool\":\"okbase\",\"page\":{page},\"ms\":{:.2},\"ext\":\"{ext}\",\"bytes\":{size}}}",
            times[runs / 2]
        );
    }
}
