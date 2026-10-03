//! A real local model. Needs the model files, so it is ignored by default:
//! `OKBASE_MODELS_DIR=<dir with bge-m3-int8/> cargo test -p okbase-embed --features local -- --ignored`.
#![cfg(feature = "local")]

use okbase_embed::{Embedder, LocalEmbedder, dot};

#[test]
#[ignore = "downloads or needs a ~570 MB model"]
fn multilingual_query_finds_the_right_document() {
    let e = LocalEmbedder::load("bge-m3-int8", None).unwrap();
    let docs = vec![
        (
            "Chính sách đổi trả sản phẩm".to_owned(),
            "Khách hàng được đổi hoặc trả sản phẩm trong vòng 30 ngày kể từ ngày nhận hàng."
                .to_owned(),
        ),
        (
            "Partner API rate limits".to_owned(),
            "The partner REST API allows 600 requests per minute per API key.".to_owned(),
        ),
        (
            "ポイントの有効期限".to_owned(),
            "ヒカリポイントの有効期限は、最後にポイントを獲得または利用した日から1年間です。"
                .to_owned(),
        ),
    ];
    let d = e.embed_documents(&docs).unwrap();
    for (q, want) in [
        ("how many days do I have to return an item?", 0),
        ("API の呼び出し回数の上限は?", 1),
        ("điểm thưởng hết hạn khi nào", 2),
    ] {
        let qv = e.embed_queries(&[q.to_owned()]).unwrap().remove(0);
        let best = (0..d.len())
            .max_by(|&a, &b| dot(&qv, &d[a]).total_cmp(&dot(&qv, &d[b])))
            .unwrap();
        assert_eq!(best, want, "{q}");
    }
}
