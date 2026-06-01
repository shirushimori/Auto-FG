use std::io::{self, BufRead, Read, Write};

use chrono::Local;
use colored::*;
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use regex::Regex;

pub fn clear_screen() {
    let _ = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/c", "cls"]).status()
    } else {
        std::process::Command::new("sh").args(["-c", "clear"]).status()
    };
}

pub fn timestamp() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

pub struct Log;

impl Log {
    fn fmt(tag: &str, color: &dyn Fn(&str) -> ColoredString, msg: &str, obj: &str) {
        println!(
            "{} {} {} {} {} : {} ",
            timestamp().bright_black(),
            "»".bright_black(),
            color(tag),
            "•".bright_black(),
            msg.white(),
            color(obj),
        );
    }

    pub fn success(msg: &str, obj: &str) {
        Self::fmt("SUCC", &|s| s.bright_green(), msg, obj);
    }

    pub fn error(msg: &str, obj: &str) {
        Self::fmt("ERRR", &|s| s.bright_red(), msg, obj);
    }

    pub fn done(msg: &str, obj: &str) {
        Self::fmt("DONE", &|s| s.bright_magenta(), msg, obj);
    }

    pub fn warning(msg: &str, obj: &str) {
        Self::fmt("WARN", &|s| s.bright_yellow(), msg, obj);
    }

    pub fn info(msg: &str, obj: &str) {
        Self::fmt("INFO", &|s| s.bright_blue(), msg, obj);
    }

    pub fn input(msg: &str) -> String {
        let prompt = format!(
            "{} {} {} {} {} ",
            timestamp().bright_black(),
            "»".bright_black(),
            "INPUT".bright_cyan(),
            "•".bright_black(),
            msg.white(),
        );
        print!("{prompt}");
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).ok();
        line.trim().to_string()
    }
}

// ---- PrivateBin decryption ----

use aes::Aes256;
use aes_gcm::aead::{Aead, Nonce, Payload};
use aes_gcm::{AesGcm, KeyInit};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use typenum::U16;

type Aes256Gcm16 = AesGcm<Aes256, U16>;

fn privatebin_paste_id(url: &str) -> Option<String> {
    let q = url.split('?').nth(1)?;
    Some(q.split('#').next()?.to_string())
}

fn privatebin_key(url: &str) -> Option<String> {
    let key = url.split('#').nth(1)?;
    if key.is_empty() { None } else { Some(key.to_string()) }
}

fn privatebin_fetch(paste_id: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .get(format!(
            "https://paste.fitgirl-repacks.site/?pasteid={paste_id}"
        ))
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("X-Requested-With", "XMLHttpRequest")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Referer", &format!("https://paste.fitgirl-repacks.site/?{paste_id}"))
        .send()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("HTTP {}", res.status()));
    }

    res.text().map_err(|e| format!("read body: {e}"))
}

fn privatebin_decrypt(json_str: &str, key_b58: &str) -> Result<String, String> {
    let data: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("bad JSON: {e}"))?;

    let adata = data["adata"]
        .as_array()
        .ok_or("missing adata".to_string())?;
    let spec = adata[0]
        .as_array()
        .ok_or("missing spec".to_string())?;
    let iv_b64 = spec[0]
        .as_str()
        .ok_or("missing iv".to_string())?;
    let salt_b64 = spec[1]
        .as_str()
        .ok_or("missing salt".to_string())?;
    let iterations = spec[2]
        .as_u64()
        .ok_or("missing iterations".to_string())? as u32;
    let key_size = spec[3]
        .as_u64()
        .ok_or("missing key_size".to_string())? as usize;
    let ct_b64 = data["ct"]
        .as_str()
        .ok_or("missing ct".to_string())?;

    let key_raw = bs58::decode(key_b58)
        .into_vec()
        .map_err(|e| format!("base58: {e}"))?;
    let mut key = vec![0u8; 32];
    let offset = 32usize.saturating_sub(key_raw.len());
    key[offset..].copy_from_slice(&key_raw[..key_raw.len().min(32)]);

    let salt = BASE64
        .decode(salt_b64.as_bytes())
        .map_err(|e| format!("salt base64: {e}"))?;
    let iv = BASE64
        .decode(iv_b64.as_bytes())
        .map_err(|e| format!("iv base64: {e}"))?;
    let ct = BASE64
        .decode(ct_b64.as_bytes())
        .map_err(|e| format!("ct base64: {e}"))?;

    let mut derived = vec![0u8; key_size / 8];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(&key, &salt, iterations, &mut derived);

    let cipher = Aes256Gcm16::new(aes_gcm::Key::<Aes256Gcm16>::from_slice(&derived));
    let nonce = Nonce::<Aes256Gcm16>::from_slice(&iv);

    let aad_str =
        serde_json::to_string(adata).map_err(|e| format!("stringify adata: {e}"))?;

    let plain = cipher
        .decrypt(nonce, Payload { msg: &ct, aad: aad_str.as_bytes() })
        .map_err(|_| "decryption failed (wrong key?)".to_string())?;

    let mut raw = Vec::new();
    flate2::read::DeflateDecoder::new(&plain[..])
        .read_to_end(&mut raw)
        .map_err(|e| format!("decompress: {e}"))?;

    Ok(String::from_utf8_lossy(&raw).to_string())
}

    use std::sync::LazyLock;
use std::time::Instant;

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://fuckingfast\.co/[^\s"'>]+"#).unwrap()
});

fn extract_links(text: &str) -> Vec<String> {
    // Try to unwrap PrivateBin JSON wrapper
    let body = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(json) => json
            .get("paste")
            .and_then(|v| v.as_str())
            .unwrap_or(text)
            .to_string(),
        Err(_) => text.to_string(),
    };

    let mut links: Vec<String> = URL_RE
        .find_iter(&body)
        .map(|m| m.as_str().trim_end_matches(&['.', ',', ')', ']', '>']).to_string())
        .collect();
    links.sort();
    links.dedup();
    links
}

// ---- shared input helpers ----

pub fn scrape_links(url: &str) -> Vec<String> {
    if url.contains("paste.fitgirl-repacks.site") {
        return scrape_privatebin(url);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let res = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .send()
        .unwrap_or_else(|e| {
            Log::error("Failed to fetch URL", &e.to_string());
            std::process::exit(1);
        });

    if !res.status().is_success() {
        Log::error("HTTP error", &format!("{}", res.status()));
        std::process::exit(1);
    }

    let html = res.text().unwrap_or_default();
    extract_links(&html)
}

fn scrape_privatebin(url: &str) -> Vec<String> {
    let paste_id = privatebin_paste_id(url).unwrap_or_else(|| {
        Log::error("Could not extract paste ID", "check URL format");
        std::process::exit(1);
    });

    let key = privatebin_key(url).unwrap_or_else(|| {
        Log::error("Missing decryption key", "URL must have #fragment");
        std::process::exit(1);
    });

    let json = privatebin_fetch(&paste_id).unwrap_or_else(|e| {
        Log::error("Failed to fetch encrypted paste", &e);
        std::process::exit(1);
    });

    let decrypted = privatebin_decrypt(&json, &key).unwrap_or_else(|e| {
        Log::error("Failed to decrypt paste", &e);
        std::process::exit(1);
    });

    extract_links(&decrypted)
}

fn display_name(link: &str) -> String {
    if let Some(fragment) = link.split('#').nth(1) {
        fragment.to_string()
    } else {
        link.rsplit('/').next().unwrap_or(link).to_string()
    }
}

pub fn manual_input_mode() -> Vec<String> {
    println!();
    Log::info("Paste links below", "one per line, Ctrl+D when done");
    println!();

    let mut links = Vec::new();
    for line in io::stdin().lock().lines() {
        match line {
            Ok(l) => {
                let trimmed = l.trim().to_string();
                if !trimmed.is_empty() {
                    links.push(trimmed);
                }
            }
            Err(e) => {
                Log::error("Read error", &e.to_string());
                break;
            }
        }
    }
    links
}

pub fn auto_scrape_mode() -> Vec<String> {
    let url = Log::input("Enter URL to scrape links from");
    Log::info("Fetching", &url);

    let all_links = scrape_links(&url);

    if all_links.is_empty() {
        Log::error("No fuckingfast.co links found", "check the URL");
        std::process::exit(1);
    }

    Log::info("Found links", &all_links.len().to_string());

    let items: Vec<String> = all_links
        .iter()
        .map(|link| {
            let name = display_name(link);
            let short = if link.len() > 80 {
                format!("{}...", &link[..77])
            } else {
                link.clone()
            };
            format!("{}  [{}]", short, name.bright_cyan())
        })
        .collect();

    println!();
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .items(&items)
        .with_prompt("SPACE to toggle, ↑↓ to move, ENTER to confirm")
        .interact()
        .unwrap_or_else(|e| {
            Log::error("Selection failed", &e.to_string());
            std::process::exit(1);
        });

    if selections.is_empty() {
        Log::warning("No links selected", "nothing to save");
        return Vec::new();
    }

    selections.iter().map(|&i| all_links[i].clone()).collect()
}

pub fn choose_input_mode() -> Vec<String> {
    println!(
        "{}",
        "How would you like to provide download links?"
            .bright_cyan()
            .bold()
    );
    println!(
        "  {}  {} — type or paste links into the terminal",
        "1".bright_green(),
        "Manual".bold()
    );
    println!(
        "  {}  {} — fetch links from a URL and pick which to download",
        "2".bright_green(),
        "Auto-scrape".bold()
    );
    println!();

    let choice = Log::input("Enter choice (1 or 2)");

    match choice.trim() {
        "2" => auto_scrape_mode(),
        _ => manual_input_mode(),
    }
}

pub fn write_input_file(links: &[String]) {
    if links.is_empty() {
        Log::warning("No links", "nothing to save");
        return;
    }

    let output = links.join("\n");
    std::fs::write("input.txt", &output).unwrap_or_else(|e| {
        Log::error("Failed to write input.txt", &e.to_string());
        std::process::exit(1);
    });

    let _ = arboard::Clipboard::new().map(|mut cb| cb.set_text(&output));

    println!();
    for link in links {
        println!("  {} {}", "✓".bright_green(), link.bright_cyan());
    }
    println!();
    Log::success("Saved", &format!("{} links to input.txt", links.len()));
}

// ---- GUI download helpers ----

use std::path::PathBuf;

#[derive(Clone)]
pub struct DownloadItem {
    pub link: String,
    pub file_name: String,
    pub status: DownloadStatus,
}

#[derive(Clone)]
pub enum DownloadStatus {
    Pending,
    FetchingPage,
    Downloading { total: u64, downloaded: u64, speed: f64 },
    Done,
    Failed(String),
}

pub static HEADERS: &[(&str, &str)] = &[
    ("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
    ("accept-language", "en-US,en;q=0.5"),
    ("referer", "https://fitgirl-repacks.site/"),
    ("sec-ch-ua", "\"Brave\";v=\"131\", \"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\""),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", "\"Windows\""),
    ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"),
];

fn get_download_url(html: &str) -> Option<String> {
    let doc = scraper::Html::parse_document(html);
    let script_sel = scraper::Selector::parse("script").unwrap();
    let re = Regex::new(r#"window\.open\(["'](https?://[^\s"'\)]+)"#).unwrap();
    for script in doc.select(&script_sel) {
        let text = script.text().collect::<String>();
        if text.contains("function download") {
            if let Some(cap) = re.captures(&text) {
                return Some(cap.get(1).unwrap().as_str().to_string());
            }
        }
    }
    None
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub fn download_all(
    links: Vec<String>,
    game_name: String,
    items: Arc<Mutex<Vec<DownloadItem>>>,
    pause_flag: Arc<AtomicBool>,
) {
    let downloads_folder = format!("downloads/{game_name}");
    let _ = std::fs::create_dir_all(&downloads_folder);

    let client = reqwest::blocking::Client::builder()
        .default_headers(
            HEADERS
                .iter()
                .map(|(k, v)| {
                    (
                        reqwest::header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                        reqwest::header::HeaderValue::from_str(v).unwrap(),
                    )
                })
                .collect(),
        )
        .build()
        .unwrap();

    for (i, link) in links.iter().enumerate() {
        let res = match client.get(link).send() {
            Ok(r) => r,
            Err(e) => {
                items.lock().unwrap().get_mut(i).unwrap().status =
                    DownloadStatus::Failed(format!("HTTP: {e}"));
                continue;
            }
        };
        let status = res.status();
        if !status.is_success() {
            items.lock().unwrap().get_mut(i).unwrap().status =
                DownloadStatus::Failed(format!("HTTP {status}"));
            continue;
        }

        let html = res.text().unwrap_or_default();

        let soup = scraper::Html::parse_document(&html);
        let meta_sel = scraper::Selector::parse(r#"meta[name="title"]"#).unwrap();
        let file_name = soup
            .select(&meta_sel)
            .next()
            .and_then(|el| el.value().attr("content"))
            .unwrap_or("default_file_name")
            .to_string();

        {
            let mut guard = items.lock().unwrap();
            let item = guard.get_mut(i).unwrap();
            item.file_name = file_name.clone();
            item.status = DownloadStatus::FetchingPage;
        }

        let download_url = match get_download_url(&html) {
            Some(u) => u,
            None => {
                items.lock().unwrap().get_mut(i).unwrap().status =
                    DownloadStatus::Failed("no download URL found".into());
                continue;
            }
        };

        let output_path = PathBuf::from(&downloads_folder).join(&file_name);
        let mut resp = match client.get(&download_url).send() {
            Ok(r) => r,
            Err(e) => {
                items.lock().unwrap().get_mut(i).unwrap().status =
                    DownloadStatus::Failed(format!("download: {e}"));
                continue;
            }
        };

        let total = resp.content_length().unwrap_or(0);
        let start = Instant::now();
        {
            let mut guard = items.lock().unwrap();
            let item = guard.get_mut(i).unwrap();
            item.status = DownloadStatus::Downloading { total, downloaded: 0, speed: 0.0 };
        }

        let mut file = match std::fs::File::create(&output_path) {
            Ok(f) => f,
            Err(e) => {
                items.lock().unwrap().get_mut(i).unwrap().status =
                    DownloadStatus::Failed(format!("file: {e}"));
                continue;
            }
        };

        let mut buf = vec![0u8; 262144];
        let mut downloaded: u64 = 0;
        let mut chunks: u32 = 0;
        loop {
            while pause_flag.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            let n = match resp.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    items.lock().unwrap().get_mut(i).unwrap().status =
                        DownloadStatus::Failed(format!("read: {e}"));
                    break;
                }
            };
            if file.write_all(&buf[..n]).is_err() {
                items.lock().unwrap().get_mut(i).unwrap().status =
                    DownloadStatus::Failed("write error".into());
                break;
            }
            downloaded += n as u64;
            chunks += 1;
            if chunks % 4 == 0 {
                let elapsed = start.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 { downloaded as f64 / elapsed } else { 0.0 };
                let mut guard = items.lock().unwrap();
                guard[i].status = DownloadStatus::Downloading { total, downloaded, speed };
            }
        }

        let mut guard = items.lock().unwrap();
        let item = guard.get_mut(i).unwrap();
        match &item.status {
            DownloadStatus::Failed(_) => {}
            _ => {
                item.status = DownloadStatus::Done;
                drop(guard);
                remove_link(link);
            }
        }
    }
}

pub fn remove_link(link: &str) {
    let Ok(content) = std::fs::read_to_string("input.txt") else { return };
    let mut remaining = Vec::new();
    let mut removed = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == link {
            removed = true;
        } else if !trimmed.is_empty() {
            remaining.push(trimmed.to_string());
        }
    }
    if removed {
        let _ = std::fs::write("input.txt", remaining.join("\n") + "\n");
    }
}
