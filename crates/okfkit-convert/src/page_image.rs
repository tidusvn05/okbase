//! The image of a scanned PDF page, so an agent that reads images can transcribe it without a
//! PDF renderer. A scan is usually one image covering the page: JPEG data is handed over as is,
//! raw (Flate) pixels become a PNG. Fax and JBIG2 encodings are not decoded (the agent opens the
//! PDF page instead).

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
    let (w, h) = (
        usize::try_from(img.width).ok()?,
        usize::try_from(img.height).ok()?,
    );
    // Flate data often uses PNG predictors (DecodeParms /Predictor >= 10): every row starts with a
    // filter byte that has to be undone.
    let predictor = img
        .origin_dict
        .get(b"DecodeParms")
        .ok()
        .and_then(|d| d.as_dict().ok())
        .and_then(|d| d.get(b"Predictor").ok())
        .and_then(|p| p.as_i64().ok())
        .unwrap_or(1);
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
                    px.push(if bit == 1 { 255 } else { 0 });
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
