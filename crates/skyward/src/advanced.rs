//! The whole-disk tools a device configuration offers in Advanced: export
//! its main disk to a computer, and repair the GPT tables it lists.

use crate::wipe;
use anyhow::{ensure, Context, Result};
use skyward_config::{DeviceConfig, Disk, Gpt};
use skyward_vab::Slot;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;

const BLKFLSBUF: libc::c_ulong = 0x1261;

/// GPT repair runs off the UI thread so the progress report paints and touches
/// continue to be handled while disks are released and gdisk writes tables.
pub struct GptWorker {
    events: Receiver<Result<Vec<String>, String>>,
    thread: Option<JoinHandle<()>>,
}

impl GptWorker {
    pub fn start(cfg: DeviceConfig, slot: Option<Slot>) -> Result<Self> {
        let (tx, events) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("gpt-repair".into())
            .spawn(move || {
                let _ = tx.send(repair_gpt(&cfg, slot).map_err(|e| format!("{e:#}")));
            })
            .context("start the GPT repair worker")?;
        Ok(Self { events, thread: Some(thread) })
    }

    pub fn done(&self) -> Option<Result<Vec<String>, String>> {
        self.events.try_recv().ok()
    }
}

impl Drop for GptWorker {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Export the whole main UFS LUN read/write and keep adb beside it. There is
/// deliberately no disable action: the recovery is restarted after a safe
/// eject, which takes the export away.
pub fn enable_mass_storage(cfg: &DeviceConfig, slot: Option<Slot>) -> Result<()> {
    let disk = configured(cfg)?;
    ensure!(!mass_storage_active(disk)?, "USB mass storage is already enabled");
    prepare_disk(disk, slot)?;
    export(disk)
}

/// Repair GPT on every disk the configuration lists. Both table copies are
/// moved to the ends of each disk; partition entries and data are not
/// changed.
pub fn repair_gpt(cfg: &DeviceConfig, slot: Option<Slot>) -> Result<Vec<String>> {
    let disk = configured(cfg)?;
    check_gpt(cfg)?;
    prepare_disk(disk, slot)?;
    if let Some(script) = &disk.gpt.prepare_script {
        let prepared = Command::new("/system/bin/sh").arg(script).status().with_context(|| format!("run {script}"))?;
        ensure!(prepared.success(), "{script} ended {prepared}");
    }
    let mut repaired = Vec::new();
    let mut failed = Vec::new();
    for name in &disk.gpt.disks {
        let device = PathBuf::from("/dev/block").join(name);
        if !fs::metadata(&device).is_ok_and(|metadata| metadata.file_type().is_block_device()) {
            continue;
        }
        let _ = skyward_sys::kmsg(&format!("repairing GPT on {}", device.display()));
        match fix_gpt_with_gdisk(&disk.gpt, &device) {
            Ok(()) => repaired.push(name.clone()),
            Err(error) => failed.push(format!("{name}: {error:#}")),
        }
    }
    unsafe { libc::sync() };
    ensure!(failed.is_empty(), "GPT repair failed on {}. Completed: {:?}. Restart recovery before mounting partitions.", failed.join(", "), repaired);
    ensure!(!repaired.is_empty(), "no UFS GPT disks were found");
    Ok(repaired)
}

pub fn check_gpt(cfg: &DeviceConfig) -> Result<()> {
    let disk = configured(cfg)?;
    ensure!(!mass_storage_active(disk)?, "USB mass storage must be disabled by restarting the recovery first");
    for tool in [&disk.gpt.gdisk, &disk.gpt.dmctl] {
        let tool = std::ffi::CString::new(tool.as_bytes()).context("tool path")?;
        ensure!(
            unsafe { libc::access(tool.as_ptr(), libc::X_OK) } == 0,
            "{} is unavailable or not executable",
            tool.to_string_lossy(),
        );
    }
    Ok(())
}

/// Run gdisk with the configured repair sequence.
fn fix_gpt_with_gdisk(gpt: &Gpt, device: &Path) -> Result<()> {
    let output = Command::new("/system/bin/sh")
        .args(["-c", "printf '%s' \"$1\" | \"$2\" \"$3\"", "sh", &gpt.input, &gpt.gdisk])
        .arg(device)
        .output()
        .with_context(|| format!("run gdisk on {}", device.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let _ = skyward_sys::kmsg(&format!(
        "gdisk {} ended {}: {}{}",
        device.display(),
        output.status,
        stdout,
        stderr
    ));
    ensure!(output.status.success(), "gdisk {} ended {}", device.display(), output.status);
    ensure!(
        stdout.contains(&gpt.success),
        "gdisk {} did not report a completed operation",
        device.display()
    );
    Ok(())
}

fn configured(cfg: &DeviceConfig) -> Result<&Disk> {
    cfg.disk.as_ref().context("whole-disk tools are not configured")
}

/// Let go of mappings that can keep a filesystem on the disk mounted, then
/// unmount what the configured script releases.
fn prepare_disk(disk: &Disk, slot: Option<Slot>) -> Result<()> {
    wipe::release_data().context("close the data partition")?;
    if let Some(slot) = slot {
        skyward_vab::unmount_partitions(slot).context("unmap the installed system's logical partitions")?;
    }
    unsafe { libc::sync() };
    let script = &disk.mass_storage.unmount_script;
    let unmounted = Command::new("/system/bin/sh").arg(script).status().with_context(|| format!("run {script}"))?;
    ensure!(unmounted.success(), "{script} ended {unmounted}");
    unsafe { libc::sync() };
    // Like the reference, try to invalidate the kernel's cache for the LUN.
    if let Ok(file) = fs::File::open(&disk.device) {
        unsafe { libc::ioctl(file.as_raw_fd(), BLKFLSBUF as _, 0) };
    }
    Ok(())
}

fn export(disk: &Disk) -> Result<()> {
    ensure!(Path::new(&disk.gadget).exists(), "{} is not mounted", disk.gadget);
    let script = &disk.mass_storage.start_script;
    let output = Command::new("/system/bin/sh")
        .arg(script)
        .output()
        .with_context(|| format!("run {script}"))?;
    ensure!(
        output.status.success(),
        "{script} ended {}: {}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    skyward_sys::kmsg(&format!("{} is exported to the computer as USB mass storage", disk.device))?;
    Ok(())
}

fn mass_storage_active(disk: &Disk) -> Result<bool> {
    let file = Path::new(&disk.gadget).join("functions/mass_storage.0/lun.0/file");
    match fs::read_to_string(&file) {
        Ok(value) => Ok(!value.trim().is_empty()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("read {}", file.display())),
    }
}
