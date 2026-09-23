use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbImage};

pub fn process_scanned_images(
    paths: &[String],
    remove_shadow: bool,
    correct_perspective: bool,
    dpi: u32,
) -> Result<Vec<u8>, String> {
    if paths.is_empty() {
        return Err("No images provided for scan processing".to_string());
    }

    let mut doc = lopdf::Document::with_version("1.7");
    let pages_id = doc.add_object(lopdf::Object::Dictionary(lopdf::Dictionary::new()));
    let mut kids = Vec::new();

    for path in paths {
        let img = image::open(path).map_err(|e| format!("Failed to open {path}: {e}"))?;

        let mut result = img;
        if correct_perspective {
            result = correct_perspective_simple(&result);
        }

        if remove_shadow {
            result = remove_shadow_simple(&result);
        }

        // Respect original image resolution or cap if excessively large for PDF embedding at given DPI
        // Avoid the bug of multiplying by (dpi / 150) which doubles 4000px images to 8000px
        let max_dim = 4096u32;
        if result.width() > max_dim || result.height() > max_dim {
            result = result.resize(max_dim, max_dim, FilterType::Lanczos3);
        }

        let mut rgb: RgbImage = result.to_rgb8();
        enhance_contrast(&mut rgb);

        let (width, height) = rgb.dimensions();
        let dynamic = DynamicImage::ImageRgb8(rgb);

        // Encode as JPEG for efficient PDF embedding
        let mut jpeg_buf = std::io::Cursor::new(Vec::new());
        dynamic
            .write_to(&mut jpeg_buf, image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to encode image to JPEG: {e}"))?;
        let jpeg_bytes = jpeg_buf.into_inner();

        let mut img_dict = lopdf::Dictionary::new();
        img_dict.set("Type", lopdf::Object::Name("XObject".into()));
        img_dict.set("Subtype", lopdf::Object::Name("Image".into()));
        img_dict.set("Width", lopdf::Object::Integer(width as i64));
        img_dict.set("Height", lopdf::Object::Integer(height as i64));
        img_dict.set("ColorSpace", lopdf::Object::Name("DeviceRGB".into()));
        img_dict.set("BitsPerComponent", lopdf::Object::Integer(8));
        img_dict.set("Filter", lopdf::Object::Name("DCTDecode".into()));

        let img_stream = lopdf::Stream::new(img_dict, jpeg_bytes);
        let img_id = doc.add_object(lopdf::Object::Stream(img_stream));

        let effective_dpi = dpi.max(72);
        let pt_w = (width as f64 * 72.0 / effective_dpi as f64) as f32;
        let pt_h = (height as f64 * 72.0 / effective_dpi as f64) as f32;

        let mut xobj_dict = lopdf::Dictionary::new();
        xobj_dict.set("Im1", lopdf::Object::Reference(img_id));
        let mut res_dict = lopdf::Dictionary::new();
        res_dict.set("XObject", lopdf::Object::Dictionary(xobj_dict));

        let content_stream = format!("q {pt_w:.2} 0 0 {pt_h:.2} 0 0 cm /Im1 Do Q");
        let content_id = doc.add_object(lopdf::Object::Stream(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            content_stream.into_bytes(),
        )));

        let mut page_dict = lopdf::Dictionary::new();
        page_dict.set("Type", lopdf::Object::Name("Page".into()));
        page_dict.set("Parent", lopdf::Object::Reference(pages_id));
        page_dict.set(
            "MediaBox",
            lopdf::Object::Array(vec![
                lopdf::Object::Real(0.0),
                lopdf::Object::Real(0.0),
                lopdf::Object::Real(pt_w),
                lopdf::Object::Real(pt_h),
            ]),
        );
        page_dict.set("Resources", lopdf::Object::Dictionary(res_dict));
        page_dict.set("Contents", lopdf::Object::Reference(content_id));

        let page_id = doc.add_object(lopdf::Object::Dictionary(page_dict));
        kids.push(lopdf::Object::Reference(page_id));
    }

    let mut pages_dict = lopdf::Dictionary::new();
    pages_dict.set("Type", lopdf::Object::Name("Pages".into()));
    pages_dict.set("Kids", lopdf::Object::Array(kids));
    pages_dict.set("Count", lopdf::Object::Integer(paths.len() as i64));
    doc.objects
        .insert(pages_id, lopdf::Object::Dictionary(pages_dict));

    let mut root_dict = lopdf::Dictionary::new();
    root_dict.set("Type", lopdf::Object::Name("Catalog".into()));
    root_dict.set("Pages", lopdf::Object::Reference(pages_id));
    let root_id = doc.add_object(lopdf::Object::Dictionary(root_dict));
    doc.trailer.set("Root", lopdf::Object::Reference(root_id));

    let mut buf = Vec::new();
    doc.save_to(&mut buf)
        .map_err(|e| format!("Failed to generate scanned PDF: {e}"))?;
    Ok(buf)
}

fn correct_perspective_simple(img: &DynamicImage) -> DynamicImage {
    let (w, h) = img.dimensions();
    let rgb = img.to_rgb8();

    let gray = to_grayscale(&rgb);
    let edges = sobel_edges(&gray, w, h);

    let corners = find_document_corners(&edges, w, h);

    if let Some((tl, tr, br, bl)) = corners {
        perspective_transform(img, tl, tr, br, bl)
    } else {
        img.clone()
    }
}

fn to_grayscale(rgb: &RgbImage) -> Vec<u8> {
    rgb.pixels()
        .map(|p| {
            let r = p[0] as u32;
            let g = p[1] as u32;
            let b = p[2] as u32;
            ((r * 299 + g * 587 + b * 114) / 1000) as u8
        })
        .collect()
}

fn sobel_edges(gray: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut edges = vec![0u8; (w * h) as usize];
    if w < 3 || h < 3 {
        return edges;
    }

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let idx = (y * w + x) as usize;
            let gx = -(gray[((y - 1) * w + x - 1) as usize] as i16)
                + gray[((y - 1) * w + x + 1) as usize] as i16
                - 2 * gray[(y * w + x - 1) as usize] as i16
                + 2 * gray[(y * w + x + 1) as usize] as i16
                - gray[((y + 1) * w + x - 1) as usize] as i16
                + gray[((y + 1) * w + x + 1) as usize] as i16;

            let gy = -(gray[((y - 1) * w + x - 1) as usize] as i16)
                - 2 * gray[((y - 1) * w + x) as usize] as i16
                - gray[((y - 1) * w + x + 1) as usize] as i16
                + gray[((y + 1) * w + x - 1) as usize] as i16
                + 2 * gray[((y + 1) * w + x) as usize] as i16
                + gray[((y + 1) * w + x + 1) as usize] as i16;

            let gx = gx as i32;
            let gy = gy as i32;
            let magnitude = ((gx * gx + gy * gy) as f64).sqrt().min(255.0) as u8;
            edges[idx] = if magnitude > 50 { 255 } else { 0 };
        }
    }
    edges
}

fn find_document_corners(
    edges: &[u8],
    w: u32,
    h: u32,
) -> Option<((f64, f64), (f64, f64), (f64, f64), (f64, f64))> {
    if w < 20 || h < 20 {
        return None;
    }

    let margin_x = (w / 15).max(1);
    let margin_y = (h / 15).max(1);

    // Collect candidate edge points away from extreme outer borders
    let mut edge_points = Vec::new();
    let step = 2.max(w / 400); // subsample for efficiency on high-res images
    for y in (margin_y..h - margin_y).step_by(step as usize) {
        for x in (margin_x..w - margin_x).step_by(step as usize) {
            if edges[(y * w + x) as usize] > 0 {
                edge_points.push((x as f64, y as f64));
            }
        }
    }

    if edge_points.len() < 40 {
        return None;
    }

    // In a document photo, the 4 corners of the quad correspond to:
    // Top-Left: minimizes (x + y)
    // Bottom-Right: maximizes (x + y)
    // Top-Right: maximizes (x - y)
    // Bottom-Left: minimizes (x - y)
    // We compute the 5% extremal percentiles to avoid single outlier noise pixels.
    let mut sum_xy: Vec<(f64, (f64, f64))> = edge_points.iter().map(|&(x, y)| (x + y, (x, y))).collect();
    sum_xy.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let mut diff_xy: Vec<(f64, (f64, f64))> = edge_points.iter().map(|&(x, y)| (x - y, (x, y))).collect();
    diff_xy.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let n = edge_points.len();
    let k = (n / 50).clamp(1, 15); // use 2% robust trimmed extremum

    let tl = sum_xy[k].1;
    let br = sum_xy[n - 1 - k].1;
    let bl = diff_xy[k].1;
    let tr = diff_xy[n - 1 - k].1;

    // Verify that the detected quadrilateral forms a reasonable document area (at least 20% of image width and height)
    let top_w = ((tr.0 - tl.0).powi(2) + (tr.1 - tl.1).powi(2)).sqrt();
    let bot_w = ((br.0 - bl.0).powi(2) + (br.1 - bl.1).powi(2)).sqrt();
    let left_h = ((bl.0 - tl.0).powi(2) + (bl.1 - tl.1).powi(2)).sqrt();
    let right_h = ((br.0 - tr.0).powi(2) + (br.1 - tr.1).powi(2)).sqrt();

    let min_dim_w = w as f64 * 0.20;
    let min_dim_h = h as f64 * 0.20;

    if top_w > min_dim_w && bot_w > min_dim_w && left_h > min_dim_h && right_h > min_dim_h {
        Some((tl, tr, br, bl))
    } else {
        None
    }
}

/// Compute 3x3 projective homography matrix mapping rectangle (0,0)-(W,H) back to quad (tl, tr, br, bl)
/// so we can perform true backward-mapping perspective warp with subpixel bilinear interpolation.
fn get_destination_to_source_homography(
    w: f64,
    h: f64,
    tl: (f64, f64),
    tr: (f64, f64),
    br: (f64, f64),
    bl: (f64, f64),
) -> Option<[f64; 9]> {
    // We map unit square [0,1]^2 -> quad (tl, tr, br, bl), then compose with scaling from [0,W]x[0,H] to [0,1]^2.
    // Let source points:
    // x0=tl.0, y0=tl.1
    // x1=tr.0, y1=tr.1
    // x2=br.0, y2=br.1
    // x3=bl.0, y3=bl.1
    let x0 = tl.0; let y0 = tl.1;
    let x1 = tr.0; let y1 = tr.1;
    let x2 = br.0; let y2 = br.1;
    let x3 = bl.0; let y3 = bl.1;

    let dx1 = x1 - x2;
    let dx2 = x3 - x2;
    let sx = x0 - x1 + x2 - x3;

    let dy1 = y1 - y2;
    let dy2 = y3 - y2;
    let sy = y0 - y1 + y2 - y3;

    let (h6, h7, h0, h1, h2, h3, h4, h5) = if sx.abs() < 1e-7 && sy.abs() < 1e-7 {
        // Affine case
        let a = x1 - x0;
        let b = x3 - x0;
        let c = x0;
        let d = y1 - y0;
        let e = y3 - y0;
        let f = y0;
        (0.0, 0.0, a, b, c, d, e, f)
    } else {
        let det = dx1 * dy2 - dy1 * dx2;
        if det.abs() < 1e-7 {
            return None;
        }
        let g = (sx * dy2 - sy * dx2) / det;
        let h = (dx1 * sy - dy1 * sx) / det;
        let a = x1 - x0 + g * x1;
        let b = x3 - x0 + h * x3;
        let c = x0;
        let d = y1 - y0 + g * y1;
        let e = y3 - y0 + h * y3;
        let f = y0;
        (g, h, a, b, c, d, e, f)
    };

    // Matrix M maps normalized coords (u, v) in [0, 1]^2 to (X, Y, Z) in source image
    // To map from pixel coords (x, y) in [0, W]x[0, H], we compose with u = x/W, v = y/H
    // M' = M * diag(1/W, 1/H, 1)
    let inv_w = 1.0 / w;
    let inv_h = 1.0 / h;

    Some([
        h0 * inv_w, h1 * inv_h, h2,
        h3 * inv_w, h4 * inv_h, h5,
        h6 * inv_w, h7 * inv_h, 1.0,
    ])
}

fn perspective_transform(
    img: &DynamicImage,
    tl: (f64, f64),
    tr: (f64, f64),
    br: (f64, f64),
    bl: (f64, f64),
) -> DynamicImage {
    let src = img.to_rgb8();
    let (sw, sh) = src.dimensions();

    // Destination dimensions based on true euclidean edge lengths of the document quad
    let top_w = ((tr.0 - tl.0).powi(2) + (tr.1 - tl.1).powi(2)).sqrt();
    let bot_w = ((br.0 - bl.0).powi(2) + (br.1 - bl.1).powi(2)).sqrt();
    let left_h = ((bl.0 - tl.0).powi(2) + (bl.1 - tl.1).powi(2)).sqrt();
    let right_h = ((br.0 - tr.0).powi(2) + (br.1 - tr.1).powi(2)).sqrt();

    let dst_w = top_w.max(bot_w).round() as u32;
    let dst_h = left_h.max(right_h).round() as u32;

    if dst_w == 0 || dst_h == 0 {
        return img.clone();
    }

    let h_mat = match get_destination_to_source_homography(dst_w as f64, dst_h as f64, tl, tr, br, bl) {
        Some(m) => m,
        None => return img.clone(),
    };

    let mut dst: RgbImage = ImageBuffer::new(dst_w, dst_h);

    for dy in 0..dst_h {
        let y_f = dy as f64;
        for dx in 0..dst_w {
            let x_f = dx as f64;

            // Projective transform: [sx, sy, sz]^T = H * [dx, dy, 1]^T
            let sz = h_mat[6] * x_f + h_mat[7] * y_f + h_mat[8];
            if sz.abs() < 1e-9 {
                continue;
            }
            let inv_z = 1.0 / sz;
            let sx = (h_mat[0] * x_f + h_mat[1] * y_f + h_mat[2]) * inv_z;
            let sy = (h_mat[3] * x_f + h_mat[4] * y_f + h_mat[5]) * inv_z;

            // #51 是正: NaN/Inf ガード。
            // 射影行列のスケールによっては inv_z が極大になり sx/sy が Inf や NaN になる。
            // Rust の `f64 as u32` キャストは Inf → u32::MAX、NaN → 0 の飽和挙動になるが
            // 境界チェック (sx >= 0.0 && sx < ...) が NaN では常に false になるため
            // 正常動作し得ないピクセルが黒点として残る。明示ガードで完全にスキップする。
            if !sx.is_finite() || !sy.is_finite() {
                continue;
            }

            // Subpixel bilinear interpolation in source image
            if sx >= 0.0 && sx < (sw as f64 - 1.0) && sy >= 0.0 && sy < (sh as f64 - 1.0) {
                let x0 = sx.floor() as u32;
                let y0 = sy.floor() as u32;
                let x1 = (x0 + 1).min(sw - 1);
                let y1 = (y0 + 1).min(sh - 1);

                let fx = (sx - x0 as f64) as f32;
                let fy = (sy - y0 as f64) as f32;

                let p00 = src.get_pixel(x0, y0).0;
                let p10 = src.get_pixel(x1, y0).0;
                let p01 = src.get_pixel(x0, y1).0;
                let p11 = src.get_pixel(x1, y1).0;

                let mut out = [0u8; 3];
                for c in 0..3 {
                    let top = p00[c] as f32 * (1.0 - fx) + p10[c] as f32 * fx;
                    let bot = p01[c] as f32 * (1.0 - fx) + p11[c] as f32 * fx;
                    let val = top * (1.0 - fy) + bot * fy;
                    out[c] = val.round().clamp(0.0, 255.0) as u8;
                }
                dst.put_pixel(dx, dy, Rgb(out));
            } else if sx >= 0.0 && sx < sw as f64 && sy >= 0.0 && sy < sh as f64 {
                // Nearest neighbor fallback at strict boundary
                let px = sx.round() as u32;
                let py = sy.round() as u32;
                if px < sw && py < sh {
                    dst.put_pixel(dx, dy, *src.get_pixel(px, py));
                }
            }
        }
    }

    DynamicImage::ImageRgb8(dst)
}

fn remove_shadow_simple(img: &DynamicImage) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();

    let mut result = RgbImage::new(w, h);

    let window_size = 31;
    let half_win = window_size / 2;

    let mut integral = vec![0u64; ((w + 1) * (h + 1)) as usize];

    for y in 0..h {
        for x in 0..w {
            let p = rgb.get_pixel(x, y);
            let lum = (p[0] as u64 + p[1] as u64 + p[2] as u64) / 3;
            let idx = ((y + 1) * (w + 1) + x + 1) as usize;
            integral[idx] = lum
                + integral[((y) * (w + 1) + x + 1) as usize]
                + integral[((y + 1) * (w + 1) + x) as usize]
                - integral[((y) * (w + 1) + x) as usize];
        }
    }

    for y in 0..h {
        for x in 0..w {
            let x1 = x.saturating_sub(half_win).min(w - 1);
            let y1 = y.saturating_sub(half_win).min(h - 1);
            let x2 = (x + half_win).min(w - 1);
            let y2 = (y + half_win).min(h - 1);

            let count = ((x2 - x1 + 1) * (y2 - y1 + 1)) as u64;
            let sum = integral[((y2 + 1) * (w + 1) + x2 + 1) as usize]
                + integral[(y1 * (w + 1) + x1) as usize]
                - integral[((y2 + 1) * (w + 1) + x1) as usize]
                - integral[(y1 * (w + 1) + x2 + 1) as usize];

            let mean = sum / count;
            let p = rgb.get_pixel(x, y);

            let scale = if mean > 10 { 128.0 / mean as f64 } else { 1.0 };
            let scale = scale.clamp(0.5, 2.0);

            let nr = (p[0] as f64 * scale).min(255.0) as u8;
            let ng = (p[1] as f64 * scale).min(255.0) as u8;
            let nb = (p[2] as f64 * scale).min(255.0) as u8;

            result.put_pixel(x, y, Rgb([nr, ng, nb]));
        }
    }

    DynamicImage::ImageRgb8(result)
}

fn enhance_contrast(img: &mut RgbImage) {
    let mut hist = [0u32; 256];
    for p in img.pixels() {
        let lum = ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as usize;
        hist[lum] += 1;
    }

    let total = img.width() * img.height();
    let mut cumulative = [0u32; 256];
    cumulative[0] = hist[0];
    for i in 1..256 {
        cumulative[i] = cumulative[i - 1] + hist[i];
    }

    let lut: Vec<u8> = (0..256)
        .map(|i| ((cumulative[i] as f64 / total as f64) * 255.0).min(255.0) as u8)
        .collect();

    for p in img.pixels_mut() {
        p[0] = lut[p[0] as usize];
        p[1] = lut[p[1] as usize];
        p[2] = lut[p[2] as usize];
    }
}
