use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub timings: bool,
    pub root_url: Option<String>,
    pub math: MathConfig,
    pub html: HtmlConfig,
    pub images: ImagesConfig,
    pub feed: FeedConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct MathConfig {
    pub prefer_persistent: bool,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HtmlConfig {
    pub template_path: String,
    pub css_href: String,
    pub blog_dir: Option<String>,
}

impl Default for HtmlConfig {
    fn default() -> Self {
        Self {
            template_path: "static/template.html".into(),
            css_href: "static/styles.css".into(),
            blog_dir: Some("blog".into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ImagesConfig {
    pub cache_dir: String,
    pub base_dir: Option<String>,
    pub img_root_url: Option<String>,
    pub sizes: Vec<u32>,
    pub display_sizes: Vec<u32>,
    pub meta_size: Option<u32>,
    pub jpeg_quality: u8,
    pub layout_width: u32,
    pub remote_fetch_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FeedConfig {
    pub enabled: bool,
    pub output_path: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub link: Option<String>,
    pub limit: Option<usize>,
}

impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            output_path: "rss.xml".into(),
            title: None,
            description: None,
            link: None,
            limit: None,
        }
    }
}

impl Default for ImagesConfig {
    fn default() -> Self {
        Self {
            cache_dir: "img".into(),
            base_dir: None,
            img_root_url: None,
            sizes: vec![480, 800, 1200],
            display_sizes: Vec::new(),
            meta_size: None,
            jpeg_quality: 85,
            layout_width: 1200,
            remote_fetch_timeout_secs: 10,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("failed to read config {}: {}", display(path), e))?;
        let mut config: Config = toml::from_str(&contents)
            .map_err(|e| format!("failed to parse config {}: {}", display(path), e))?;
        config.normalize();
        Ok(config)
    }

    fn normalize(&mut self) {
        if let Some(root) = &mut self.root_url {
            if root != "/" {
                *root = root.trim_end_matches('/').to_string();
            }
        }
        if let Some(blog_dir) = &mut self.html.blog_dir {
            let trimmed = blog_dir.trim();
            if trimmed.is_empty() {
                self.html.blog_dir = None;
            } else {
                *blog_dir = trimmed.trim_matches('/').to_string();
                if blog_dir.is_empty() {
                    self.html.blog_dir = None;
                }
            }
        }
        self.feed.normalize();
        self.images.normalize();
    }
}

impl ImagesConfig {
    fn normalize(&mut self) {
        if self.cache_dir.trim().is_empty() {
            self.cache_dir = "img".into();
        }
        self.sizes.retain(|v| *v > 0);
        self.sizes.sort_unstable();
        self.sizes.dedup();
        if self.sizes.is_empty() {
            self.sizes.push(self.layout_width.max(1));
        }
        self.display_sizes.retain(|v| *v > 0);
        self.display_sizes.sort_unstable();
        self.display_sizes.dedup();
        if self.display_sizes.is_empty() {
            self.display_sizes = self.sizes.clone();
        } else {
            self.display_sizes
                .retain(|v| self.sizes.binary_search(v).is_ok());
            if self.display_sizes.is_empty() {
                self.display_sizes = self.sizes.clone();
            }
        }
        if self.layout_width == 0 {
            self.layout_width = 1200;
        }
        self.meta_size = self.meta_size.and_then(|value| {
            if value == 0 {
                None
            } else if self.sizes.binary_search(&value).is_ok() {
                Some(value)
            } else {
                None
            }
        });
        self.jpeg_quality = self.jpeg_quality.clamp(10, 100);
        if self.remote_fetch_timeout_secs == 0 {
            self.remote_fetch_timeout_secs = 10;
        }
        if let Some(root) = &mut self.img_root_url {
            let trimmed = root.trim();
            if trimmed.is_empty() {
                self.img_root_url = None;
            } else if trimmed == "/" {
                *root = "/".into();
            } else {
                *root = trimmed.trim_end_matches('/').to_string();
            }
        }
    }
}

impl FeedConfig {
    fn normalize(&mut self) {
        let trimmed = self.output_path.trim();
        if trimmed.is_empty() {
            self.output_path = "rss.xml".into();
        } else {
            self.output_path = trimmed.to_string();
        }

        self.title = self.title.as_ref().and_then(|t| {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        self.description = self.description.as_ref().and_then(|d| {
            let trimmed = d.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        self.link = self.link.as_ref().and_then(|l| {
            let trimmed = l.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        if let Some(limit) = self.limit {
            if limit == 0 {
                self.limit = None;
            }
        }
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn default_config_path(input_path: &Path) -> PathBuf {
    let dir = input_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join("dllup.toml")
}
