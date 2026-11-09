//! The two disk tools in Advanced on the Xiaomi Pad 8 Pro: export the main
//! UFS LUN to a computer, and repair the GPT tables on its UFS LUNs.

use crate::{services, wipe};
use anyhow::{ensure, Context, Result};
use skyward_config::DeviceConfig;
use skyward_vab::Slot;
use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::Duration;

const DISK: &str = "/dev/block/sda";
const GADGET: &str = "/config/usb_gadget/g1";
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
    only_piano(cfg)?;
    ensure!(!mass_storage_active()?, "USB mass storage is already enabled");
    prepare_disk(cfg, slot)?;
    export()
}

/// Repair GPT on every UFS LUN the reference tree tested, from sda through
/// sdf. Both table copies are moved to the ends of each disk; partition
/// entries and data are not changed.
pub fn repair_gpt(cfg: &DeviceConfig, slot: Option<Slot>) -> Result<Vec<String>> {
    check_gpt(cfg)?;
    prepare_disk(cfg, slot)?;
    let prepared = Command::new("/system/bin/sh")
        .arg("/piano/bin/piano-fixgpt-unmount.sh")
        .status()
        .context("run the tested piano Fix GPT preparation script")?;
    ensure!(prepared.success(), "piano Fix GPT preparation script ended {prepared}");
    let mut repaired = Vec::new();
    let mut failed = Vec::new();
    for suffix in b'a'..=b'f' {
        let name = format!("sd{}", suffix as char);
        let device = PathBuf::from("/dev/block").join(&name);
        if !fs::metadata(&device).is_ok_and(|metadata| metadata.file_type().is_block_device()) {
            continue;
        }
        let _ = skyward_sys::kmsg(&format!("repairing GPT on {}", device.display()));
        match fix_gpt_with_gdisk(&device) {
            Ok(()) => repaired.push(name),
            Err(error) => failed.push(format!("{name}: {error:#}")),
        }
    }
    unsafe { libc::sync() };
    ensure!(failed.is_empty(), "GPT repair failed on {}. Completed: {:?}. Restart recovery before mounting partitions.", failed.join(", "), repaired);
    ensure!(!repaired.is_empty(), "no UFS GPT disks were found");
    Ok(repaired)
}

pub fn check_gpt(cfg: &DeviceConfig) -> Result<()> {
    only_piano(cfg)?;
    ensure!(!mass_storage_active()?, "USB mass storage must be disabled by restarting the recovery first");
    for tool in [c"/system/bin/gdisk", c"/system/bin/dmctl"] {
        ensure!(
            unsafe { libc::access(tool.as_ptr(), libc::X_OK) } == 0,
            "{} is unavailable or not executable",
            tool.to_string_lossy(),
        );
    }
    Ok(())
}

/// Run the same interactive sequence as the tested piano recovery tree. It
/// moves the primary table to the first usable sector and the backup table
/// to the last usable sectors without replacing any partition entry.
fn fix_gpt_with_gdisk(device: &Path) -> Result<()> {
    let output = Command::new("/system/bin/sh")
        .args([
            "-c",
            "printf 'x\\nj\\n\\nk\\n\\nw\\nY\\n' | /system/bin/gdisk \"$1\"",
            "gdisk",
        ])
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
        stdout.contains("The operation has completed successfully."),
        "gdisk {} did not report a completed operation",
        device.display()
    );
    Ok(())
}

fn only_piano(cfg: &DeviceConfig) -> Result<()> {
    ensure!(
        cfg.device.codename == "piano",
        "whole-disk tools are configured for piano, not {}",
        cfg.device.codename
    );
    Ok(())
}

/// Let go of the services and mappings that can keep a filesystem on sda
/// mounted, then unmount every remaining mount backed by that disk.
fn prepare_disk(cfg: &DeviceConfig, slot: Option<Slot>) -> Result<()> {
    // The tested piano recovery stops only the vendor services that can hold
    // files from the exported disk. Touch, the UI, BootControl and the other
    // ramdisk-only services keep running.
    const EXPORT_DISK_SERVICES: [&str; 10] = [
        "/piano/bin/piano-qseecomd",
        "/piano/bin/piano-minkdaemon",
        "/piano/bin/piano-ssgtzd",
        "/piano/bin/piano-qwesd",
        "/piano/bin/piano-keymint",
        "/piano/bin/piano-gatekeeper",
        "/piano/bin/piano-weaver-hos3",
        "/piano/bin/piano-weaver-hos4",
        "/vendor/bin/pd-mapper",
        "/vendor/bin/sscrpcd",
    ];
    let services_to_stop: Vec<_> = cfg
        .services
        .iter()
        .filter(|service| EXPORT_DISK_SERVICES.contains(&service.path.as_str()))
        .cloned()
        .collect();
    services::stop_all(&services_to_stop)?;
    for service in ["vendor.piano_boot_adsp", "vendor.piano_batterysecret"] {
        let property = format!("init.svc.{service}");
        if skyward_sys::prop(&property).is_some() {
            skyward_sys::set_prop("ctl.stop", service)?;
            skyward_sys::wait_prop(&property, "stopped", Duration::from_secs(5))?;
        }
    }
    wipe::release_data().context("close the data partition")?;
    if let Some(slot) = slot {
        skyward_vab::unmount_partitions(slot).context("unmap the installed system's logical partitions")?;
    }
    unsafe { libc::sync() };
    let unmounted = Command::new("/system/bin/sh")
        .arg("/piano/bin/piano-msc-unmount.sh")
        .status()
        .context("run the tested piano MSC unmount script")?;
    ensure!(unmounted.success(), "piano MSC unmount script ended {unmounted}");
    unsafe { libc::sync() };
    // Like the reference, try to invalidate the kernel's cache for the LUN.
    if let Ok(disk) = fs::File::open(DISK) {
        unsafe { libc::ioctl(disk.as_raw_fd(), BLKFLSBUF as _, 0) };
    }
    Ok(())
}

fn export() -> Result<()> {
    ensure!(gadget().exists(), "{} is not mounted", GADGET);
    let output = Command::new("/system/bin/sh")
        .arg("/piano/bin/piano-msc-start.sh")
        .output()
        .context("run the tested piano MSC USB setup script")?;
    ensure!(
        output.status.success(),
        "piano MSC USB setup ended {}: {}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    skyward_sys::kmsg(&format!("{DISK} is exported to the computer as USB mass storage"))?;
    Ok(())
}

fn mass_storage_active() -> Result<bool> {
    let file = gadget().join("functions/mass_storage.0/lun.0/file");
    match fs::read_to_string(&file) {
        Ok(value) => Ok(!value.trim().is_empty()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| format!("read {}", file.display())),
    }
}

fn gadget() -> PathBuf {
    PathBuf::from(GADGET)
}
