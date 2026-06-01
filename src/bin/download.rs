use std::io::{Read, Write};
use std::path::Path;

use colored::Colorize;

use fitgirl_auto_downloader_mori::{choose_input_mode, clear_screen, timestamp, Log, write_input_file};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use scraper::Html;
use url::Url;

const HEADERS: &[(&str, &str)] = &[
    ("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
    ("accept-language", "en-US,en;q=0.5"),
    ("referer", "https://fitgirl-repacks.site/"),
    ("sec-ch-ua", "\"Brave\";v=\"131\", \"Chromium\";v=\"131\", \"Not_A Brand\";v=\"24\""),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", "\"Windows\""),
    ("user-agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"),
];

fn download_file(client: &reqwest::blocking::Client, url: &str, path: &Path) -> bool {
    let mut res = match client.get(url).send() {
        Ok(r) => r,
        Err(e) => {
            Log::error("Failed to start download", &e.to_string());
            return false;
        }
    };

    if !res.status().is_success() {
        Log::error("Failed To Download File", &res.status().to_string());
        return false;
    }

    let total_size = res.content_length().unwrap_or(0);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.bright_black} {msg} [{bar:32.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );
    pb.set_message(format!(
        "{} {} {} {} Downloading -> {}",
        timestamp().bright_black(),
        "»".bright_black(),
        "INFO".bright_blue(),
        "•".bright_black(),
        file_name,
    ));

    let mut file = match std::fs::File::create(path) {
        Ok(f) => f,
        Err(e) => {
            Log::error("Failed to create file", &e.to_string());
            pb.finish_and_clear();
            return false;
        }
    };

    let mut buf = [0u8; 8192];
    let mut downloaded: u64 = 0;
    loop {
        let n = match res.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                Log::error("Failed to read chunk", &e.to_string());
                pb.finish_and_clear();
                return false;
            }
        };
        if let Err(e) = file.write_all(&buf[..n]) {
            Log::error("Failed to write data", &e.to_string());
            pb.finish_and_clear();
            return false;
        }
        downloaded += n as u64;
        pb.set_position(downloaded);
    }

    pb.finish_and_clear();
    Log::success(
        "Successfully Downloaded File",
        &format!("{}...{}", &path.to_string_lossy()[..35.min(path.to_string_lossy().len())], &path.to_string_lossy()[55..].chars().take(path.to_string_lossy().len().saturating_sub(55)).collect::<String>()),
    );
    true
}

fn remove_link(processed_link: &str, input_file: &str) {
    let original = match std::fs::read_to_string(input_file) {
        Ok(c) => c,
        Err(_) => return,
    };

    let new_content: String = original
        .lines()
        .filter(|line| line.trim() != processed_link)
        .collect::<Vec<_>>()
        .join("\n");

    let _ = std::fs::write(input_file, &new_content);
}

fn get_download_url(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
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

fn main() {
    clear_screen();

    let input_file = "input.txt";
    let links = match std::fs::read_to_string(input_file) {
        Ok(c) => {
            let parsed: Vec<String> = c
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();
            if parsed.is_empty() {
                Log::warning("input.txt is empty", "create links now");
                let links = choose_input_mode();
                write_input_file(&links);
                links
            } else {
                parsed
            }
        }
        Err(_) => {
            Log::warning("input.txt not found", "create it now");
            let links = choose_input_mode();
            write_input_file(&links);
            links
        }
    };

    let first_game_link = links
        .iter()
        .find(|l| {
            Url::parse(l)
                .ok()
                .and_then(|u| u.fragment().map(|f| f.contains("fitgirl-repacks.site")))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| {
            Log::error(
                "Could not determine game name",
                "no fitgirl part files found in input.txt",
            );
            std::process::exit(1);
        });

    let parsed = Url::parse(first_game_link).unwrap();
    let fragment = parsed.fragment().unwrap_or("");
    let game_name = fragment.split("--").next().unwrap_or("").trim_matches('_');

    let downloads_folder = format!("downloads/{game_name}");
    if let Err(e) = std::fs::create_dir_all(&downloads_folder) {
        Log::error("Could not create download folder", &e.to_string());
        std::process::exit(1);
    }
    Log::info("Download folder", &downloads_folder);

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

    for link in &links {
        let display = if link.len() > 60 {
            format!("{}...{}", &link[..30], &link[60..])
        } else {
            link.clone()
        };
        Log::info("Started Processing", &display);

        let res = match client.get(link).send() {
            Ok(r) => r,
            Err(e) => {
                Log::error("Failed To Fetch Page", &e.to_string());
                continue;
            }
        };

        let status = res.status();
        if !status.is_success() {
            Log::error("Failed To Fetch Page", &status.to_string());
            continue;
        }

        let html = res.text().unwrap_or_default();

        let soup = Html::parse_document(&html);
        let meta_sel = scraper::Selector::parse(r#"meta[name="title"]"#).unwrap();
        let file_name = soup
            .select(&meta_sel)
            .next()
            .and_then(|el| el.value().attr("content"))
            .unwrap_or("default_file_name")
            .to_string();

        match get_download_url(&html) {
            Some(download_url) => {
                Log::info(
                    "Found Download Url",
                    &format!("{}...", &download_url[..70.min(download_url.len())]),
                );
                let output_path = Path::new(&downloads_folder).join(&file_name);
                if download_file(&client, &download_url, &output_path) {
                    remove_link(link, input_file);
                }
            }
            None => {
                Log::error("No Download Url Found", &status.to_string());
            }
        }
    }
}
