//! Installer images for Home Assistant hardware by Nabu Casa
//!
//! The Green and Yellow are reset with a small installer image instead of
//! Home Assistant OS itself. Booted once, the installer writes Home Assistant
//! OS to the device's own storage. [China mirror] The same pattern covers the
//! generic x86-64 installer ISO published by ota.hasscn.top, which lets a PC
//! install Home Assistant OS to its own internal drive from a USB stick.
//!
//! NabuCasa/buildroot-installer has no "latest" release API like the HAOS
//! version service, so these images are pinned on purpose. GitHub publishes no
//! digest for these older release assets: the SHA-256 digests below were
//! computed from the published files. They go through the same download path
//! as HAOS images, which checks the size and digest before extraction. The
//! x86 installer is served by the China mirror without a digest; HTTPS and the
//! installer's own behavior stand in for that missing check.

use crate::types::{HaosConfig, HaosImage, ImageFormat};

/// Nominal drive capacity an installer needs. The images are at most about
/// 500 MB, so this only keeps the storage checks meaningful: any card or
/// stick fits, as does the eMMC of a Yellow CM5. Recommended equals minimum,
/// because the installer never grows into the space.
const INSTALLER_STORAGE_BYTES: u64 = 1_000_000_000;

/// A pinned installer image for one Home Assistant hardware board.
#[derive(Debug, Clone, Copy)]
pub struct HardwareInstaller {
    /// Board identifier the frontend sends in a flash request.
    pub board: &'static str,
    pub download_url: &'static str,
    /// Size of the compressed `.img.xz` asset in bytes.
    pub size: u64,
    /// SHA-256 of the compressed asset, as `sha256:<hex>`, when the server
    /// publishes one. Without a digest the download skips verification.
    pub digest: Option<&'static str>,
    /// Size of the extracted raw image in bytes. For an uncompressed
    /// installer this is the image itself, byte for byte.
    pub extracted_size: u64,
    /// Whether the downloaded file is already the image to write, with no
    /// extraction in between. The x86 installer ships as a plain ISO.
    pub uncompressed: bool,
}

impl HardwareInstaller {
    /// The image as the download pipeline expects it.
    pub fn image(&self) -> HaosImage {
        HaosImage {
            board: self.board.to_string(),
            format: ImageFormat::Raw,
            download_url: self.download_url.to_string(),
            size: self.size,
            digest: self.digest.map(str::to_string),
        }
    }

    /// Storage requirements for the drive the installer is written to.
    pub fn storage(&self) -> HaosConfig {
        HaosConfig {
            board: self.board.to_string(),
            minimum_storage_bytes: INSTALLER_STORAGE_BYTES,
            recommended_storage_bytes: INSTALLER_STORAGE_BYTES,
        }
    }
}

pub static INSTALLERS: [HardwareInstaller; 3] = [
    HardwareInstaller {
        board: "green-installer",
        download_url: "https://github.com/NabuCasa/buildroot-installer/releases/download/green-installer-20240410/green-installer-20240410.img.xz",
        size: 38_254_492,
        digest: Some("sha256:bc2b72b518fab6b7bcbe8d5edcb1e1f55caae281b5c0856da8c939fa211a1422"),
        extracted_size: 76_627_968,
        uncompressed: false,
    },
    HardwareInstaller {
        board: "yellow-installer",
        download_url: "https://github.com/NabuCasa/buildroot-installer/releases/download/yellow-installer-20231025/yellow-installer-20231025.img.xz",
        size: 33_371_388,
        digest: Some("sha256:fa1c4fdd72d116527cf37a1d3b849c480c08fff373b4a6c8d0017dae7da3e3f2"),
        extracted_size: 68_157_440,
        uncompressed: false,
    },
    // The China mirror publishes a bootable HAOS installer ISO for generic
    // x86-64 machines; written to a USB stick, it installs HAOS onto the
    // machine's own storage. The mirror publishes no digest, and the size is
    // pinned like the Nabu Casa installers above: bump both when the mirror
    // updates the file.
    HardwareInstaller {
        board: "x86-installer",
        download_url: "https://ota.hasscn.top/haos-installer-amd64.iso",
        size: 501_219_328,
        digest: None,
        extracted_size: 501_219_328,
        uncompressed: true,
    },
];

/// The pinned installer for `board`, if it is an installer board.
pub fn find(board: &str) -> Option<&'static HardwareInstaller> {
    INSTALLERS.iter().find(|installer| installer.board == board)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installers_come_from_the_buildroot_installer_releases() {
        for installer in &INSTALLERS {
            let Some(url) = installer.download_url.strip_prefix("https://github.com/") else {
                continue;
            };
            let (repo, path) = url.split_once('/').unwrap();
            assert_eq!(repo, "NabuCasa/buildroot-installer");
            let (tag, file) = path.split_once('/').unwrap();
            assert!(tag.starts_with(installer.board), "{tag}");
            assert_eq!(file, format!("{tag}.img.xz"));
        }
    }

    #[test]
    fn the_x86_installer_is_a_whole_iso_from_the_china_mirror() {
        let installer = find("x86-installer").unwrap();
        assert!(installer.uncompressed);
        assert_eq!(
            installer.download_url,
            "https://ota.hasscn.top/haos-installer-amd64.iso"
        );
        assert_eq!(installer.digest, None);
        assert_eq!(installer.extracted_size, installer.size);
    }

    #[test]
    fn installer_digests_are_accepted_by_the_download_check() {
        for installer in &INSTALLERS {
            let Some(digest) = installer.digest else {
                continue;
            };
            let expected = crate::download::expected_sha256(&installer.image()).unwrap();
            assert_eq!(
                digest,
                format!("sha256:{}", hex::encode(expected)),
                "{}",
                installer.board
            );
        }
    }

    #[test]
    fn installer_sizes_fit_the_storage_minimum() {
        for installer in &INSTALLERS {
            let storage = installer.storage();
            assert!(installer.size > 1_000_000, "{}", installer.board);
            // A compressed archive shrinks; an uncompressed ISO matches itself.
            assert!(
                installer.extracted_size >= installer.size,
                "{}",
                installer.board
            );
            assert!(installer.extracted_size < storage.minimum_reported_storage_bytes());
            assert_eq!(
                storage.minimum_storage_bytes,
                storage.recommended_storage_bytes
            );
        }
    }

    #[test]
    fn installer_boards_are_unique_and_never_haos_boards() {
        let manifest = crate::manifest::bundled_manifest();
        for installer in &INSTALLERS {
            assert_eq!(find(installer.board).unwrap().digest, installer.digest);
            assert!(manifest
                .devices
                .iter()
                .all(|device| device.haos.board != installer.board));
        }
        assert!(find("green").is_none());
        assert!(find("odroid-n2").is_none());
    }

    #[test]
    fn installer_image_is_a_raw_image_with_a_digest() {
        let image = find("yellow-installer").unwrap().image();
        assert_eq!(image.format, ImageFormat::Raw);
        assert_eq!(image.size, 33_371_388);
        assert!(image.digest.is_some());
    }
}
