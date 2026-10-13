//! Verified private-chart storage. Only mp-local content is eligible for eviction.
use anyhow::{anyhow, ensure, Context, Result};
use prpr::info::ChartInfo;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_FILES: usize = 4096;
const MAX_EXPANDED: u64 = 128 * 1024 * 1024;
const CACHE_LIMIT: u64 = 512 * 1024 * 1024;

pub fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn relative(path: &str) -> Result<PathBuf> {
    let normalized = path.replace('\\', "/");
    ensure!(
        !normalized.is_empty()
            && normalized.split('/').all(|s| {
                let stem = s.split('.').next().unwrap_or_default().to_ascii_uppercase();
                !s.is_empty()
                    && s != "."
                    && s != ".."
                    && !s.ends_with(['.', ' '])
                    && !s.chars().any(|c| c.is_control() || "<>:\"|?*".contains(c))
                    && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                    && !(stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && matches!(stem.as_bytes()[3], b'1'..=b'9'))
            }),
        "unsafe or non-portable chart path"
    );
    Ok(PathBuf::from(normalized))
}

fn entries(root: &Path) -> Result<Vec<(String, PathBuf, u64)>> {
    ensure!(!fs::symlink_metadata(root)?.file_type().is_symlink(), "chart folder cannot be a symlink");
    let mut files = Vec::new();
    let mut size = 0u64;
    let mut count = 0usize;
    let mut names = HashSet::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false).min_depth(1) {
        let entry = entry?;
        count += 1;
        ensure!(count <= MAX_FILES, "chart has too many entries");
        ensure!(!entry.file_type().is_symlink(), "chart contains a symlink");
        if entry.file_type().is_dir() {
            continue;
        }
        ensure!(entry.file_type().is_file(), "chart contains a non-file entry");
        let path = entry
            .path()
            .strip_prefix(root)?
            .to_str()
            .ok_or_else(|| anyhow!("chart path is not UTF-8"))?
            .replace('\\', "/");
        relative(&path)?;
        ensure!(names.insert(path.to_lowercase()), "case-colliding chart paths");
        let len = entry.metadata()?.len();
        size = size.checked_add(len).ok_or_else(|| anyhow!("chart too large"))?;
        ensure!(size <= MAX_EXPANDED, "chart exceeds 128 MiB unpacked");
        files.push((path, entry.path().to_owned(), len));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

pub fn fingerprint(root: &Path) -> Result<String> {
    let files = entries(root)?;
    ensure!(!files.is_empty(), "empty chart");
    let mut hash = Sha256::new();
    hash.update(b"phira-private-chart-files-v1\0");
    for (name, path, size) in files {
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update(size.to_le_bytes());
        let mut file_hash = Sha256::new();
        let mut file = File::open(path)?.take(MAX_EXPANDED + 1);
        let mut buffer = [0u8; 65536];
        let mut count = 0u64;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            count += read as u64;
            file_hash.update(&buffer[..read]);
        }
        ensure!(count == size, "chart changed while hashing");
        hash.update(file_hash.finalize());
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn cache_root(charts: &Path) -> Result<PathBuf> {
    let root = charts.join("mp-local");
    fs::create_dir_all(&root)?;
    ensure!(!fs::symlink_metadata(&root)?.file_type().is_symlink(), "cache root cannot be a symlink");
    Ok(root)
}

fn touch(root: &Path, hash: &str) -> Result<()> {
    fs::write(root.join(format!("{hash}.access")), b"")?;
    Ok(())
}

pub fn find(charts: &Path, hash: &str, saved: &[String], warnings: &mut Vec<String>) -> Result<Option<String>> {
    ensure!(valid_hash(hash), "invalid content hash");
    if let Some(path) = find_cached(charts, hash, warnings)? {
        return Ok(Some(path));
    }
    find_saved(charts, hash, saved, warnings)
}

fn find_saved(charts: &Path, hash: &str, saved: &[String], warnings: &mut Vec<String>) -> Result<Option<String>> {
    // Saved entries are verified too: a user may have edited or deleted one.
    for path in saved {
        if path.starts_with(':') {
            continue;
        }
        let absolute = charts.join(relative(path)?);
        if !absolute.try_exists()? {
            continue;
        }
        match fingerprint(&absolute) {
            Ok(actual) if actual == hash => return Ok(Some(path.clone())),
            Ok(_) => (),
            Err(error) => warnings.push(format!("Could not verify saved chart {path}: {error:#}")),
        }
    }
    Ok(None)
}

fn find_cached(charts: &Path, hash: &str, warnings: &mut Vec<String>) -> Result<Option<String>> {
    let root = cache_root(charts)?;
    let target = root.join(hash);
    if target.try_exists()? {
        let reason = match fingerprint(&target) {
            Ok(actual) if actual == hash => {
                touch(&root, hash)?;
                return Ok(Some(format!("mp-local/{hash}")));
            }
            Ok(_) => "content checksum mismatch".to_owned(),
            Err(error) => format!("{error:#}"),
        };
        ensure!(!fs::symlink_metadata(&target)?.file_type().is_symlink(), "refusing to remove symlinked cache entry");
        warnings.push(format!("A damaged cached chart was removed and will be downloaded again: {reason}"));
        fs::remove_dir_all(&target).context("failed to remove damaged cache")?;
    }
    Ok(None)
}

fn prune(charts: &Path, protected: &[String], reserve: u64, limit: u64) -> Result<()> {
    let root = cache_root(charts)?;
    let mut total = 0u64;
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // A crashed extraction is never eligible for play; only one cache task runs at a time.
        if name.starts_with(".partial-") {
            fs::remove_dir_all(entry.path()).context("incomplete chart cleanup failed")?;
            continue;
        }
        // Never follow links or delete folders outside the managed cache.
        let size = entries(&entry.path())?.iter().map(|it| it.2).sum::<u64>();
        total = total.checked_add(size).ok_or_else(|| anyhow!("cache size overflow"))?;
        if !valid_hash(&name) || protected.contains(&format!("mp-local/{name}")) {
            continue;
        }
        let stamp = root.join(format!("{name}.access"));
        let modified = if stamp.try_exists()? {
            fs::metadata(&stamp)?.modified()?
        } else {
            entry.metadata()?.modified()?
        };
        candidates.push((modified, name, size));
    }
    candidates.sort_by_key(|it| it.0);
    for (_, name, size) in candidates {
        if total.saturating_add(reserve) <= limit {
            break;
        }
        fs::remove_dir_all(root.join(&name)).context("cache cleanup failed")?;
        let stamp = root.join(format!("{name}.access"));
        if stamp.try_exists()? {
            fs::remove_file(stamp)?;
        }
        total -= size;
    }
    ensure!(total.saturating_add(reserve) <= limit, "cache is full of protected or incomplete charts; free space in mp-local before retrying");
    Ok(())
}

pub fn install(charts: &Path, bytes: Vec<u8>, archive_hash: &str, expected: &str, protected: &[String]) -> Result<String> {
    ensure!(bytes.len() <= phira_mp_common::MAX_LOCAL_CHART_BYTES, "chart ZIP too large");
    ensure!(valid_hash(archive_hash), "invalid archive hash");
    ensure!(format!("{:x}", Sha256::digest(&bytes)) == archive_hash, "archive checksum mismatch");
    ensure!(expected.is_empty() || valid_hash(expected), "invalid content hash");
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    ensure!(zip.len() <= MAX_FILES, "too many ZIP entries");
    let mut expanded = 0u64;
    for i in 0..zip.len() {
        expanded = expanded.checked_add(zip.by_index(i)?.size()).ok_or_else(|| anyhow!("ZIP too large"))?;
        ensure!(expanded <= MAX_EXPANDED, "ZIP expands beyond 128 MiB");
    }
    prune(charts, protected, expanded, CACHE_LIMIT)?;
    let root = cache_root(charts)?;
    let temp = tempfile::Builder::new().prefix(".partial-").tempdir_in(&root)?;
    let result = (|| -> Result<String> {
        let mut used = HashSet::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry.name().replace('\\', "/");
            let path = relative(name.trim_end_matches('/'))?;
            ensure!(entry.unix_mode().is_none_or(|mode| mode & 0o170000 != 0o120000), "ZIP symlinks are forbidden");
            ensure!(used.insert(name.trim_end_matches('/').to_lowercase()), "duplicate ZIP path");
            let target = temp.path().join(path);
            if entry.is_dir() {
                fs::create_dir_all(target)?;
                continue;
            }
            total = total.checked_add(entry.size()).ok_or_else(|| anyhow!("ZIP too large"))?;
            ensure!(total <= MAX_EXPANDED, "ZIP expands beyond 128 MiB");
            fs::create_dir_all(target.parent().unwrap())?;
            let copied = std::io::copy(&mut (&mut entry).take(MAX_EXPANDED + 1), &mut File::create(target)?)?;
            ensure!(copied == entry.size(), "ZIP entry length mismatch");
        }
        // Our uploader puts info.yml at the archive root. Reject ambiguous layouts.
        let info: ChartInfo = serde_yaml::from_reader(File::open(temp.path().join("info.yml"))?)?;
        for file in [&info.chart, &info.music, &info.illustration].into_iter().chain(info.unlock_video.iter()) {
            ensure!(temp.path().join(relative(file)?).is_file(), "missing chart asset: {file}");
        }
        let hash = fingerprint(temp.path())?;
        ensure!(expected.is_empty() || hash == expected, "chart content hash differs from the room metadata");
        let target = root.join(&hash);
        if target.try_exists()? {
            ensure!(fingerprint(&target)? == hash, "existing cache is corrupted; retry the download");
        } else {
            fs::rename(temp.path(), &target)?;
        }
        touch(&root, &hash)?;
        Ok(format!("mp-local/{hash}"))
    })();
    // Keep both operation and cleanup errors visible.
    let cleanup = if temp.path().try_exists()? { temp.close() } else { Ok(()) };
    match (result, cleanup) {
        (Ok(path), Ok(())) => Ok(path),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error).context("temporary chart cleanup failed"),
        (Err(error), Err(cleanup)) => Err(error.context(format!("temporary chart cleanup also failed: {cleanup}"))),
    }
}

pub struct SavedChart {
    pub path: String,
    pub info: ChartInfo,
    pub already_saved: bool,
    pub warnings: Vec<String>,
}

pub fn save(charts: &Path, path: &str, saved: &[String]) -> Result<SavedChart> {
    let source = charts.join(relative(path)?);
    let hash = fingerprint(&source)?;
    let mut warnings = Vec::new();
    let existing = find_saved(charts, &hash, saved, &mut warnings)?;
    if let Some(existing) = existing.filter(|p| !p.starts_with("mp-local/")) {
        let info = serde_yaml::from_reader(File::open(charts.join(relative(&existing)?).join("info.yml"))?)?;
        return Ok(SavedChart {
            path: existing,
            info,
            already_saved: true,
            warnings,
        });
    }
    ensure!(path == format!("mp-local/{hash}"), "only a verified cached chart can be saved");
    fs::create_dir_all(charts.join("custom"))?;
    let mut target = format!("custom/mp-{hash}");
    if charts.join(&target).try_exists()? {
        if fingerprint(&charts.join(&target))? == hash {
            let info = serde_yaml::from_reader(File::open(charts.join(&target).join("info.yml"))?)?;
            return Ok(SavedChart {
                path: target,
                info,
                already_saved: true,
                warnings,
            });
        }
        target = format!("custom/mp-{hash}-{}", Uuid::new_v4());
    }
    let info = serde_yaml::from_reader(File::open(source.join("info.yml"))?)?;
    let stamp = cache_root(charts)?.join(format!("{hash}.access"));
    if stamp.try_exists()? {
        fs::remove_file(stamp).context("failed to remove cache access marker")?;
    }
    fs::rename(source, charts.join(&target)).context("failed to save chart; cached copy was retained")?;
    Ok(SavedChart {
        path: target,
        info,
        already_saved: false,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chart(path: &Path, music: &[u8]) {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join("info.yml"), "name: Test\nlevel: Test\nchart: chart.json\nmusic: music.ogg\nillustration: cover.png\n").unwrap();
        fs::write(path.join("chart.json"), "{}").unwrap();
        fs::write(path.join("music.ogg"), music).unwrap();
        fs::write(path.join("cover.png"), b"cover").unwrap();
    }
    #[test]
    fn fingerprint_is_order_independent_and_content_sensitive() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        chart(a.path(), b"music");
        chart(b.path(), b"music");
        assert_eq!(fingerprint(a.path()).unwrap(), fingerprint(b.path()).unwrap());
        fs::write(b.path().join("music.ogg"), b"changed").unwrap();
        assert_ne!(fingerprint(a.path()).unwrap(), fingerprint(b.path()).unwrap());
    }
    #[test]
    fn save_moves_without_duplication_and_detects_existing_copy() {
        let root = tempfile::tempdir().unwrap();
        let seed = tempfile::tempdir().unwrap();
        chart(seed.path(), b"music");
        let hash = fingerprint(seed.path()).unwrap();
        let path = format!("mp-local/{hash}");
        chart(&root.path().join(&path), b"music");
        let result = save(root.path(), &path, &[]).unwrap();
        let saved = result.path;
        assert!(!result.already_saved);
        assert!(!root.path().join(&path).exists());
        let same = save(root.path(), &saved, std::slice::from_ref(&saved)).unwrap();
        assert!(same.already_saved);
        assert_eq!(same.path, saved);
    }
    #[test]
    fn corrupt_cache_is_not_reused_and_saved_files_are_not_evicted() {
        let root = tempfile::tempdir().unwrap();
        let hash = "a".repeat(64);
        chart(&root.path().join(format!("mp-local/{hash}")), b"bad");
        let mut warnings = Vec::new();
        assert!(find(root.path(), &hash, &[], &mut warnings).unwrap().is_none());
        assert!(!warnings.is_empty());
        chart(&root.path().join("custom/saved"), b"saved");
        prune(root.path(), &[], 0, 0).unwrap();
        assert!(root.path().join("custom/saved/info.yml").exists());
    }
    fn archive(root: &Path, reverse: bool) -> Vec<u8> {
        use std::io::Write;
        let mut files = entries(root).unwrap();
        if reverse {
            files.reverse();
        }
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, path, _) in files {
            zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(&fs::read(path).unwrap()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn repacked_archives_reuse_the_same_content_and_saved_chart() {
        let root = tempfile::tempdir().unwrap();
        let seed = tempfile::tempdir().unwrap();
        chart(seed.path(), b"music");
        let hash = fingerprint(seed.path()).unwrap();
        let first = archive(seed.path(), false);
        let second = archive(seed.path(), true);
        assert_ne!(Sha256::digest(&first), Sha256::digest(&second));
        let path = install(root.path(), first.clone(), &format!("{:x}", Sha256::digest(&first)), &hash, &[]).unwrap();
        assert_eq!(find(root.path(), &hash, &[], &mut Vec::new()).unwrap(), Some(path.clone()));
        let same = install(root.path(), second.clone(), &format!("{:x}", Sha256::digest(&second)), &hash, &[]).unwrap();
        assert_eq!(path, same);
        let saved = save(root.path(), &path, &[]).unwrap().path;
        assert_eq!(find(root.path(), &hash, std::slice::from_ref(&saved), &mut Vec::new()).unwrap(), Some(saved.clone()));
        // A stale cache and already-saved library entry must not make a second library item.
        install(root.path(), first.clone(), &format!("{:x}", Sha256::digest(&first)), &hash, &[]).unwrap();
        let deduped = save(root.path(), &path, std::slice::from_ref(&saved)).unwrap();
        assert!(deduped.already_saved);
        assert_eq!(deduped.path, saved);
        // Edits are a different version and may never be mistaken for the original.
        fs::write(root.path().join(&saved).join("music.ogg"), b"edit").unwrap();
        assert!(find_saved(root.path(), &hash, &[saved], &mut Vec::new()).unwrap().is_none());
    }

    #[test]
    fn failed_integrity_or_extraction_never_publishes_partial_chart() {
        let root = tempfile::tempdir().unwrap();
        let seed = tempfile::tempdir().unwrap();
        chart(seed.path(), b"music");
        let bytes = archive(seed.path(), false);
        let digest = format!("{:x}", Sha256::digest(&bytes));
        assert!(install(root.path(), bytes.clone(), &"a".repeat(64), "", &[]).is_err());
        assert!(install(root.path(), bytes, &digest, &"b".repeat(64), &[]).is_err());
        assert_eq!(fs::read_dir(cache_root(root.path()).unwrap()).unwrap().count(), 0);
        fs::write(seed.path().join("info.yml"), "music: ../outside\n").unwrap();
        let bad = archive(seed.path(), false);
        assert!(install(root.path(), bad.clone(), &format!("{:x}", Sha256::digest(&bad)), "", &[]).is_err());
        assert_eq!(fs::read_dir(cache_root(root.path()).unwrap()).unwrap().count(), 0);
    }

    #[test]
    fn eviction_respects_active_chart_and_cleans_interrupted_extraction() {
        let root = tempfile::tempdir().unwrap();
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let active = format!("mp-local/{a}");
        chart(&root.path().join(&active), b"music");
        chart(&root.path().join(format!("mp-local/{b}")), b"music");
        chart(&root.path().join("mp-local/.partial-interrupted"), b"partial");
        let one = entries(&root.path().join(&active)).unwrap().iter().map(|e| e.2).sum();
        prune(root.path(), std::slice::from_ref(&active), 0, one).unwrap();
        assert!(root.path().join(&active).exists());
        assert!(!root.path().join(format!("mp-local/{b}")).exists());
        assert!(!root.path().join("mp-local/.partial-interrupted").exists());
        assert!(prune(root.path(), &[active], 1, one).is_err());
    }

    #[test]
    fn paths_and_hashes_are_bounded() {
        for path in ["../bad", "/bad", "C:/bad", "a/../bad"] {
            assert!(relative(path).is_err());
        }
        assert!(!valid_hash("../../bad"));
        assert!(valid_hash(&"a".repeat(64)));
    }
}
