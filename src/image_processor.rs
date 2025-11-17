use crate::config;
use image::codecs::gif::GifDecoder;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat};
use rexif::{parse_buffer_quiet, ExifData, ExifTag, TagValue};
use roxmltree::Document;
use std::fs;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ImageProcessor {
    config: config::ImagesConfig,
    cache_dir: PathBuf,
    root_url: Option<String>,
}

lazy_static! {
    static ref RESIZE_DISPATCHER: Arc<ResizeDispatcher> = Arc::new(ResizeDispatcher::new());
}

#[derive(Debug, Clone)]
pub struct ProcessedImage {
    pub variants: Vec<ImageVariant>,
    pub original: Option<ImageVariant>,
    pub display_width: u32,
    pub display_height: u32,
    pub original_reference: String,
    pub exif: Option<ExifSummary>,
    pub is_wide: bool,
}

#[derive(Debug, Clone)]
pub struct ImageVariant {
    pub width: u32,
    pub height: u32,
    pub url: String,
    pub mime_type: String,
}

#[derive(Debug, Clone)]
pub struct ExifSummary {
    pub entries: Vec<(String, String)>,
}

#[derive(Debug)]
struct SourceImage {
    reference: String,
    bytes: Arc<[u8]>,
    format: SourceFormat,
    cached_path: Option<PathBuf>,
}

#[derive(Clone)]
struct VariantSpec {
    width: u32,
    height: u32,
    path: PathBuf,
}

#[derive(Clone)]
struct VariantJob {
    width: u32,
    height: u32,
    path: PathBuf,
}

#[derive(Debug, Clone)]
enum SourceFormat {
    Svg,
    Raster(ImageFormat),
}

#[derive(Debug, Error)]
pub enum ImageError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("network error: {0}")]
    Network(String),
    #[error("image decoding failed: {0}")]
    Decode(String),
    #[error("unsupported image format")]
    UnsupportedFormat,
}

impl ImageProcessor {
    pub fn new(config: &config::Config) -> Self {
        let cache_dir = PathBuf::from(&config.images.cache_dir);
        let _ = fs::create_dir_all(&cache_dir);
        Self {
            config: config.images.clone(),
            cache_dir,
            root_url: config.root_url.clone(),
        }
    }

    fn process_gif(&self, source: SourceImage) -> Result<ProcessedImage, ImageError> {
        let decoder = GifDecoder::new(Cursor::new(&*source.bytes))
            .map_err(|e| ImageError::Decode(e.to_string()))?;
        let (width, height) = decoder.dimensions();
        let width = width.max(1);
        let height = height.max(1);

        let (display_width, display_height, is_wide) =
            compute_display_dimensions(width as f64, height as f64, self.config.layout_width);

        let original_path = self.ensure_original_cached(&source, "gif")?;
        let original_url = self.public_url_for(&original_path);
        let original_variant = ImageVariant {
            width,
            height,
            url: original_url.clone(),
            mime_type: "image/gif".into(),
        };

        Ok(ProcessedImage {
            variants: Vec::new(),
            original: Some(original_variant),
            display_width,
            display_height,
            original_reference: source.reference,
            exif: None,
            is_wide,
        })
    }

    pub fn process(
        &self,
        reference: &str,
        asset_root: &Path,
    ) -> Result<ProcessedImage, ImageError> {
        let source = self.load_source(reference, asset_root)?;
        match source.format {
            SourceFormat::Svg => self.process_svg(source),
            SourceFormat::Raster(format) => self.process_raster(source, format),
        }
    }

    fn process_svg(&self, source: SourceImage) -> Result<ProcessedImage, ImageError> {
        let original_path = self.ensure_original_cached(&source, "svg")?;
        let original_url = self.public_url_for(&original_path);

        let layout_limit = self.config.layout_width;
        let svg_dimensions = estimate_svg_dimensions(source.bytes.as_ref())
            .unwrap_or((layout_limit as f64, layout_limit as f64));
        let scaled_dimensions = if svg_dimensions.0 > 0.0 && svg_dimensions.1 > 0.0 {
            let max_dim = svg_dimensions.0.max(svg_dimensions.1);
            if max_dim < layout_limit as f64 {
                let scale = layout_limit as f64 / max_dim;
                (svg_dimensions.0 * scale, svg_dimensions.1 * scale)
            } else {
                svg_dimensions
            }
        } else {
            svg_dimensions
        };
        let (display_width, display_height, is_wide) =
            compute_display_dimensions(scaled_dimensions.0, scaled_dimensions.1, layout_limit);
        let svg_width = display_width.max(1);
        let svg_height = display_height.max(1);

        Ok(ProcessedImage {
            variants: Vec::new(),
            original: Some(ImageVariant {
                width: svg_width,
                height: svg_height,
                url: original_url,
                mime_type: "image/svg+xml".into(),
            }),
            display_width,
            display_height,
            original_reference: source.reference,
            exif: None,
            is_wide,
        })
    }

    fn process_raster(
        &self,
        source: SourceImage,
        format: ImageFormat,
    ) -> Result<ProcessedImage, ImageError> {
        if format == ImageFormat::Gif {
            return self.process_gif(source);
        }
        let extension = extension_for_format(format).ok_or(ImageError::UnsupportedFormat)?;
        let original_path = self.ensure_original_cached(&source, extension)?;
        if let Some(mut processed) = self.try_build_processed_from_cache(
            &source,
            &original_path,
            format,
            extension,
        ) {
            if processed.exif.is_none() {
                processed.exif = parse_buffer_quiet(source.bytes.as_ref())
                    .0
                    .ok()
                    .map(|data| summarize_exif(&data));
            }
            return Ok(processed);
        }

        let exif_data = parse_buffer_quiet(source.bytes.as_ref()).0.ok();
        let original_url = self.public_url_for(&original_path);
        let mime_type = mime_type_for_format(format).to_string();

        let mut exif_bytes_raw = exif_data
            .as_ref()
            .and_then(|data| data.serialize().ok())
            .map(ensure_exif_header);
        let original_orientation = exif_data.as_ref().and_then(exif_orientation);

        if let Some(bytes) = exif_bytes_raw.as_mut() {
            normalize_exif_orientation(bytes);
        }
        let exif_bytes = exif_bytes_raw.map(Arc::new);

        let (mut width, mut height) =
            image::image_dimensions(&original_path).map_err(|e| ImageError::Decode(e.to_string()))?;
        if matches!(original_orientation, Some(5..=8)) {
            std::mem::swap(&mut width, &mut height);
        }
        let (display_width, display_height, is_wide) =
            compute_display_dimensions(width as f64, height as f64, self.config.layout_width);
        save_cached_dimensions(&original_path, width, height)?;

        let original_stem = original_path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "image".to_string());

        let target_widths = self.target_resize_widths(width, display_width);
        let mut variant_specs: Vec<VariantSpec> = Vec::new();
        let mut resize_jobs: Vec<VariantJob> = Vec::new();
        for target_width in target_widths {
            let filename = if extension.is_empty() {
                format!("{}-{}", original_stem, target_width)
            } else {
                format!("{}-{}.{}", original_stem, target_width, extension)
            };
            let target_path = self.cache_dir.join(filename);
            let target_height = ((target_width as f64 / width as f64) * height as f64)
                .round()
                .max(1.0) as u32;
            if !target_path.exists() {
                resize_jobs.push(VariantJob {
                    width: target_width,
                    height: target_height,
                    path: target_path.clone(),
                });
            }
            variant_specs.push(VariantSpec {
                width: target_width,
                height: target_height,
                path: target_path,
            });
        }

        if !resize_jobs.is_empty() {
            fs::create_dir_all(&self.cache_dir)?;
            let dispatch_exif = exif_bytes.clone();
            schedule_resize_generation(
                source.reference.clone(),
                Arc::clone(&source.bytes),
                format,
                original_orientation,
                resize_jobs,
                dispatch_exif,
                self.config.jpeg_quality,
            );
        }

        let mut variants: Vec<ImageVariant> = variant_specs
            .into_iter()
            .map(|spec| ImageVariant {
                width: spec.width,
                height: spec.height,
                url: self.public_url_for(&spec.path),
                mime_type: mime_type.clone(),
            })
            .collect();
        variants.sort_by_key(|v| v.width);
        let entries = exif_data.as_ref().map(summarize_exif);
        let original_variant = ImageVariant {
            width,
            height,
            url: original_url,
            mime_type: mime_type.clone(),
        };

        Ok(ProcessedImage {
            variants,
            original: Some(original_variant),
            display_width,
            display_height,
            original_reference: source.reference,
            exif: entries,
            is_wide,
        })
    }

    fn load_source(&self, reference: &str, asset_root: &Path) -> Result<SourceImage, ImageError> {
        if is_remote(reference) {
            self.fetch_remote(reference)
        } else {
            self.read_local(reference, asset_root)
        }
    }

    fn fetch_remote(&self, reference: &str) -> Result<SourceImage, ImageError> {
        fs::create_dir_all(&self.cache_dir)?;
        let candidates = self.remote_cache_candidates(reference);
        let primary_path = candidates
            .first()
            .cloned()
            .unwrap_or_else(|| self.cache_dir.join("image"));
        if let Some(source) = self.try_load_cached_remote(reference, &candidates)? {
            return Ok(source);
        }

        eprintln!("[images] fetching remote {}", reference);
        let fetch_start = Instant::now();
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(self.config.remote_fetch_timeout_secs))
            .build();
        let response = agent
            .get(reference)
            .call()
            .map_err(|e| ImageError::Network(e.to_string()))?;
        if response.status() >= 400 {
            return Err(ImageError::Network(format!(
                "failed to fetch {}: HTTP {}",
                reference,
                response.status()
            )));
        }
        let mut reader = response.into_reader();
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        fs::write(&primary_path, &buf)?;
        eprintln!(
            "[images] fetched remote {} in {:?}",
            reference,
            fetch_start.elapsed()
        );

        Ok(SourceImage {
            reference: reference.to_string(),
            cached_path: Some(primary_path),
            format: detect_format(reference, &buf)?,
            bytes: Arc::from(buf),
        })
    }

    fn try_load_cached_remote(
        &self,
        reference: &str,
        candidates: &[PathBuf],
    ) -> Result<Option<SourceImage>, ImageError> {
        for path in candidates {
            if path.exists() {
                let bytes = fs::read(path)?;
                return Ok(Some(SourceImage {
                    reference: reference.to_string(),
                    cached_path: Some(path.clone()),
                    format: detect_format(reference, &bytes)?,
                    bytes: Arc::from(bytes),
                }));
            }
        }
        Ok(None)
    }

    fn remote_cache_candidates(&self, reference: &str) -> Vec<PathBuf> {
        let trimmed = reference.split('?').next().unwrap_or(reference);
        let raw_name = Path::new(trimmed)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(trimmed);
        let mut base = sanitize_filename(raw_name);
        if base.is_empty() {
            base = "image".to_string();
        }

        let mut names = Vec::new();
        let mut canonical = base.clone();
        if Path::new(&canonical).extension().is_none() {
            if let Some(ext) = path_extension_from_str(trimmed) {
                let ext = ext.trim();
                if !ext.is_empty() {
                    let ext_lower = ext.to_ascii_lowercase();
                    if !canonical.ends_with('.') {
                        canonical.push('.');
                    }
                    canonical.push_str(&ext_lower);
                }
            }
        }

        push_unique_name(&mut names, canonical.clone());
        push_unique_name(&mut names, base);
        if let Some(stem) = Path::new(&canonical)
            .file_stem()
            .and_then(|s| s.to_str())
        {
            push_unique_name(&mut names, stem.to_string());
        }

        names
            .into_iter()
            .map(|name| self.cache_dir.join(name))
            .collect()
    }

    fn read_local(&self, reference: &str, asset_root: &Path) -> Result<SourceImage, ImageError> {
        let path = if reference.starts_with("./") || reference.starts_with("../") {
            asset_root.join(reference)
        } else {
            let candidate = Path::new(reference);
            if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                asset_root.join(candidate)
            }
        };
        let bytes = fs::read(&path)?;
        Ok(SourceImage {
            reference: reference.to_string(),
            cached_path: Some(path.clone()),
            format: detect_format(reference, &bytes)?,
            bytes: Arc::from(bytes),
        })
    }

    fn try_build_processed_from_cache(
        &self,
        source: &SourceImage,
        original_path: &Path,
        format: ImageFormat,
        extension: &str,
    ) -> Option<ProcessedImage> {
        let (width, height) = load_cached_dimensions(original_path)?;
        let original_stem = original_path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())?;
        let mime_type = mime_type_for_format(format).to_string();
        let (display_width, display_height, is_wide) =
            compute_display_dimensions(width as f64, height as f64, self.config.layout_width);

        let target_widths = self.target_resize_widths(width, display_width);
        let mut variants = Vec::new();
        for target_width in target_widths {
            let filename = if extension.is_empty() {
                format!("{}-{}", original_stem, target_width)
            } else {
                format!("{}-{}.{}", original_stem, target_width, extension)
            };
            let variant_path = self.cache_dir.join(&filename);
            if !variant_path.exists() {
                return None;
            }
            let target_height = ((target_width as f64 / width as f64) * height as f64)
                .round()
                .max(1.0) as u32;
            variants.push(ImageVariant {
                width: target_width,
                height: target_height,
                url: self.public_url_for(&variant_path),
                mime_type: mime_type.clone(),
            });
        }
        variants.sort_by_key(|v| v.width);

        Some(ProcessedImage {
            variants,
            original: Some(ImageVariant {
                width,
                height,
                url: self.public_url_for(original_path),
                mime_type,
            }),
            display_width,
            display_height,
            original_reference: source.reference.clone(),
            exif: None,
            is_wide,
        })
    }

fn target_resize_widths(&self, original_width: u32, display_width: u32) -> Vec<u32> {
    let mut sizes = self.config.sizes.clone();
    if !sizes.contains(&self.config.layout_width) {
        sizes.push(self.config.layout_width);
    }
        if display_width > 0 && !sizes.contains(&display_width) {
            sizes.push(display_width);
        }
        sizes.sort_unstable();
        sizes.dedup();

        let mut widths = Vec::new();
    for size in sizes {
        let target_width = size.min(original_width);
        if target_width == 0 || target_width == original_width {
            continue;
        }
            if widths.last().copied() == Some(target_width) {
                continue;
        }
        widths.push(target_width);
    }
    widths
}

    fn ensure_original_cached(
        &self,
        source: &SourceImage,
        extension: &str,
    ) -> Result<PathBuf, ImageError> {
        if let Some(existing) = &source.cached_path {
            if existing.starts_with(&self.cache_dir) {
                return Ok(existing.clone());
            }
        }
        fs::create_dir_all(&self.cache_dir)?;

        let base_name =
            preferred_filename(source, extension).unwrap_or_else(|| default_filename(extension));
        let mut target = self.cache_dir.join(&base_name);

        if target.exists() {
            if fs::read(&target)? == source.bytes.as_ref() {
                return Ok(target);
            }
            let mut counter = 2usize;
            loop {
                let candidate_name = numbered_filename(&base_name, counter);
                let candidate_path = self.cache_dir.join(&candidate_name);
                if candidate_path.exists() {
                    if fs::read(&candidate_path)? == source.bytes.as_ref() {
                        return Ok(candidate_path);
                    }
                } else {
                    target = candidate_path;
                    break;
                }
                counter += 1;
            }
        }

        fs::write(&target, &*source.bytes)?;
        Ok(target)
    }

    fn public_url_for(&self, path: &Path) -> String {
        use std::path::Component;

        let relative = path.strip_prefix(&self.cache_dir).unwrap_or(path);
        let rel_components: Vec<String> = relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(segment) => Some(segment.to_string_lossy().replace('\\', "/")),
                _ => None,
            })
            .collect();
        let rel_without_cache = rel_components.join("/");

        if let Some(prefix) = self.config.img_root_url.as_ref().filter(|s| !s.is_empty()) {
            if rel_without_cache.is_empty() {
                prefix.clone()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), rel_without_cache)
            }
        } else {
            let cache_components: Vec<String> = Path::new(&self.config.cache_dir)
                .components()
                .filter_map(|component| match component {
                    Component::Normal(segment) => {
                        Some(segment.to_string_lossy().replace('\\', "/"))
                    }
                    _ => None,
                })
                .collect();
            let mut full_parts = cache_components;
            if !rel_without_cache.is_empty() {
                full_parts.push(rel_without_cache);
            }
            let joined = full_parts.join("/");
            if let Some(prefix) = self.root_url.as_ref() {
                if prefix == "/" {
                    if joined.is_empty() {
                        "/".to_string()
                    } else {
                        format!("/{}", joined.trim_start_matches('/'))
                    }
                } else if joined.is_empty() {
                    prefix.clone()
                } else {
                    format!("{}/{}", prefix.trim_end_matches('/'), joined)
                }
            } else if joined.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", joined.trim_start_matches('/'))
            }
        }
    }
}

fn detect_format(reference: &str, bytes: &[u8]) -> Result<SourceFormat, ImageError> {
    if looks_like_svg(reference, bytes) {
        return Ok(SourceFormat::Svg);
    }
    match image::guess_format(bytes) {
        Ok(format) => Ok(SourceFormat::Raster(format)),
        Err(_) => {
            if let Some(ext) = path_extension_from_str(reference) {
                if let Some(format) = image_format_from_extension(ext) {
                    return Ok(SourceFormat::Raster(format));
                }
            }
            Err(ImageError::UnsupportedFormat)
        }
    }
}

fn looks_like_svg(reference: &str, bytes: &[u8]) -> bool {
    if reference.to_ascii_lowercase().ends_with(".svg") {
        return true;
    }
    let head = bytes
        .iter()
        .skip_while(|b| b.is_ascii_whitespace())
        .take(512)
        .cloned()
        .collect::<Vec<_>>();
    let head_str = String::from_utf8_lossy(&head).to_lowercase();
    head_str.contains("<svg")
}

fn path_extension_from_str(s: &str) -> Option<&str> {
    s.split('?')
        .next()
        .and_then(|path| Path::new(path).extension())
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.trim_start_matches('.'))
        .filter(|ext| !ext.is_empty())
}

fn image_format_from_extension(ext: &str) -> Option<ImageFormat> {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "webp" => Some(ImageFormat::WebP),
        _ => None,
    }
}

fn extension_for_format(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Jpeg => Some("jpg"),
        ImageFormat::Png => Some("png"),
        ImageFormat::WebP => Some("webp"),
        _ => None,
    }
}

fn mime_type_for_format(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Png => "image/png",
        ImageFormat::Gif => "image/gif",
        ImageFormat::Bmp => "image/bmp",
        ImageFormat::Tiff => "image/tiff",
        ImageFormat::WebP => "image/webp",
        _ => "application/octet-stream",
    }
}

fn encode_image(
    image: &DynamicImage,
    format: ImageFormat,
    exif_bytes: Option<&[u8]>,
    jpeg_quality: u8,
) -> Result<Vec<u8>, ImageError> {
    let mut buf = Vec::new();
    match format {
        ImageFormat::Jpeg => {
            let mut encoder = JpegEncoder::new_with_quality(&mut buf, jpeg_quality);
            encoder
                .encode_image(image)
                .map_err(|e| ImageError::Decode(e.to_string()))?;
            if let Some(exif_data) = exif_bytes {
                insert_exif_segment(&mut buf, exif_data);
            }
        }
        _ => {
            let mut cursor = io::Cursor::new(&mut buf);
            let format = image::ImageOutputFormat::from(format);
            image
                .write_to(&mut cursor, format)
                .map_err(|e| ImageError::Decode(e.to_string()))?;
        }
    }
    Ok(buf)
}

fn generate_variant_file(
    job: &VariantJob,
    source_image: &DynamicImage,
    format: ImageFormat,
    exif_bytes: Option<&[u8]>,
    jpeg_quality: u8,
) -> Result<(), ImageError> {
    let resized = source_image.resize(job.width, job.height, FilterType::Lanczos3);
    let encoded = encode_image(&resized, format, exif_bytes, jpeg_quality)?;
    if let Some(parent) = job.path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&job.path, &encoded)?;
    Ok(())
}

fn schedule_resize_generation(
    reference: String,
    bytes: Arc<[u8]>,
    format: ImageFormat,
    orientation: Option<u16>,
    jobs: Vec<VariantJob>,
    exif_bytes: Option<Arc<Vec<u8>>>,
    jpeg_quality: u8,
) {
    if jobs.is_empty() {
        return;
    }

    let dispatcher = Arc::clone(&RESIZE_DISPATCHER);
    dispatcher.spawn(move || {
        eprintln!("[images] loading full-size {}", reference);
        let start = Instant::now();
        let mut image = match image::load_from_memory(bytes.as_ref()) {
            Ok(img) => img,
            Err(err) => {
                eprintln!("Failed to load {}: {}", reference, err);
                return;
            }
        };
        if let Some(orientation) = orientation {
            image = apply_orientation(image, orientation);
        }
        eprintln!(
            "[images] loaded full-size {} in {:?}",
            reference,
            start.elapsed()
        );
        let exif_slice = exif_bytes
            .as_deref()
            .map(|buf| buf.as_slice());
        for job in jobs {
            if let Err(err) =
                generate_variant_file(&job, &image, format, exif_slice, jpeg_quality)
            {
                eprintln!(
                    "Failed to build variant {} for {}: {}",
                    job.path.display(),
                    reference,
                    err
                );
            }
        }
    });
}

fn ensure_exif_header(bytes: Vec<u8>) -> Vec<u8> {
    const EXIF_HEADER: &[u8; 6] = b"Exif\0\0";
    if bytes.starts_with(EXIF_HEADER) {
        bytes
    } else {
        let mut prefixed = Vec::with_capacity(bytes.len() + EXIF_HEADER.len());
        prefixed.extend_from_slice(EXIF_HEADER);
        prefixed.extend_from_slice(&bytes);
        prefixed
    }
}

fn insert_exif_segment(jpeg: &mut Vec<u8>, exif_data: &[u8]) {
    if jpeg.len() < 2 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return;
    }
    if exif_data.len() + 2 > u16::MAX as usize {
        eprintln!("skipping EXIF attachment: data too large");
        return;
    }

    // Remove existing EXIF APP1 segment if present.
    let mut scan = 2;
    while scan + 4 <= jpeg.len() && jpeg[scan] == 0xFF {
        let marker = jpeg[scan + 1];
        if marker == 0xDA {
            break;
        }
        if marker == 0xE1 {
            let len = ((jpeg[scan + 2] as usize) << 8) | jpeg[scan + 3] as usize;
            let end = (scan + 2 + len).min(jpeg.len());
            jpeg.drain(scan..end);
            break;
        }
        if marker == 0xD8 || marker == 0xD9 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            scan += 2;
            continue;
        }
        let len = ((jpeg[scan + 2] as usize) << 8) | jpeg[scan + 3] as usize;
        if len < 2 {
            break;
        }
        scan += 2 + len;
    }

    // Insert after any existing APP markers at the start.
    let mut insert_pos = 2;
    while insert_pos + 4 <= jpeg.len() && jpeg[insert_pos] == 0xFF {
        let marker = jpeg[insert_pos + 1];
        if !(0xE0..=0xEF).contains(&marker) {
            break;
        }
        if marker == 0xDA {
            break;
        }
        let len = ((jpeg[insert_pos + 2] as usize) << 8) | jpeg[insert_pos + 3] as usize;
        if len < 2 {
            break;
        }
        insert_pos += 2 + len;
    }

    let mut segment = Vec::with_capacity(exif_data.len() + 4);
    segment.extend_from_slice(&[0xFF, 0xE1]);
    let len = (exif_data.len() + 2) as u16;
    segment.extend_from_slice(&len.to_be_bytes());
    segment.extend_from_slice(exif_data);
    jpeg.splice(insert_pos..insert_pos, segment);
}

fn apply_orientation(image: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate270().fliph(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn normalize_exif_orientation(exif_bytes: &mut [u8]) {
    if exif_bytes.len() < 12 {
        return;
    }
    let mut cursor = 6; // skip "Exif\0\0"
    if cursor + 8 > exif_bytes.len() {
        return;
    }
    let byte_order = &exif_bytes[cursor..cursor + 2];
    let le = match byte_order {
        b"II" => true,
        b"MM" => false,
        _ => return,
    };
    cursor += 2;
    cursor += 2; // skip fixed 0x002A
    let ifd_offset = read_u32(&exif_bytes[cursor..cursor + 4], le) as usize;
    let mut pos = 6 + ifd_offset;
    if pos + 2 > exif_bytes.len() {
        return;
    }
    let entries = read_u16(&exif_bytes[pos..pos + 2], le) as usize;
    pos += 2;
    for _ in 0..entries {
        if pos + 12 > exif_bytes.len() {
            return;
        }
        let tag = read_u16(&exif_bytes[pos..pos + 2], le);
        if tag == 0x0112 {
            let value_offset = pos + 8;
            if value_offset + 2 > exif_bytes.len() {
                return;
            }
            if le {
                exif_bytes[value_offset] = 1;
                exif_bytes[value_offset + 1] = 0;
            } else {
                exif_bytes[value_offset] = 0;
                exif_bytes[value_offset + 1] = 1;
            }
            return;
        }
        pos += 12;
    }
}

fn read_u16(slice: &[u8], le: bool) -> u16 {
    if le {
        u16::from_le_bytes([slice[0], slice[1]])
    } else {
        u16::from_be_bytes([slice[0], slice[1]])
    }
}

fn read_u32(slice: &[u8], le: bool) -> u32 {
    if le {
        u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
    } else {
        u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]])
    }
}

fn summarize_exif(exif: &ExifData) -> ExifSummary {
    let mut entries = Vec::new();

    if let Some(camera) = camera_description(exif) {
        entries.push(("Camera".to_string(), camera));
    }

    if let Some(lens) = lens_description(exif) {
        entries.push(("Lens".to_string(), lens));
    }

    if let Some(aperture) = aperture_value(exif) {
        entries.push(("Aperture".to_string(), aperture));
    }

    if let Some(shutter) = shutter_value(exif) {
        entries.push(("Shutter speed".to_string(), shutter));
    }

    if let Some(iso) = exif_value(exif, ExifTag::ISOSpeedRatings) {
        entries.push(("ISO".to_string(), iso));
    }

    if let Some(software) = exif_value(exif, ExifTag::Software) {
        entries.push(("Software".to_string(), software));
    }

    if let Some(date) = exif_value(exif, ExifTag::DateTimeOriginal) {
        entries.push(("Date".to_string(), date));
    }

    ExifSummary { entries }
}

fn exif_orientation(exif: &ExifData) -> Option<u16> {
    exif.entries
        .iter()
        .find(|entry| entry.tag == ExifTag::Orientation)
        .and_then(|entry| tag_value_to_u16(&entry.value))
}

fn tag_value_to_u16(value: &TagValue) -> Option<u16> {
    match value {
        TagValue::U16(values) => values.first().copied(),
        TagValue::U8(values) => values.first().copied().map(u16::from),
        TagValue::U32(values) => values
            .first()
            .copied()
            .map(|v| v.min(u16::MAX as u32) as u16),
        TagValue::I16(values) => values
            .first()
            .copied()
            .map(|v| if v < 0 { 0 } else { v as u16 }),
        TagValue::I32(values) => values
            .first()
            .copied()
            .map(|v| if v < 0 { 0 } else { v as u16 }),
        TagValue::I8(values) => values
            .first()
            .copied()
            .map(|v| if v < 0 { 0 } else { v as u16 }),
        _ => None,
    }
}

fn exif_value(exif: &ExifData, tag: ExifTag) -> Option<String> {
    exif.entries
        .iter()
        .find(|entry| entry.tag == tag)
        .and_then(|entry| {
            let value = entry.value_more_readable.trim();
            if value.is_empty() || value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(value.to_string())
            }
        })
}

fn camera_description(exif: &ExifData) -> Option<String> {
    let make = exif_value(exif, ExifTag::Make);
    let model = exif_value(exif, ExifTag::Model);
    match (make, model) {
        (Some(make), Some(model)) => {
            let make_lower = make.to_ascii_lowercase();
            let model_lower = model.to_ascii_lowercase();
            if model_lower.starts_with(&make_lower) {
                Some(model)
            } else {
                Some(format!("{} {}", make, model).trim().to_string())
            }
        }
        (Some(make), None) => Some(make),
        (None, Some(model)) => Some(model),
        _ => None,
    }
}

fn lens_description(exif: &ExifData) -> Option<String> {
    if let Some(model) = exif_value(exif, ExifTag::LensModel) {
        return Some(model);
    }
    let make = exif_value(exif, ExifTag::LensMake);
    let spec = exif_value(exif, ExifTag::LensSpecification);
    match (make, spec) {
        (Some(make), Some(spec)) => Some(format!("{} {}", make, spec).trim().to_string()),
        (Some(make), None) => Some(make),
        (None, Some(spec)) => Some(spec),
        _ => None,
    }
}

fn aperture_value(exif: &ExifData) -> Option<String> {
    exif_value(exif, ExifTag::FNumber).or_else(|| exif_value(exif, ExifTag::ApertureValue))
}

fn shutter_value(exif: &ExifData) -> Option<String> {
    exif_value(exif, ExifTag::ExposureTime).or_else(|| exif_value(exif, ExifTag::ShutterSpeedValue))
}

fn compute_display_dimensions(width: f64, height: f64, layout_limit: u32) -> (u32, u32, bool) {
    let layout = layout_limit.max(1) as f64;
    let mut original_width = if width > 0.0 { width } else { layout };
    let mut original_height = if height > 0.0 { height } else { layout };
    if original_width <= 0.0 {
        original_width = layout;
    }
    if original_height <= 0.0 {
        original_height = layout;
    }

    let ratio = if original_height > 0.0 {
        original_width / original_height
    } else {
        1.0
    };

    let mut is_wide = false;
    let (display_width, display_height) = if ratio < 1.0 {
        let max_height = layout.min(original_height);
        let width = (max_height * ratio).max(1.0);
        (width, max_height)
    } else if ratio >= 2.0 {
        is_wide = true;
        let max_width = layout * 2.0;
        let width = original_width.min(max_width);
        let height = (width / ratio).max(1.0);
        (width, height)
    } else {
        let width = layout.min(original_width);
        let height = (width / ratio).max(1.0);
        (width, height)
    };

    (
        display_width.round().max(1.0) as u32,
        display_height.round().max(1.0) as u32,
        is_wide,
    )
}

fn estimate_svg_dimensions(bytes: &[u8]) -> Option<(f64, f64)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let doc = Document::parse(text).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "svg" {
        return None;
    }

    let width = root.attribute("width").and_then(parse_dimension);
    let height = root.attribute("height").and_then(parse_dimension);
    if let (Some(w), Some(h)) = (width, height) {
        if w > 0.0 && h > 0.0 {
            return Some((w, h));
        }
    }

    root.attribute("viewBox")
        .and_then(parse_viewbox)
        .filter(|(w, h)| *w > 0.0 && *h > 0.0)
}

fn parse_viewbox(value: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() == 4 {
        if let (Ok(width), Ok(height)) = (parts[2].parse::<f64>(), parts[3].parse::<f64>()) {
            return Some((width, height));
        }
    }
    None
}

fn parse_dimension(input: &str) -> Option<f64> {
    let trimmed = input.trim();
    if trimmed.ends_with('%') {
        return None;
    }
    let numeric = trimmed
        .trim_end_matches("px")
        .trim_end_matches("cm")
        .trim_end_matches("mm")
        .trim_end_matches("in")
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_alphabetic());
    if numeric.is_empty() {
        return None;
    }
    numeric.parse::<f64>().ok()
}

fn is_remote(reference: &str) -> bool {
    let lower = reference.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn preferred_filename(source: &SourceImage, extension: &str) -> Option<String> {
    let raw = source
        .cached_path
        .as_ref()
        .and_then(|p| p.file_name().and_then(|s| s.to_str()))
        .map(|s| s.to_string())
        .or_else(|| {
            let trimmed = source
                .reference
                .split('?')
                .next()
                .unwrap_or(&source.reference);
            Path::new(trimmed)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })?;

    let mut sanitized = sanitize_filename(&raw);
    if sanitized.is_empty() {
        return None;
    }

    if let Some(ext) = Path::new(&sanitized).extension().and_then(|e| e.to_str()) {
        if !extension.is_empty() && !ext.eq_ignore_ascii_case(extension) {
            // Keep existing extension when present, even if it differs.
        }
    } else if !extension.is_empty() {
        if !sanitized.ends_with('.') {
            sanitized.push('.');
        }
        sanitized.push_str(extension);
    }

    Some(sanitized)
}

fn default_filename(extension: &str) -> String {
    if extension.is_empty() {
        "image".to_string()
    } else {
        format!("image.{}", extension)
    }
}

fn numbered_filename(base: &str, counter: usize) -> String {
    let path = Path::new(base);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(base);
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .filter(|s| !s.is_empty())
    {
        Some(ext) => format!("{}-{}.{}", stem, counter, ext),
        None => format!("{}-{}", stem, counter),
    }
}

fn sanitize_filename(input: &str) -> String {
    let no_query = input.split(&['?', '#'][..]).next().unwrap_or(input);
    let base = no_query.rsplit(['/', '\\']).next().unwrap_or(no_query);
    let sanitized: String = base
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    sanitized.trim_matches('_').to_string()
}

fn push_unique_name(names: &mut Vec<String>, candidate: String) {
    if candidate.is_empty() {
        return;
    }
    if !names.iter().any(|existing| existing == &candidate) {
        names.push(candidate);
    }
}

fn dimension_cache_path(original_path: &Path) -> PathBuf {
    original_path.with_extension("txt")
}

fn load_cached_dimensions(original_path: &Path) -> Option<(u32, u32)> {
    let cache_path = dimension_cache_path(original_path);
    let contents = fs::read_to_string(cache_path).ok()?;
    let mut parts = contents.split_whitespace();
    let width = parts.next()?.parse().ok()?;
    let height = parts.next()?.parse().ok()?;
    Some((width, height))
}

fn save_cached_dimensions(original_path: &Path, width: u32, height: u32) -> Result<(), io::Error> {
    let cache_path = dimension_cache_path(original_path);
    fs::write(cache_path, format!("{} {}\n", width, height))
}

struct ResizeDispatcher {
    pending: AtomicUsize,
    lock: Mutex<()>,
    condvar: Condvar,
}

impl ResizeDispatcher {
    fn new() -> Self {
        Self {
            pending: AtomicUsize::new(0),
            lock: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    fn spawn(self: Arc<Self>, job: impl FnOnce() + Send + 'static) {
        self.pending.fetch_add(1, Ordering::SeqCst);
        rayon::spawn_fifo(move || {
            job();
            self.job_finished();
        });
    }

    fn job_finished(&self) {
        if self.pending.fetch_sub(1, Ordering::SeqCst) == 1 {
            let _guard = self.lock.lock().unwrap();
            self.condvar.notify_all();
        }
    }

    fn wait(&self) {
        let mut guard = self.lock.lock().unwrap();
        while self.pending.load(Ordering::SeqCst) > 0 {
            guard = self.condvar.wait(guard).unwrap();
        }
    }
}

pub fn wait_for_pending_resizes() {
    RESIZE_DISPATCHER.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_webp_extension() {
        assert!(matches!(
            image_format_from_extension("webp"),
            Some(ImageFormat::WebP)
        ));
    }

    #[test]
    fn webp_extension_roundtrip() {
        assert_eq!(extension_for_format(ImageFormat::WebP), Some("webp"));
    }
}
