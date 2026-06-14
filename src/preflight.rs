use std::ffi::CString;
use std::fs;
use std::mem::MaybeUninit;
use std::path::Path;
use anyhow::{anyhow, Result, Context};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootcStatus {
    pub api_version: String,
    pub kind: String,
    pub status: HostStatus,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    pub booted: Option<BootedStatus>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BootedStatus {
    pub ostree: Option<serde_json::Value>,
    pub composefs: Option<serde_json::Value>,
}

pub struct PreflightReport {
    pub is_bootc_ostree: bool,
    pub is_uefi: bool,
    pub esp_path: Option<String>,
    pub esp_free_space_bytes: u64,
    pub supports_reflink: bool,
    pub is_btrfs: bool,
}

pub fn get_free_space<P: AsRef<Path>>(path: P) -> Result<u64> {
    let path_str = path.as_ref().to_str().ok_or_else(|| anyhow!("invalid path"))?;
    let c_path = CString::new(path_str)?;
    let mut stats = MaybeUninit::<libc::statvfs>::uninit();
    let res = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if res < 0 {
        return Err(std::io::Error::last_os_error()).context("statvfs failed");
    }
    let stats = unsafe { stats.assume_init() };
    // f_frsize is fragment size, which is the actual block size. f_bavail is free blocks available to unprivileged users.
    let block_size = if stats.f_frsize > 0 { stats.f_frsize } else { stats.f_bsize };
    let free_space = block_size as u64 * stats.f_bavail as u64;
    Ok(free_space)
}

pub fn check_reflink_support<P: AsRef<Path>>(dir: P) -> bool {
    let src = dir.as_ref().join(".reflink_test_src");
    let dest = dir.as_ref().join(".reflink_test_dest");
    
    // Clean up first
    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&dest);
    
    let result = (|| -> Result<()> {
        fs::write(&src, b"test")?;
        crate::reflink::reflink(&src, &dest)?;
        Ok(())
    })();
    
    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&dest);
    
    result.is_ok()
}

pub fn run_preflight_checks() -> Result<PreflightReport> {
    // 1. Check bootc status
    let output = std::process::Command::new("bootc")
        .args(["status", "--json"])
        .output()
        .context("failed to run bootc status")?;
        
    let is_bootc_ostree = if output.status.success() {
        let status: BootcStatus = serde_json::from_slice(&output.stdout)
            .context("failed to parse bootc status json")?;
        if let Some(booted) = status.status.booted {
            booted.ostree.is_some()
        } else {
            false
        }
    } else {
        false
    };

    // 2. Check UEFI mode
    let is_uefi = Path::new("/sys/firmware/efi").exists();

    // 3. Locate ESP and check space
    let mut esp_path = None;
    let mut esp_free_space_bytes = 0;
    
    // Check common mount points for ESP
    for path in ["/boot/efi", "/efi", "/boot"] {
        if Path::new(path).exists() {
            // Check if it is FAT / VFAT
            if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
                let is_vfat = mounts.lines().any(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    parts.len() >= 3 && parts[1] == path && (parts[2] == "vfat" || parts[2] == "msdos")
                });
                if is_vfat {
                    esp_path = Some(path.to_string());
                    if let Ok(free_space) = get_free_space(path) {
                        esp_free_space_bytes = free_space;
                    }
                    break;
                }
            }
        }
    }

    // 4. Check filesystem type and reflink support
    // We check /sysroot
    let sysroot = "/sysroot";
    let mut is_btrfs = false;
    if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
        is_btrfs = mounts.lines().any(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts.len() >= 3 && parts[1] == sysroot && parts[2] == "btrfs"
        });
    }

    let supports_reflink = check_reflink_support(sysroot);

    Ok(PreflightReport {
        is_bootc_ostree,
        is_uefi,
        esp_path,
        esp_free_space_bytes,
        supports_reflink,
        is_btrfs,
    })
}
