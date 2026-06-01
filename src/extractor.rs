use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum ExtractStatus {
    Pending,
    Extracting,
    Done,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct ArchiveItem {
    pub path: PathBuf,
    pub name: String,
    pub status: ExtractStatus,
}

#[derive(Clone, Debug)]
pub struct GameFolder {
    pub path: PathBuf,
    pub name: String,
    pub archives: Vec<ArchiveItem>,
}

static PART_RAR_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\.part(\d+)\.rar$").unwrap()
});

/// Scans the `downloads/` directory and returns every subfolder
/// that contains at least one `.rar` or `.zip` file.
pub fn search_games() -> Vec<GameFolder> {
    let mut folders = Vec::new();
    let downloads = Path::new("downloads");
    if !downloads.exists() {
        return folders;
    }

    let Ok(entries) = std::fs::read_dir(downloads) else { return folders };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let archives = scan_archives(&path);
        if archives.is_empty() {
            continue;
        }

        folders.push(GameFolder { path, name, archives });
    }

    // sort alphabetically
    folders.sort_by(|a, b| a.name.cmp(&b.name));
    folders
}

/// Scans a single directory for RAR/ZIP archive files.
/// For multi-part RARs (`.part01.rar`, `.part02.rar` …) only the
/// first part is kept – the user only needs to extract that one.
pub fn scan_archives(dir: &Path) -> Vec<ArchiveItem> {
    let mut items = Vec::new();
    if !dir.exists() {
        return items;
    }

    let mut rar_groups: std::collections::HashMap<String, Vec<(u32, PathBuf)>> =
        std::collections::HashMap::new();
    let mut zip_files: Vec<PathBuf> = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else { return items };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if name.ends_with(".rar") {
            if let Some(caps) = PART_RAR_RE.captures(&name) {
                let num: u32 = caps[1].parse().unwrap_or(0);
                let base = PART_RAR_RE.replace(&name, "").to_string();
                rar_groups.entry(base).or_default().push((num, path));
            } else {
                rar_groups.entry(name).or_default().push((0, path));
            }
        } else if name.ends_with(".zip") {
            zip_files.push(path);
        }
    }

    for (_base, mut parts) in rar_groups {
        parts.sort_by_key(|(n, _)| *n);
        if let Some((_, first)) = parts.into_iter().next() {
            let name = first
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            items.push(ArchiveItem {
                path: first,
                name,
                status: ExtractStatus::Pending,
            });
        }
    }

    for zip_path in zip_files {
        let name = zip_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        items.push(ArchiveItem {
            path: zip_path,
            name,
            status: ExtractStatus::Pending,
        });
    }

    items
}

/// Extract a single archive **in-place** – files land next to the archive.
pub fn extract_archive(
    archive: &Path,
    items: &Arc<Mutex<Vec<ArchiveItem>>>,
    idx: usize,
    cancel: &AtomicBool,
) {
    let ext = archive
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // each archive extracts into a subdirectory named after itself (no extension)
    let dest = {
        let stem = archive
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("extracted");
        archive.parent().unwrap_or(Path::new(".")).join(stem)
    };
    let _ = std::fs::create_dir_all(&dest);

    match ext.as_str() {
        "rar" => extract_rar_cmd(archive, &dest, items, idx, cancel),
        "zip" => extract_zip_crate(archive, &dest, items, idx, cancel),
        _ => {
            let mut guard = items.lock().unwrap();
            guard[idx].status = ExtractStatus::Failed("unsupported format".into());
        }
    }
}

/// Extract a RAR by calling the system `unrar` command.
/// Falls back to `unrar-free` if `unrar` is not found.
fn extract_rar_cmd(
    path: &Path,
    dest: &Path,
    items: &Arc<Mutex<Vec<ArchiveItem>>>,
    idx: usize,
    cancel: &AtomicBool,
) {
    let cmd = if which("unrar") { "unrar" } else { "unrar-free" };

    let output = std::process::Command::new(cmd)
        .args([
            "x",          // extract with full path
            "-o+",        // overwrite existing files
            "-y",         // assume yes
        ])
        .arg(path)
        .arg(dest)
        .output();

    if cancel.load(Ordering::Relaxed) {
        let mut guard = items.lock().unwrap();
        guard[idx].status = ExtractStatus::Failed("cancelled".into());
        return;
    }

    match output {
        Ok(out) if out.status.success() => {
            let mut guard = items.lock().unwrap();
            guard[idx].status = ExtractStatus::Done;
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let mut guard = items.lock().unwrap();
            guard[idx].status = ExtractStatus::Failed(format!("{cmd}: {stderr}"));
        }
        Err(e) => {
            let mut guard = items.lock().unwrap();
            guard[idx].status =
                ExtractStatus::Failed(format!("{cmd} not found: {e}"));
        }
    }
}

/// Extract a ZIP file using the pure-Rust `zip` crate.
fn extract_zip_crate(
    path: &Path,
    dest: &Path,
    items: &Arc<Mutex<Vec<ArchiveItem>>>,
    idx: usize,
    cancel: &AtomicBool,
) {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            let mut guard = items.lock().unwrap();
            guard[idx].status = ExtractStatus::Failed(format!("open: {e}"));
            return;
        }
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            let mut guard = items.lock().unwrap();
            guard[idx].status = ExtractStatus::Failed(format!("read: {e}"));
            return;
        }
    };

    let total = archive.len();
    for i in 0..total {
        if cancel.load(Ordering::Relaxed) {
            let mut guard = items.lock().unwrap();
            guard[idx].status = ExtractStatus::Failed("cancelled".into());
            return;
        }

        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                let mut guard = items.lock().unwrap();
                guard[idx].status = ExtractStatus::Failed(format!("entry {i}: {e}"));
                return;
            }
        };

        let out_path = dest.join(entry.name());
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&out_path);
        } else if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
            if let Ok(mut out) = std::fs::File::create(&out_path) {
                let _ = std::io::copy(&mut entry, &mut out);
            }
        }
    }

    let mut guard = items.lock().unwrap();
    guard[idx].status = ExtractStatus::Done;
}

/// Thin wrapper around `which` to check if a command exists on PATH.
fn which(cmd: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .is_ok_and(|o| o.status.success())
}
