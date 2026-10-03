//! The image of a scanned PDF page, so an agent that reads images can transcribe it without a
//! PDF renderer. A scan is usually one image covering the page: JPEG data is handed over as is,
//! raw (Flate) pixels and CCITT Group 4 fax data (black-and-white office scanners) become a PNG.
//! Group 3 fax and JBIG2 are not decoded (the agent opens the PDF page instead).

use std::io::{Read, Write};

/// An exported page image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageImage {
    /// File extension: `jpg` or `png`.
    pub ext: &'static str,
    /// The image file.
    pub bytes: Vec<u8>,
}

/// The largest image on 1-based `page` of the PDF in `pdf`, when it can be exported.
pub fn page_image(pdf: &[u8], page: u32) -> Option<PageImage> {
    let doc = lopdf::Document::load_mem(pdf).ok()?;
    let page_id = *doc.get_pages().get(&page)?;
    let images = doc.get_page_images(page_id).ok()?;
    let img = images.iter().max_by_key(|i| i.width * i.height)?;
    let filters = img.filters.clone().unwrap_or_default();
    if filters.iter().any(|f| f == "DCTDecode") {
        return Some(PageImage {
            ext: "jpg",
            bytes: img.content.to_vec(),
        });
    }
    let (w, h) = (
        usize::try_from(img.width).ok()?,
        usize::try_from(img.height).ok()?,
    );
    // DecodeParms is a dictionary, or an array with one per filter (only one filter is decoded).
    let parms = img.origin_dict.get(b"DecodeParms").ok().and_then(|d| {
        d.as_dict()
            .ok()
            .or_else(|| d.as_array().ok()?.first()?.as_dict().ok())
    });
    let int = |key: &[u8]| {
        parms
            .and_then(|d| d.get(key).ok())
            .and_then(|p| p.as_i64().ok())
    };
    // `/Decode [1 0]` swaps black and white.
    let inverted = img
        .origin_dict
        .get(b"Decode")
        .ok()
        .and_then(|d| d.as_array().ok())
        .and_then(|a| a.first())
        .and_then(|v| {
            v.as_float()
                .ok()
                .or_else(|| v.as_i64().ok().map(|i| i as f32))
        })
        .is_some_and(|first| first > 0.5);
    if let [f] = filters.as_slice()
        && f == "CCITTFaxDecode"
    {
        // Only Group 4 (K < 0), what scanners write; Group 3 is rare in PDFs.
        if int(b"K").unwrap_or(0) >= 0 {
            return None;
        }
        let black_is_1 = parms
            .and_then(|d| d.get(b"BlackIs1").ok())
            .and_then(|b| b.as_bool().ok())
            .unwrap_or(false);
        let columns = int(b"Columns")
            .and_then(|c| u32::try_from(c).ok())
            .unwrap_or(1728);
        let mut px = Vec::with_capacity(w * h);
        fax::decoder::decode_g4(
            img.content.iter().copied(),
            columns,
            u32::try_from(h).ok(),
            |line| {
                for c in fax::decoder::pels(line, columns).take(w) {
                    // The sample is 1 for black when BlackIs1; DeviceGray shows 1 as white.
                    let one = (c == fax::Color::Black) == black_is_1;
                    px.push(if one != inverted { 255 } else { 0 });
                }
            },
        )?;
        if px.len() < w * h {
            return None;
        }
        return Some(PageImage {
            ext: "png",
            bytes: png(w, h, 1, &px[..w * h])?,
        });
    }
    let raw = match filters.as_slice() {
        [] => img.content.to_vec(),
        [f] if f == "FlateDecode" => {
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(img.content)
                .read_to_end(&mut out)
                .ok()?;
            out
        }
        _ => return None,
    };
    // Flate data often uses PNG predictors (DecodeParms /Predictor >= 10): every row starts with a
    // filter byte that has to be undone.
    let predictor = int(b"Predictor").unwrap_or(1);
    let bpc = usize::try_from(img.bits_per_component.unwrap_or(8)).ok()?;
    let colors = match img.color_space.as_deref() {
        Some("DeviceRGB") => 3,
        _ => 1,
    };
    let raw = if predictor >= 10 {
        unpredict(
            &raw,
            (w * colors * bpc).div_ceil(8),
            (colors * bpc).div_ceil(8).max(1),
        )?
    } else {
        raw
    };
    let channels = match img.color_space.as_deref() {
        Some("DeviceRGB") => 3,
        Some("DeviceGray") | None => 1,
        _ => return None,
    };
    let pixels = match img.bits_per_component.unwrap_or(8) {
        8 if raw.len() >= w * h * channels => raw[..w * h * channels].to_vec(),
        // 1-bit gray (black-and-white scans): rows are padded to whole bytes; 0 is black.
        1 if channels == 1 && raw.len() >= w.div_ceil(8) * h => {
            let stride = w.div_ceil(8);
            let mut px = Vec::with_capacity(w * h);
            for y in 0..h {
                for x in 0..w {
                    let bit = raw[y * stride + x / 8] >> (7 - (x % 8)) & 1;
                    px.push(if (bit == 1) != inverted { 255 } else { 0 });
                }
            }
            px
        }
        _ => return None,
    };
    Some(PageImage {
        ext: "png",
        bytes: png(w, h, channels, &pixels)?,
    })
}

/// Undoes PNG row filters (None, Sub, Up, Average, Paeth); `stride` bytes per row without the
/// filter byte, `bpp` bytes per pixel.
fn unpredict(data: &[u8], stride: usize, bpp: usize) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(data.len());
    let mut prev = vec![0u8; stride];
    for row in data.chunks(stride + 1) {
        if row.len() < stride + 1 {
            break;
        }
        let (kind, src) = (row[0], &row[1..]);
        let mut cur = vec![0u8; stride];
        for i in 0..stride {
            let left = if i >= bpp { cur[i - bpp] } else { 0 };
            let up = prev[i];
            let up_left = if i >= bpp { prev[i - bpp] } else { 0 };
            let pred = match kind {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => {
                    let p = i16::from(left) + i16::from(up) - i16::from(up_left);
                    let (pa, pb, pc) = (
                        (p - i16::from(left)).abs(),
                        (p - i16::from(up)).abs(),
                        (p - i16::from(up_left)).abs(),
                    );
                    if pa <= pb && pa <= pc {
                        left
                    } else if pb <= pc {
                        up
                    } else {
                        up_left
                    }
                }
                _ => return None,
            };
            cur[i] = src[i].wrapping_add(pred);
        }
        out.extend_from_slice(&cur);
        prev = cur;
    }
    Some(out)
}

/// A minimal PNG encoder (8-bit gray or RGB, no filtering).
fn png(w: usize, h: usize, channels: usize, pixels: &[u8]) -> Option<Vec<u8>> {
    let chunk = |out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
        let mut crc = crc32fast::Hasher::new();
        crc.update(kind);
        crc.update(data);
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc.finalize().to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&u32::try_from(w).ok()?.to_be_bytes());
    ihdr.extend_from_slice(&u32::try_from(h).ok()?.to_be_bytes());
    ihdr.extend_from_slice(&[8, if channels == 3 { 2 } else { 0 }, 0, 0, 0]);
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    for row in pixels.chunks(w * channels) {
        z.write_all(&[0]).ok()?;
        z.write_all(row).ok()?;
    }
    let idat = z.finish().ok()?;
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_predictors_are_undone() {
        // Two gray rows of 3 pixels: "Sub" then "Up".
        let data = [1, 10, 5, 5, 2, 1, 1, 1];
        assert_eq!(unpredict(&data, 3, 1).unwrap(), [10, 15, 20, 11, 16, 21]);
    }

    #[test]
    fn png_is_well_formed() {
        let p = png(2, 1, 1, &[0, 255]).unwrap();
        assert!(p.starts_with(b"\x89PNG\r\n\x1a\n") && p.ends_with(&[0xAE, 0x42, 0x60, 0x82]));
    }

    /// A one-page PDF whose page is the image stream `content` with extra dictionary entries.
    fn pdf_with_image(extra: lopdf::Dictionary, content: Vec<u8>, w: i64, h: i64) -> Vec<u8> {
        use lopdf::{Object, Stream, dictionary};
        let mut doc = lopdf::Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut dict = dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w, "Height" => h,
            "ColorSpace" => "DeviceGray", "BitsPerComponent" => 1,
        };
        dict.extend(&extra);
        let image = doc.add_object(Stream::new(dict, content));
        let contents = doc.add_object(Stream::new(
            dictionary! {},
            format!("q {w} 0 0 {h} 0 0 cm /Im0 Do Q").into_bytes(),
        ));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => contents,
            "MediaBox" => vec![0.into(), 0.into(), w.into(), h.into()],
            "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => image } },
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn ccitt_group4_scans_become_png() {
        use lopdf::{Object, dictionary};
        let (w, h) = (24usize, 6usize);
        let black = |x: usize, y: usize| (x + 2 * y).is_multiple_of(5);
        // Encodes the page with `black` runs coded as black (or as white when `swap`).
        let encode = |swap: bool| {
            let mut enc = fax::encoder::Encoder::new(fax::VecWriter::new());
            for y in 0..h {
                let line = (0..w).map(|x| {
                    if black(x, y) != swap {
                        fax::Color::Black
                    } else {
                        fax::Color::White
                    }
                });
                enc.encode_line(line, w as u32).unwrap();
            }
            enc.finish().unwrap().finish()
        };
        let page: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).map(move |x| if black(x, y) { 0 } else { 255 }))
            .collect();
        let inverted: Vec<u8> = page.iter().map(|p| 255 - p).collect();
        let parms = |black_is_1: bool| {
            dictionary! { "K" => -1, "Columns" => w as i64, "Rows" => h as i64, "BlackIs1" => black_is_1 }
        };
        let cases = [
            // The default: black runs are black.
            (
                dictionary! { "Filter" => "CCITTFaxDecode", "DecodeParms" => parms(false) },
                false,
                &page,
            ),
            // img2pdf: runs swapped and BlackIs1 true; DecodeParms as an array.
            (
                dictionary! {
                    "Filter" => vec![Object::from("CCITTFaxDecode")],
                    "DecodeParms" => vec![Object::from(parms(true))],
                },
                true,
                &page,
            ),
            // /Decode [1 0] inverts.
            (
                dictionary! {
                    "Filter" => "CCITTFaxDecode", "DecodeParms" => parms(false),
                    "Decode" => vec![1.into(), 0.into()],
                },
                false,
                &inverted,
            ),
        ];
        for (i, (extra, swap, expected)) in cases.into_iter().enumerate() {
            let pdf = pdf_with_image(extra, encode(swap), w as i64, h as i64);
            let img = page_image(&pdf, 1).unwrap_or_else(|| panic!("case {i}: no image"));
            assert_eq!(img.ext, "png");
            assert_eq!(img.bytes, png(w, h, 1, expected).unwrap(), "case {i}");
        }
        // Group 3 is not decoded.
        let g3 =
            dictionary! { "Filter" => "CCITTFaxDecode", "DecodeParms" => dictionary! { "K" => 0 } };
        assert!(page_image(&pdf_with_image(g3, vec![0; 8], 8, 1), 1).is_none());
    }

    #[test]
    fn scanned_sample_when_present() {
        let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../spikes/import-bench/samples/scanned-vi.pdf");
        let Ok(bytes) = std::fs::read(f) else { return };
        let img = page_image(&bytes, 1).expect("the scan's image");
        assert!(matches!(img.ext, "jpg" | "png"));
        assert!(img.bytes.len() > 1000);
        assert!(page_image(&bytes, 2).is_none());
    }
}
