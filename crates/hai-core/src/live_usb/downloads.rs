//! Downloads and caches the official Alpine ISO and the HAOS image.
//!
//! Cache layout, inside the installer's cache folder:
//! `live-usb/alpine/<iso>` and `live-usb/haos/<version>/<image>.img.xz`.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::download::{expected_sha256, parse_version};
use crate::error::{Error, Result};
use crate::{FlashProgress, FlashStage, HaosImage, ImageFormat, ProgressCallback, ReleaseSource};

/// Pinned Alpine release. Bumping it means updating the name, URL, checksum and size below.
pub const ALPINE_VERSION: &str = "3.24.2";
const ALPINE_ISO_NAME: &str = "alpine-standard-3.24.2-x86_64.iso";
// [China mirror] The same pinned file, in the order we try it: China's
// mirrors first, the project's own CDN last. Whichever answers must match the
// checksum below, so a slow or blocked source only costs a retry.
const ALPINE_ISO_URLS: [&str; 3] = [
    "https://mirrors.ustc.edu.cn/alpine/v3.24/releases/x86_64/alpine-standard-3.24.2-x86_64.iso",
    "https://mirrors.tuna.tsinghua.edu.cn/alpine/v3.24/releases/x86_64/alpine-standard-3.24.2-x86_64.iso",
    "https://dl-cdn.alpinelinux.org/alpine/v3.24/releases/x86_64/alpine-standard-3.24.2-x86_64.iso",
];
/// From Alpine's published `.sha256` file for this ISO.
const ALPINE_ISO_SHA256: &str = "20c026e3a788bfb75fc8b50a54bcc12aee85e3c75909740bba6a6f4563d63296";
const ALPINE_ISO_SIZE: u64 = 370_147_328;

/// HAOS board name for generic x86-64 PCs.
pub const HAOS_BOARD: &str = "generic-x86-64";

/// A verified file in the cache.
#[derive(Debug, Clone)]
pub struct Download {
    /// Location in the cache.
    pub path: PathBuf,
    /// Release version, for example `3.24.2` or `17.1`.
    pub version: String,
    /// Lowercase hex SHA-256 of the file.
    pub sha256: String,
}

/// The live USB flow's folder inside the installer's cache folder.
pub(crate) const FOLDER: &str = "live-usb";

/// The live USB cache folder, created if missing.
pub fn cache_dir<B: ReleaseSource>(backend: &B) -> Result<PathBuf> {
    // A folder of its own, so `remove_cache` can take it away as a whole
    let dir = backend.cache_dir()?.join(FOLDER);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Returns the pinned Alpine ISO from `cache`, downloading and verifying it if needed.
pub async fn fetch_alpine<B: ReleaseSource, P: ProgressCallback>(
    backend: &B,
    cache: &Path,
    progress: &P,
) -> Result<Download> {
    let path = cache.join("alpine").join(ALPINE_ISO_NAME);
    // [China mirror] A source that is down must not end the flow; the next one
    // carries the same file. Every attempt is verified, so none can slip a
    // wrong file in. A cancelled run still stops at the first failure.
    let mut last_failure = None;
    for url in ALPINE_ISO_URLS {
        // hai-core's downloader takes a HaosImage; it's only used here as "URL + size + digest".
        let image = HaosImage {
            board: "alpine".into(),
            format: ImageFormat::Raw,
            download_url: url.into(),
            size: ALPINE_ISO_SIZE,
            digest: Some(format!("sha256:{ALPINE_ISO_SHA256}")),
        };
        match fetch(backend, &image, &path, Some(ALPINE_ISO_SHA256), progress).await {
            Ok(()) => {
                last_failure = None;
                break;
            }
            Err(error) if progress.is_cancelled() => return Err(error),
            Err(error) => last_failure = Some(error),
        }
    }
    if let Some(error) = last_failure {
        return Err(error);
    }
    Ok(Download {
        path,
        version: ALPINE_VERSION.into(),
        sha256: ALPINE_ISO_SHA256.into(),
    })
}

/// Returns the latest stable HAOS generic x86-64 image (still xz-compressed) from `cache`,
/// downloading and verifying it if needed. Needs network to look up the latest version.
pub async fn fetch_haos<B: ReleaseSource, P: ProgressCallback>(
    backend: &B,
    cache: &Path,
    progress: &P,
) -> Result<Download> {
    let release = backend
        .get_latest_haos_release_for_board(HAOS_BOARD)
        .await?;
    let version = release.version.clone();
    // The version becomes a folder name.
    if parse_version(&version).is_none() {
        return Err(Error::ReleaseUnusable(format!(
            "unexpected version {version:?}"
        )));
    }
    let image = release.require_image(HAOS_BOARD, ImageFormat::Raw)?;
    // [China mirror] The mirror does not carry the release's signed
    // SHA256SUMS, so a mirrored image may have no digest. The download then
    // relies on HTTPS and the XZ integrity checks, and the hash below is
    // taken from the file that is kept.
    let referenced = expected_sha256(image).ok().map(hex::encode);

    let path = cache
        .join("haos")
        .join(&version)
        .join(format!("haos_{HAOS_BOARD}-{version}.img.xz"));
    let sha256 = match referenced {
        Some(sha256) => {
            fetch(backend, image, &path, Some(&sha256), progress).await?;
            sha256
        }
        // No digest to compare against; download, then hash what was kept so
        // the cache marker and the `.sha256` on the stick still match it.
        None => {
            fetch(backend, image, &path, None, progress).await?;
            sha256_with_progress(&path, image.size, progress).await?
        }
    };
    Ok(Download {
        path,
        version,
        sha256,
    })
}

async fn fetch<B: ReleaseSource, P: ProgressCallback>(
    backend: &B,
    image: &HaosImage,
    path: &Path,
    sha256: Option<&str>,
    progress: &P,
) -> Result<()> {
    if is_cached(path, image.size, sha256, progress).await? {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // What the other flows left goes before this build's own files
    let downloads = backend.cache_dir()?;
    crate::download::make_room(&downloads, &downloads.join(FOLDER), image.size).await?;
    // Verifies size and SHA-256, and only moves the file into place once both match.
    backend.download_image(image, path, progress).await?;
    Ok(())
}

/// Whether `path` exists with exactly `size` bytes and the given SHA-256.
/// Without a known SHA-256, the size alone counts as cached.
async fn is_cached<P: ProgressCallback>(
    path: &Path,
    size: u64,
    sha256: Option<&str>,
    progress: &P,
) -> Result<bool> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() == size => {}
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    }
    let Some(sha256) = sha256 else {
        // [China mirror] No digest for this image; the download itself did
        // the XZ integrity checks.
        return Ok(true);
    };
    let actual = sha256_with_progress(path, size, progress).await?;
    Ok(actual.eq_ignore_ascii_case(sha256))
}

/// Hashing a cached HAOS image takes several seconds, so it reports progress like a download
/// and stops with `Error::Cancelled` when asked to.
async fn sha256_with_progress<P: ProgressCallback>(
    path: &Path,
    size: u64,
    progress: &P,
) -> Result<String> {
    use tokio::io::AsyncReadExt;
    const REPORT_EVERY: u64 = 32 * 1024 * 1024;

    let report = |done: u64| {
        progress.on_progress(FlashProgress {
            stage: FlashStage::Downloading,
            progress: (done.saturating_mul(100) / size.max(1)).min(100) as u8,
            bytes_processed: done,
            total_bytes: size,
            message: "Checking the earlier download...".to_string(),
        });
    };
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let (mut done, mut next_report) = (0u64, 0u64);
    loop {
        if progress.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if done >= next_report {
            report(done);
            next_report = done + REPORT_EVERY;
        }
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            report(done);
            return Ok(hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
        done += read as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoOpProgress;
    use std::sync::Mutex;

    const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    /// [China mirror] Stands in for the network: records the source each
    /// download comes from, and refuses the first one like a mirror that is
    /// down while the rest serve the pinned file.
    #[derive(Default)]
    struct Sources {
        tried: Mutex<Vec<String>>,
        failures: usize,
    }

    impl ReleaseSource for Sources {
        async fn check_connection(&self) -> Result<()> {
            unreachable!()
        }
        async fn get_device_manifest(&self) -> Result<crate::DeviceManifest> {
            unreachable!()
        }
        async fn get_haos_release(&self, _: &str) -> Result<crate::HaosRelease> {
            unreachable!()
        }
        async fn get_latest_haos_release_for_board(&self, _: &str) -> Result<crate::HaosRelease> {
            unreachable!()
        }
        async fn download_image<P: ProgressCallback>(
            &self,
            image: &HaosImage,
            dest_path: &Path,
            _: &P,
        ) -> Result<()> {
            let mut tried = self.tried.lock().unwrap();
            tried.push(image.download_url.clone());
            if tried.len() <= self.failures {
                return Err(crate::Error::DownloadFailed("the mirror is down".into()));
            }
            std::fs::write(dest_path, b"alpine")?;
            Ok(())
        }
        async fn extract_xz<P: ProgressCallback>(&self, _: &Path, _: &Path, _: &P) -> Result<()> {
            unreachable!()
        }
        fn cache_dir(&self) -> Result<PathBuf> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn a_source_that_is_down_is_replaced_by_the_next_one() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Sources {
            failures: 1,
            ..Sources::default()
        };
        fetch_alpine(&backend, dir.path(), &NoOpProgress)
            .await
            .unwrap();
        assert_eq!(
            *backend.tried.lock().unwrap(),
            ALPINE_ISO_URLS[..2].to_vec(),
            "the first source is retried on the second"
        );
    }

    #[tokio::test]
    async fn every_source_failing_reports_the_last_failure() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Sources {
            failures: ALPINE_ISO_URLS.len(),
            ..Sources::default()
        };
        assert!(fetch_alpine(&backend, dir.path(), &NoOpProgress)
            .await
            .is_err());
        assert_eq!(backend.tried.lock().unwrap().len(), ALPINE_ISO_URLS.len());
    }

    #[tokio::test]
    async fn a_cancelled_run_does_not_ask_another_source() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Sources {
            failures: usize::MAX,
            ..Sources::default()
        };
        assert!(fetch_alpine(&backend, dir.path(), &Cancelled)
            .await
            .is_err());
        assert_eq!(
            backend.tried.lock().unwrap().len(),
            1,
            "a stop ends the flow instead of moving to the next mirror"
        );
    }

    #[tokio::test]
    async fn cached_file_with_matching_size_and_hash_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        assert!(is_cached(&path, 5, Some(HELLO_SHA256), &NoOpProgress)
            .await
            .unwrap());
        assert!(
            is_cached(&path, 5, Some(&HELLO_SHA256.to_uppercase()), &NoOpProgress)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn cached_file_with_a_known_size_but_no_digest_is_reused() {
        // [China mirror] Mirror images carry no digest; the size alone then
        // decides, and the caller still learns the file's hash afterwards.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        assert!(
            is_cached(&path, 5, None, &NoOpProgress).await.unwrap(),
            "size matches with no digest"
        );
        assert!(
            !is_cached(&path, 6, None, &NoOpProgress).await.unwrap(),
            "size mismatch with no digest"
        );
    }

    #[tokio::test]
    async fn cached_file_is_not_reused_on_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hellO").unwrap();
        assert!(
            !is_cached(&path, 5, Some(HELLO_SHA256), &NoOpProgress)
                .await
                .unwrap(),
            "wrong hash"
        );
        assert!(
            !is_cached(&path, 6, Some(HELLO_SHA256), &NoOpProgress)
                .await
                .unwrap(),
            "wrong size"
        );
        assert!(!is_cached(
            &dir.path().join("missing"),
            5,
            Some(HELLO_SHA256),
            &NoOpProgress
        )
        .await
        .unwrap());
    }

    struct Recorder(Mutex<Vec<(u64, u64)>>);

    impl ProgressCallback for Recorder {
        fn on_progress(&self, progress: FlashProgress) {
            let mut seen = self.0.lock().unwrap();
            seen.push((progress.bytes_processed, progress.total_bytes));
        }
    }

    #[tokio::test]
    async fn checking_a_cached_file_reports_progress_up_to_its_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        let recorder = Recorder(Mutex::new(Vec::new()));
        assert!(is_cached(&path, 5, Some(HELLO_SHA256), &recorder)
            .await
            .unwrap());
        let seen = recorder.0.into_inner().unwrap();
        assert_eq!(seen.first(), Some(&(0, 5)));
        assert_eq!(seen.last(), Some(&(5, 5)));
    }

    struct Cancelled;

    impl ProgressCallback for Cancelled {
        fn on_progress(&self, _: FlashProgress) {}

        fn is_cancelled(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn checking_a_cached_file_stops_when_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        std::fs::write(&path, b"hello").unwrap();
        let error = is_cached(&path, 5, Some(HELLO_SHA256), &Cancelled)
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Cancelled), "{error:?}");
    }

    #[test]
    fn only_versions_that_are_safe_folder_names_are_accepted() {
        for ok in ["17.1", "2026.10.0"] {
            assert!(parse_version(ok).is_some(), "{ok}");
        }
        for bad in [
            "", "16", "..", "../17.1", "17.1/..", "17.1-rc1", ".17", "17..1", "17.1\\x",
        ] {
            assert!(parse_version(bad).is_none(), "{bad}");
        }
    }
}
