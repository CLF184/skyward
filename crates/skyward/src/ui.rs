use crate::backup;
use crate::decrypt::{self, Decryptor};
use crate::filework;
use crate::haptics::Haptics;
use crate::relabel;
use crate::input::{monotonic_ms, Event, Inputs};
use crate::merge;
use crate::partwork;
use crate::pty;
use crate::readings::{Reading, Readings};
use crate::secure;
use crate::services;
use crate::updates;
use crate::wipe::{self, Eraser, Step};
use crate::wlan;
use anyhow::{anyhow, ensure, Context, Result};
use skyward_block::fsck::Fixed;
use skyward_block::{fstab, mount, probe};
use skyward_config::{DeviceConfig, Service};
use skyward_crypto::data::{Closed, Opened};
use skyward_install::asked::{self, Needs};
use skyward_install::{Detach, Installer, Options};
use skyward_sys::bcb::{self, Work, REFORMATS};
use skyward_sys::control::{Control, Request};
use skyward_sys::Power;
use skyward_files::Ownership;
use skyward_block::usb::{self, Drive};
use skyward_net::download::{self, Failure};
use skyward_net::http::Servers;
use skyward_net::release::{Installed, Release, Unanswered};
use skyward_net::wlan::{Power as WlanPower, Wlan};
use skyward_ui::app::{
    App, Asked, BackupAt, BackupEnd, BackupEntry, BackupRefusal, ClashChoice, Data, DownloadEnd, Effect, EraseStop, FileKind, FileOp,
    FileRow, FileTask, FsAction, FsRow, Have, Held, ItemEnd, ItemState, JobEnd, Keys, MountFailure, Part, PartKind, PartStop, Place,
    RenameRefused, Screen, Shot, Target, UsbDrive, UsbLink, Wipe, DATA_OPENING, DATA_PARTITION, DOWNLOADS, INSTALL_UNFINISHED, USB_DRIVE,
};
use skyward_ui::surface::kms::KmsSurface;
use skyward_ui::{Canvas, Fonts, Lang, Surface};
use skyward_vab::{Merge, Standing};
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
使用 crate：：backup;使用 crate：:d ecrypt：：{self， Decryptor};使用 crate：：filework;使用 crate：：haptics：：触觉;使用 crate：：relabel;使用 crate：：input：：{monotonic_ms， Event， Inputs};使用 crate：：merge;使用 crate：:p artwork;使用 crate：:p ty;使用 crate：：readings：：{阅读，阅读};使用 crate：：secure;使用 crate：：services;使用 crate：：updates;使用 crate：：wipe：：{self， eraser， step};使用 crate：：wlan;使用 anyhow：：{anyhow， ensure， Context， Result};使用 skyward_block：：fsck：：Fixed;使用 skyward_block：：{fstab， mount， probe};use skyward_config：：{DeviceConfig， Service};使用 skyward_crypto：:d ata：：{关闭，打开};使用 skyward_install：：asked：：{self， Needs};use skyward_install：：{Detach， Installer， Options};使用 skyward_sys：：bcb：：{self， Work， REFORMATS};使用 skyward_sys：：control：：{Control， Request};使用 skyward_sys：:P ower;使用 skyward_files：：所有权;使用 skyward_block：：USB：：{self， Drive};使用 skyward_net：:d ownload：：{self， Failure};使用 skyward_net：：http：：服务器;请使用 skyward_net：：release：{已安装、发布、未回复};使用 使用 skyward_net：：wlan：：{Power as WlanPower， Wlan};使用 skyward_ui：：app：：{ App、Asked、BackupAt、BackupEnd、BackupEntry、BackupRefusal、ClashChoice、Data、DownloadEnd、Effect、EraseStop、FileKind、FileOp、FileRow、FileTask、FsAction、FsRow、Have、Held、ItemEnd、ItemState、JobEnd、Keys、MountFailure、Part、PartKind、PartStop、Place、RenameRefused、Screen、Shot、Target、U.B 驱动器、UsbLink、清除、DATA_OPENING、DATA_PARTITION、DOWNLOADS、INSTALL_UNFINISHED、USB_DRIVE、};使用 skyward_ui：：surface：：kms：：KmsSurface;使用 skyward_ui：：{Canvas， Fonts， Lang， Surface};使用 skyward_vab：：{合并，站立};use std：：collections：：{BTreeSet， VecDeque};使用标准版：：FS;使用 std：：io：：write;使用 std：：os：：unix：：fs：：MetadataExt;使用 std：:p ath：：{Path， PathBuf};使用 std：：sync：：{Arc， Mutex};使用 std：：time：：{Duration， 即时、SystemTime、UNIX_EPOCH};

/// How long init may take to hand the USB port to another function: the
/// program behind the new one has to start and get ready first.
const USB_TIMEOUT: Duration = Duration::from_secs(5);
init 需要多长时间才能把 USB 端给另一个功能：新接口背后的 /// 程序必须先启动并准备好。cont USB_TIMEOUT：持续时间 = 持续时间：：from_secs（5）;

/// How long the page shows that the work the system asked for is done
/// before the phone starts the system or powers off.
const LEAVE_AFTER_MS: u64 = 1500;
页面显示系统要求的工作完成的时间长 /// 在手机启动系统或关机前。cont LEAVE_AFTER_MS： u64 = 1500;

/// How long after the screen starts a touch panel may still come up; with
/// none by then, the pages are worked with the keys, their light shown.
const TOUCH_WAIT: Duration = Duration::from_secs(5);
屏幕启动后多久还会出现触摸面板;到那时没有///，页面就用键被敲动，光芒照在眼前。cont TOUCH_WAIT：时长 = 时长：：from_secs（5）;

/// Where logs saved to the temporary folder go, and where in internal
/// storage the others do.
const TMP_LOGS: &str = "/tmp/skyward-logs";
const STORAGE_LOGS: &str = "Skyward/logs";
日志保存到临时文件夹的位置，以及其他日志存放在内部 /// 存储。const TMP_LOGS： &str = “/tmp/skyward-logs”;cont STORAGE_LOGS： &str = “向天空/原木”;

/// The folder of internal storage the system saves its own screenshots in,
/// which the gallery shows as its screenshots, and where a screenshot waits
/// for internal storage to open.
const SCREENSHOTS: &str = "DCIM/Screenshots";
const HELD_SCREENSHOTS: &str = "/tmp/screenshots";
系统保存截图的内部存储文件夹，/// 画廊显示为截图，截图则等待 /// 内部存储打开。const 截图：&str = “DCIM/截图”;const HELD_SCREENSHOTS： &str = “/tmp/screenshots”;

/// The panel backlight under /sys/class/backlight.
struct Backlight {
    path: PathBuf,
    /// The raw level at the top of the slider: the panel's normal maximum.
    top: u32,
    /// The raw level last written; at first the one the bootloader left.
    level: u32,
}
面板背光灯在 /sys/class/backlight。struct Backlight { 路径：PathBuf， /// 滑块顶部的原始等级：面板的正常最大值。顶部：u32， /// 最后写入的原始等级;最初是引导加载程序留下的。等级：u32， }

impl Backlight {
    fn open() -> Result<Self> {
        let mut dirs: Vec<PathBuf> = fs::read_dir("/sys/class/backlight")
            .context("read /sys/class/backlight")?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::io::Result<_>>()?;
        dirs.sort();
        // Qualcomm panels expose panel0-backlight; take it when present.
        let dir = dirs
            .iter()
            .find(|d| d.file_name().is_some_and(|n| n == "panel0-backlight"))
            .or(dirs.first())
            .ok_or_else(|| anyhow!("no backlight device"))?
            .clone();
        let read = |name: &str| -> Result<u32> {
            fs::read_to_string(dir.join(name))
                .with_context(|| format!("read {}/{name}", dir.display()))?
                .trim()
                .parse()
                .with_context(|| format!("parse {name}"))
        };
        let max = read("max_brightness")?;
        // The panel's normal maximum is read from the node the display
        // driver booted with, which not every panel gives the same way. A
        // panel whose node says it as none of them still shows the pages,
        // at the whole of the backlight's range, and the reason goes to the
        // kernel log rather than ending the recovery.
        let top = match normal_max(max) {
            Ok(top) => top,
            Err(e) => {
                let _ = skyward_sys::kmsg(&format!(
                    "the panel's normal maximum brightness is not known, so the backlight's own maximum is used: {e:#}"
                ));
                max
            }
        };
        let level = read("brightness")?;
        Ok(Backlight { path: dir.join("brightness"), top, level })
    }
impl 背光 { fn open（） -> 结果 <Self> { let mut dirs： Vec<PathBuf> = fs：：read_dir（“/sys/class/backlight”） .context（“read /sys/class/backlight”）？ .map（|e| e.map（|e| e.path（））） .collect：：<std：：io：：Result<_>>（）？; dirs.sort（）; // 高通面板曝光 panel0 背光;出现时取用。let dir = dirs .iter（） .find（|d| d.file_name（）.is_some_and（|n| n == “panel0-backlight”）） .or（dirs.first（）） .ok_or_else（|| anyway！（”无背光装置“））？.clone（）;设 = |name： &str|-> 结果 <u32> { fs：：read_to_string（dir.join（name）） .with_context（|| format！（”read {}/{name}“， dir.display（））？.trim（） .parse（） .with_context（|| format！（”解析 {name}“）） };设 max = read（“max_brightness”）？;面板的正常最大值是从显示//驱动启动的节点读取的，但并非每个面板的输出方式相同。一个//面板，节点显示没有任何一个，但仍然显示页面，//在背光的整个范围，原因会进入//内核日志，而不是终止恢复。let top = 匹配 normal_max（max） { Ok（top） => top， Err（e） => { let _ = skyward_sys：：kmsg（&format！（“面板的正常最大亮度未知，因此使用背光自身的最大亮度：{e：#}” ））;最大 } };设电平 = read（“亮度”）？;Ok（Backlight { path： dir.join（“brightness”）， 顶部，水平 }） }

    /// Light the panel at `level` of its normal maximum, or turn it off
    /// with `None`. The file is written only when the raw level changes.
    fn show(&mut self, level: Option<f32>) -> Result<()> {
        let raw = level.map_or(0, |v| ((v.clamp(0.0, 1.0) * self.top as f32).round() as u32).max(1));
        if raw != self.level {
            fs::write(&self.path, raw.to_string()).with_context(|| format!("write {}", self.path.display()))?;
            eprintln!("backlight {raw}");
            self.level = raw;
        }
        Ok(())
    }
}
将面板点亮到正常最大亮度的“水平”，或者用“无”关闭 ///。只有当原始电平发生变化时，文件才会被写入。fn show（&mut self， level： Option<f32>） -> Result<（）> { let raw = level.map_or（0， |v|（（v.clamp（0.0， 1.0） * 以 self.top 为 f32）.round（） 为 u32）.max（1））;如果 raw ！= self.level { fs：：write（&self.path， raw.to_string（））.with_context（|| format！（”write {}“， self.path.display（））？;EprintLN！（“背光 {raw}”）;自水平 = 原始值;} Ok（（）） } }

/// How a panel's node in the device tree may give its normal maximum: the
/// property holding it, and the one holding the whole range on the same
/// scale. The 13 Ultra's panel gives both on a finer scale of its own that
/// spans the same light as the backlight's range; the 17 Ultra's gives them
/// on the backlight's own scale. The panels of the Xiaomi 13 and 15, of the
/// 14 and the K80 series and one of the K90's give it only as the level
/// their factory tests drive the panel to, on the backlight's own scale:
/// the 13 Ultra's panels give that level as well, and it comes to the same
/// half of their range as the normal maximum they give.
const NORMAL_MAX: [(&str, &str); 3] = [
    ("mi,normal-max-brightness-clone", "mi,max-brightness-clone"),
    ("mi,max-brightness-normal", "qcom,mdss-dsi-bl-max-level"),
    ("mi,mdss-dsi-fac-bl-max-level", "qcom,mdss-dsi-bl-max-level"),
];
设备树中面板节点可能给出其正常最大值：保持该节点的///属性，以及在同一/尺度上保持整个范围的节点。13 Ultra 的面板在更细微的尺度上，能实现////光线覆盖与背光范围相同的光线;17 Ultra 在背光刻度上给它们 /// 音效。小米 13 和 15、/// 14、K80 系列以及其中一款 K90 的面板，都只给出厂测试面板的////背光等级：/// 13 Ultra 的面板也给出了这个水平，结果达到了它们正常最大值的一半///。const NORMAL_MAX： [（&str， &str）; 3] = [（“MI，正常-最大亮度-克隆”，“MI，最大亮度-克隆”），（“MI，最大亮度-正常”，“Qcom，MDSS-DSI-BL-最大水平”），（“Mi，MDSS-DSI-FAC-BL-最大水平”，“qcom，MDSS-dsi-BL-最大水平”），];

/// The raw backlight level, out of `max`, at the panel's normal maximum.
/// Above it the panel runs in high brightness mode, which the stock system
/// keeps for strong sunlight and its slider does not reach. The panel the
/// display driver booted with gives that maximum in the device tree.
fn normal_max(max: u32) -> Result<u32> {
    let param = "/sys/module/msm_drm/parameters/dsi_display0";
    let boot = fs::read_to_string(param).with_context(|| format!("read {param}"))?;
    // The panel's node, then its configuration after a colon, as in
    // "qcom,mdss_dsi_m1_42_02_0a_dsc_cmd:"; the driver takes no name
    // without the colon either.
    let (name, _) = boot.trim().split_once(':').with_context(|| format!("{param} names no panel: {boot:?}"))?;
    let soc = Path::new("/proc/device-tree/soc");
    let node = fs::read_dir(soc)
        .with_context(|| format!("read {}", soc.display()))?
        .map(|e| e.map(|e| e.path().join(name)))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .find(|p| p.is_dir())
        .ok_or_else(|| anyhow!("panel {name} is not in the device tree"))?;
    let cell = |prop: &str| -> Result<u32> {
        let path = node.join(prop);
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let cell: [u8; 4] = bytes.as_slice().try_into().map_err(|_| anyhow!("{} is not one cell", path.display()))?;
        Ok(u32::from_be_bytes(cell))
    };
    let (normal, full) = NORMAL_MAX
        .iter()
        .find(|(normal, _)| node.join(normal).exists())
        .with_context(|| format!("panel {name} gives its normal maximum as none of {NORMAL_MAX:?}"))?;
    let (normal, full) = (cell(normal)?, cell(full)?);
    ensure!(normal > 0 && normal <= full, "panel {name}: normal maximum {normal} out of {full}");
    Ok((f64::from(max) * f64::from(normal) / f64::from(full)).round() as u32)
}
原始背光水平，超出“最大”值，处于面板正常的最大值。面板上方是高亮度模式，原厂系统 /// 保留了强光，滑块也够不到。/// 显示驱动启动时的面板在设备树中给出了这个最大值。fn normal_max（最大值：u32） -> 结果 <u32> { let param = “/sys/module/msm_drm/parameters/dsi_display0”; let boot = fs：：read_to_string（param）.with_context（|| format！（”读作 {param}“）？;面板的节点，然后是冒号后的配置，如 // “qcom，mdss_dsi_m1_42_02_0a_dsc_cmd：”;没有冒号的驱动程序也不会取名字 // 。设 （name， _） = boot.trim（）.split_once（'：'）.with_context（|| 格式！（”{param} 名称无面板： {boot：？}“）？;令 soc = Path：：new（“/proc/device-tree/soc”）;设 node = FS：：read_dir（SoC） .with_context（|| 格式！（”read {}“， soc.display（））？.map（|e| e.map（|e| e.path（）.join（name）））） .collect：：<std：：io：：结果<Vec<_>>>（）？ .into_iter（） .find（|p| p.is_dir（）） .ok_or_else（|| anyhow！（”面板 {name} 不在设备树中“））？;令 cell = |prop： &str|-> 结果 <u32> { let path = node.join（prop）; let bytes = fs：：read（&path）.with_context（|| format！（”read {}“， path.display（））？;设单元：[U8; 4] = bytes.as_slice（）.try_into（）.map_err（|_| 无论如何！（”{} 不是单一单元格“，path.display（））？;Ok（u32：：from_be_bytes（cell）） };设（正常，满）= NORMAL_MAX .iter（） .find（|（normal，_）|node.join（normal）.exists（）） .with_context（|| format！（”面板 {name} 给出其正常最大值为 {NORMAL_MAX：？}“））？;令（正常，满）= （单元格（正常）？， 单元（满）？）;确保！（法线>0 & 法线 <=满格，“面板 {name}：{满}中法线最大值 {normal}”）;Ok（（f64：：from（max） * f64：：from（normal） / f64：：from（full））.round（） as u32） }

/// Leave recovery for the chosen target.
fn power(target: Target) -> Result<()> {
    skyward_sys::power(match target {
        Target::System => Power::System,
        Target::Recovery => Power::Recovery,
        Target::Fastboot => Power::Fastboot,
        Target::Bootloader => Power::Bootloader,
        Target::Edl => Power::Edl,
        Target::PowerOff => Power::Off,
    })
}
把恢复留给指定目标。fn power（target： Target） -> Result<（）> { skyward_sys：:p ower（match target { Target：：System => Power：：System， Target：：Recovery => Power：：Recovery， Target：：Fastboot => Power：：Fastboot， Target：：Bootloader => Power：：Bootloader， Target：：Edl => Power：：Edl， Target：:P owerOff => Power：：Off， }） }

/// File transfer: the service offering internal storage to a computer over
/// USB, whether it offers the storage now, and the zone, by its place in
/// the settings, it tells the times of files in.
struct Transfer {
    service: skyward_mtp::Service,
    offered: bool,
    zone: usize,
}
文件传输：通过 /// USB 向电脑提供内部存储的服务，无论现在是否提供存储和区域，通过设置中的位置，它会告诉文件进入的时间。struct Transfer { service： skyward_mtp：：Service， offered： bool， zone： usize， }

impl Transfer {
    /// Serve file transfer; the port offers it from when it next rests.
    fn start(cfg: &DeviceConfig, info: &skyward_info::DeviceInfo, zone: usize) -> Result<Transfer> {
        let device = skyward_mtp::Device {
            manufacturer: cfg.device.brand.clone(),
            model: info.model.clone(),
            version: format!("Skyward {}", skyward_info::VERSION),
            serial: skyward_sys::prop("ro.serialno").unwrap_or_default(),
            name: info.model.clone(),
            battery: Box::new(|| skyward_info::battery_now().ok().flatten().map(|b| b.level)),
        };
        let service = skyward_mtp::Service::start(device, skyward_ui::fmt::rules(zone).clone())?;
        skyward_sys::usb::set_mtp(true);
        Ok(Transfer { service, offered: false, zone })
    }
impl Transfer { /// Serve file transfer;端口从下次休息时开始提供。fn start（cfg： &DeviceConfig， info： &skyward_info：:D eviceInfo， zone： usize） -> Result<Transfer> { let device = skyward_mtp：:D evice { 制造商： cfg.device.brand.clone（）， model： info.model.clone（）， version： format！（”Skyward {}“， skyward_info：：VERSION）， serial： skyward_sys：:p rop（”ro.serialno“）.unwrap_or_default（）， name： info.model.clone（）， battery： Box：：new（|| skyward_info：：battery_now（）.ok（）.flatten（）.map（|b| b.level））， };let service = skyward_mtp：：Service：：start（device， skyward_ui：：fmt：：rules（zone）.clone（）？;skyward_sys：：usb：：set_mtp（真）;Ok（Transfer { service， offered： false， zone }） }

    /// Offer internal storage, named for the computer in `lang`, or take it
    /// back. Once this returns, nothing the computer had under way in it
    /// goes on.
    fn offer(&mut self, offer: bool, lang: Lang) -> Result<()> {
        if offer != self.offered {
            let description = skyward_ui::fmt::storage_name(lang).into();
            let storage = offer.then(|| skyward_mtp::Storage { top: PathBuf::from(skyward_crypto::data::SDCARD), description });
            self.service.offer(storage)?;
            self.offered = offer;
        }
        Ok(())
    }
提供内部存储，以计算机命名，用“lang”，或者收回///。一旦恢复，电脑里原本的///都不会发生。fn offer（&mut self， offer： bool， lang： lang） -> Result<（）> { if offer ！= self.offered { let description = skyward_ui：：fmt：：storage_name（lang）.into（）; let storage = offer.then（|| skyward_mtp：：Storage { 顶部： PathBuf：：from（skyward_crypto：:d ata：：SDCARD）， description }）; self.service.offer（storage）？; self.offer = offer; }Ok（（）） }

    /// Stop serving: a port at rest goes back to adb alone first.
    fn stop(self, rests: bool) -> Result<()> {
        skyward_sys::usb::set_mtp(false);
        let switched = if rests { skyward_sys::usb::switch("adb", USB_TIMEOUT) } else { Ok(()) };
        let stopped = self.service.stop();
        switched.and(stopped)
    }
}
停止服务：一个处于静止状态的港口首先单独回到 ADB。fn stop（self，休止：bool） -> Result<（）> { skyward_sys：：usb：：set_mtp（false）; 设 switched = 如果休止符 { skyward_sys：：usb：：switch（“adb”， USB_TIMEOUT） } 否则 { Ok（（）） }; let stopped = self.service.stop（）; switched.and（stopped） } }

/// Take internal storage back from file transfer, if it is on, before work
/// that erases or rewrites the data partition, or installs; whether that
/// work can start, which the page is told otherwise.
fn withhold(transfer: &mut Option<Transfer>, app: &mut App, now: u64) -> Result<bool> {
    let Some(t) = transfer else { return Ok(true) };
    match t.offer(false, app.state.lang) {
        Ok(()) => Ok(true),
        Err(e) => {
            skyward_sys::kmsg(&format!("file transfer kept internal storage: {e:#}"))?;
            app.report(&skyward_ui::fmt::transfer_kept(app.state.lang, &format!("{e:#}")), now);
            Ok(false)
        }
    }
}
在文件传输时，如果内部存储已开启，在///处理清除或重写数据分区或安装前返回;是否能开始///工作，但页面被告知不行。fn withhold（transfer： &mut Option<Transfer>， app： &mut App， now： u64） -> Result<bool> { let Some（t） = transfer else { return Ok（true） }; match t.offer（false， app.state.lang） { Ok（（）） => Ok（true）， Err（e） => { skyward_sys：：kmsg（&format！（”文件传输保留内部存储：{e：#}“）？;app.report（&skyward_ui：：fmt：：transfer_kept（app.state.lang， &format！（”{e：#}“）），现在）;Ok（false） } } }

/// Whether the phone's fastbootd runs, as init tells it, and how far a
/// computer has the port, as the USB controller tells it.
fn fastboot_status() -> Result<(bool, UsbLink)> {
    let running = skyward_sys::prop("init.svc.fastbootd").as_deref() == Some("running");
    let controller = skyward_sys::prop("sys.usb.controller").context("init names no USB controller")?;
    let path = Path::new("/sys/class/udc").join(controller).join("state");
    let state = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let link = match state.trim() {
        "configured" => UsbLink::Connected,
        "not attached" => UsbLink::Unplugged,
        _ => UsbLink::Connecting,
    };
    Ok((running, link))
}
手机的 fastbootd 是否能像 init 说的那样运行，////电脑的端口距离（USB 控制器告诉它）是多少。fn fastboot_status（） -> Result<（bool， UsbLink）> { let running = skyward_sys：:p rop（“init.svc.fastbootd”）.as_deref（） == Some（“running”）; let controller = skyward_sys：:p rop（“sys.usb.controller”）.context（“init 名称 no USB controller”）？; let path = Path：：new（“/sys/class/udc”）.join（controller）.join（“state”）; let state = fs：：read_to_string（&path）.with_context（|| format！（”read {}“， path.display（））？;let link = match state.trim（） { “configured” => UsbLink：：Connected， “not attached” => UsbLink：：Unplugged， _ => UsbLink：：Connecting， };好（（running，链接）） }

/// What a request from misc shows on the pages that answer it.
fn asked(request: &bcb::Request) -> Asked {
    Asked {
        reason: request.reason.clone(),
        shutdown_after: request.shutdown_after,
        unhandled: match &request.work {
            Work::Unsupported(what) => Some(what.clone()),
            _ => None,
        },
    }
}
在回答它的页面上，misc 的请求会显示出多么精彩的体验。fn asked（request： &bcb：：Request） -> Asked { Asked { reason： request.reason.clone（）， shutdown_after： request.shutdown_after， unhandled： match &request.work { Work：：Unsupported（what） => Some（what.clone（））， _ => 无， }， } }

/// Read a folder for the file manager, as it is now. A folder of a user
/// whose own files are still closed says so in place of its entries: the
/// names in it cannot be read until that user's credential is given.
fn list(app: &mut App, folder: String, closed: &[Closed]) -> Result<()> {
    let files = match closed_to_user(closed, Path::new(&folder)) {
        Some((_, why)) => Err(why),
        None => read_folder(Path::new(&folder)).map_err(|e| format!("{e:#}")),
    };
    app.set_files(folder, files, unix_now()?);
    Ok(())
}
读取文件管理器的文件夹，就像现在这样。某个用户///的文件夹，其自己的文件仍然关闭，但其条目显示了这个词：里面的///名字在该用户凭证输入之前无法读取。fn list（app： &mut 应用，文件夹：String，关闭： &[Closed]） -> Result<（）> {let files = match closed_to_user（关闭，路径：：new（&folder）） { Some（（_， why）） => Err（why）， None => read_folder（Path：：new（&folder））.map_err（|e| format！（”{e：#}“）），};app.set_files（文件夹、文件、unix_now（）？）;Ok（（）） }

/// The number of the user a folder of the data partition belongs to, where
/// it is one of the phone's own user areas: `/data/user/<n>`, where each
/// user's own files live, and `/data/media/<n>`, where a user's shared
/// storage does. `None` for every other folder.
fn user_of(path: &Path) -> Option<u32> {
    skyward_crypto::users::user_of(path)
}
数据分区文件夹所属用户编号，其中///是手机自身的用户区域之一：'/data/user/<n>'，每个///用户的文件所在，'/data/media/<n>'，用户共享的///存储。每隔一个文件夹为'无'。fn user_of（路径：&Path） -> 选项 <u32> { skyward_crypto：：users：：：user_of（path） }

/// What to say of a folder that cannot be listed because the own files of
/// `user` are still closed, or `None` when that is not why.
///
/// A phone that keeps a second space or a cloned app open has users whose
/// own files stay closed until their credential is given: the recovery
/// opens the owner's and leaves the rest for the file manager to offer. A
/// folder of theirs, opened before that, reads as names it cannot show, so
/// the page says which user's files are still closed rather than that the
/// folder is empty or broken.
fn closed_to_user(closed: &[Closed], path: &Path) -> Option<(u32, String)> {
    let user = user_of(path)?;
    let entry = closed.iter().find(|c| c.user == user)?;
    let why = match &entry.why {
        Some(why) => format!("user {user}'s own files did not open: {why}"),
        None => format!(
            "user {user}'s own files are still closed. The recovery opens the owner's first; \
             return to the decrypt page and enter this user's screen lock to open them."
        ),
    };
    Some((user, why))
}
对于一个文件夹，因为///“user”的自己文件仍然关闭，或者“无”，而这并不是原因，该怎么说呢？/// 如果手机开着第二个空间或克隆应用，用户的////文件会一直关闭，直到获得凭证：恢复///打开了拥有者的，剩下的留给文件管理器。他们之前打开的一个///文件夹会显示为无法显示的名称，所以///页面显示的是哪个用户的文件仍然关闭，而不是///文件夹是空的或坏了。fn closed_to_user（closed： &[Closed]， path： &Path） -> Option<（u32， String）> {let user = user_of（path）？; let entry = closed.iter（）.find（|c| c.user == user）？; let why = match &entry.why { Some（why） => format！（”用户 {user} 自己的文件未打开：{why}“），None =>格式！（“用户 {user} 自己的文件仍然关闭。回收先打开主人的门;\ 返回解密页面并输入该用户的屏幕锁定以打开它们。“）， };Some（（用户，为什么）） }

/// Read a folder of `place` to choose a file to install in, as it is now;
/// `folder` is its path from the place's top.
fn list_place(app: &mut App, place: Place, folder: String) -> Result<()> {
    let files = read_folder(&in_place(place, &folder)).map_err(|e| format!("{e:#}"));
    app.set_place(place, folder, files, unix_now()?);
    Ok(())
}
读取一个“place”文件夹来选择安装文件，就像现在这样;“文件夹”是它从顶部开始的路径。fn list_place（app： &mut 应用， place： Place， folder： String） -> Result<（）> { let files = read_folder（&in_place（place， &folder））.map_err（|e| format！（”{e：#}“））;app.set_place（place， folder， files， unix_now（）？）;Ok（（）） }

/// Where `path`, from the top of `place`, is: internal storage is the
/// owner's shared storage, where the data partition shows it.
fn in_place(place: Place, path: &str) -> PathBuf {
    let top = match place {
        Place::Storage => Path::new(skyward_crypto::data::SDCARD),
        Place::UsbDrive => Path::new(USB_DRIVE),
    };
    if path.is_empty() {
        top.to_path_buf()
    } else {
        top.join(path)
    }
}
其中“路径”，从“位置”顶部开始是：内部存储是///所有者的共享存储，数据分区显示了它。fn in_place（place： Place， path： &str） -> PathBuf { let top = 匹配 place， { Place：：Storage => Path：：new（skyward_crypto：:d ata：：SDCARD）， Place：：UsbDrive => Path：：new（USB_DRIVE）， }; if path.is_empty（） { top.to_path_buf（） } 否则 { top.join（path） } }

/// Save the logs chosen into a folder of their own in `into`, named by when
/// they were saved in the zone the settings chose: the recovery's own, with
/// what update_engine wrote while it last installed an update and what the
/// last install script wrote, and the kernel's. Each file is owned as a
/// copy into the folder would be. Gives the folder.
fn export_logs(recovery: bool, kernel: bool, into: &Path, zone: usize) -> Result<PathBuf> {
    let there = into.ancestors().find(|p| p.exists()).context("the root is always there")?;
    let ownership = ownership(there)?;
    let folder = into.join(skyward_ui::fmt::stamp(unix_now()?, zone));
    skyward_files::make_folders(&folder, ownership)?;
    let save = |name: &str, contents: &[u8]| -> Result<()> {
        let path = folder.join(name);
        let mut file = skyward_files::make_file(&path, ownership)?;
        file.write_all(contents).with_context(|| format!("write {}", path.display()))?;
        file.sync_all().with_context(|| format!("write {}", path.display()))
    };
    if recovery {
        save("recovery.log", &fs::read(skyward_sys::LOG).with_context(|| format!("read {}", skyward_sys::LOG))?)?;
        for (name, path) in [("update_engine.log", skyward_install::ENGINE_LOG), ("install-script.log", skyward_install::SCRIPT_LOG)] {
            match fs::read(path) {
                Ok(log) => save(name, &log)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).with_context(|| format!("read {path}")),
            }
        }
    }
    if kernel {
        save("kernel.log", skyward_sys::kernel_log()?.as_bytes())?;
    }
    Ok(folder)
}
把选定的日志保存到“into”里的独立文件夹里，按保存时间命名：恢复区、/// update_engine 上次安装更新时写的内容、////最后安装脚本写的，以及内核的。每个文件都是像 /// 一样被归入文件夹的。给了文件夹。fn export_logs（recovery： bool， kernel： bool， into： &Path， zone： usize） -> Result<PathBuf> { let there = into.ancestors（）.find（|p| p.exists（））.context（“根总是在那里”）？; let ownership = ownership（there）？; let folder = into.join（skyward_ui：：fmt：：stamp（unix_now（）？， zone））; skyward_files：：make_folders（&folder， ownership）？; let save = |name： &str， contents： &[u8]| -> Result<（）> {let path = folder.join（name）; let mut file = skyward_files：：make_file（及 path，所有权）？;file.write_all（目录）.with_context（|| 格式！（”write {}“， path.display（））？;file.sync_all（）.with_context（|| 格式！（”写作{}“，path.display（））） };如果恢复 { save（“recovery.log”， &fs：：read（skyward_sys：：LOG）.with_context（|| 格式！（”read {}“， skyward_sys：：LOG））？）？;对于（name， path）在[（“update_engine.log”， skyward_install：：ENGINE_LOG）， （“install-script.log”， skyward_install：：SCRIPT_LOG）] { match fs：：read（path） { Ok（log） => save（name， &log）？， Err（e） if e.kind（） == std：：io：：ErrorKind：：NotFound => {} Err（e） => 返回 Err（e）.with_context（|| format！（”read {path}“））， } } } 如果内核 { save（”kernel.log“， skyward_sys：：kernel_log（）？.as_bytes（））？;} Ok（folder） }

/// Save `frame`, what the panel shows, as a picture named for when it was
/// taken: into the gallery's screenshots where internal storage is open, as
/// `storage` says, and otherwise into the recovery's own memory until it
/// opens. Each is made as the system makes what it saves there.
fn save_screenshot(frame: &Canvas, storage: bool, zone: usize) -> Result<Shot> {
    let (folder, shot) = match storage {
        true => (Path::new(skyward_crypto::data::SDCARD).join(SCREENSHOTS), Shot::Album),
        false => (PathBuf::from(HELD_SCREENSHOTS), Shot::Held),
    };
    let there = folder.ancestors().find(|p| p.exists()).context("the root is always there")?;
    let ownership = ownership(there)?;
    skyward_files::make_folders(&folder, ownership)?;
    let path = folder.join(skyward_ui::fmt::screenshot_name(unix_ms_now()?, zone));
    let file = skyward_files::make_file(&path, ownership)?;
    let mut out = std::io::BufWriter::new(&file);
    frame.write_png(&mut out).with_context(|| format!("write {}", path.display()))?;
    out.flush().with_context(|| format!("write {}", path.display()))?;
    drop(out);
    file.sync_all().with_context(|| format!("write {}", path.display()))?;
    skyward_sys::kmsg(&format!("screenshot saved as {}", path.display()))?;
    Ok(shot)
}
把面板显示的“frame”保存为/拍摄时的图片，保存到图库的截图中，内部存储是开放的，正如///“存储”所说，否则保存到恢复的内存中，直到///打开。每个都是系统根据保存的资源制作的。fn save_screenshot（frame： &Canvas， storage： bool， zone： usize） -> 结果 <Shot> { let （folder， shot） = match storage { true => （Path：：new（skyward_crypto：:d ata：：SDCARD）.join（SCREENSHOTS）， Shot：：Album）， false => （PathBuf：：from（HELD_SCREENSHOTS）， Shot：：Held）， }; let there = folder.ancestors（）.find（|p| p.exists（））.context（“根总是在那里”）？; let ownership = ownership（there）？; skyward_files：：make_folders（&folder， ownership）？; let path = folder.join（skyward_ui：：fmt：：screenshot_name（unix_ms_now（）？，区域））;let file = skyward_files：：make_file（&path， ownership）？;let mut out = std：：io：：BufWriter：：new（&file）;frame.write_png（&mut out）.with_context（|| 格式！（”write {}“， path.display（））？;out.flush（）.with_context（|| format！（”write {}“， path.display（））？;退出（退出）;file.sync_all（）.with_context（|| 格式！（”write {}“， path.display（））？;skyward_sys：：kmsg（&format！（”截图保存为 {}“， path.display（））？;好（镜头） }

/// Move the screenshots held while internal storage was closed into the
/// gallery's, now that it is open: each keeps its name, and is made there
/// as the system makes what it saves.
fn keep_held_screenshots() -> Result<()> {
    let held = Path::new(HELD_SCREENSHOTS);
    let shots = match fs::read_dir(held) {
        Ok(entries) => entries.map(|e| e.map(|e| e.path())).collect::<std::io::Result<Vec<_>>>().with_context(|| format!("read {}", held.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("read {}", held.display())),
    };
    if shots.is_empty() {
        return Ok(());
    }
    let album = Path::new(skyward_crypto::data::SDCARD).join(SCREENSHOTS);
    let there = album.ancestors().find(|p| p.exists()).context("the root is always there")?;
    let ownership = ownership(there)?;
    skyward_files::make_folders(&album, ownership)?;
    let stop = std::sync::atomic::AtomicBool::new(false);
    skyward_files::move_into(&shots, &album, skyward_files::Clash::KeepBoth, ownership, &stop, &mut |_| {})?;
    skyward_sys::kmsg(&format!("{} screenshots held in {} moved into {}", shots.len(), held.display(), album.display()))
}
把内部存储关闭期间保存的截图移到///画廊中，现在画廊开放了：每个截图都保留了自己的名字，并且随着系统保存内容的制作而被制作出来。fn keep_held_screenshots（） -> Result<（）> { let held = Path：：new（HELD_SCREENSHOTS）; let shots = match fs：：read_dir（holdd） { Ok（entryries） => entries.map（|e| e.map（|e| e.path（）））.collect：：<std：：io：：结果<Vec<_>>>（）.with_context（|| format！（”read {}“， held d.display（））？， err（e） if e.kind（） == std：：io：：ErrorKind：：NotFound => 返回 Ok（（））， Err（e） =>返回 Err（e）.with_context（|| format！（”read {}“， holdd.display（）））， };如果 shots.is_empty（） { return Ok（（））; } let album = Path：：new（skyward_crypto：:d ata：：SDCARD）.join（SCREENSHOTS）;让 There=album.ancestors（）.find（|p| p.exists（））.context（“根总是在那里”）？;令所有权 = 所有权（某处）？;skyward_files：：make_folders（及专辑、所有权）？;让 stop = std：：sync：：atomic：：AtomicBool：：new（false）;skyward_files：：move_into（&shots， and album， skyward_files：：Clash：：KeepBoth， ownership， &stop， &mut |_|{})?;skyward_sys：：kmsg（&format！（”{} 存放在 {} 中的截图移至 {}“， shots.len（）， held .display（）， album.display（）） }）

/// The folder backups go in and are found in, in `place`.
fn backups_in(place: Place) -> PathBuf {
    in_place(place, skyward_backup::FOLDER)
}
文件夹备份会被“定位”。fn backups_in（地点：地点） -> PathBuf { in_place（place， skyward_backup：：FOLDER） }

/// The backups in `place`: each folder in its backups folder, with what its
/// manifest records or why that cannot be read. A place with no backups
/// folder has none.
fn backups_at(place: Place) -> Result<Vec<BackupEntry>> {
    let folder = backups_in(place);
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("read {}", folder.display())),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("read {}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !entry.file_type().with_context(|| format!("look at {}", entry.path().display()))?.is_dir() {
            continue;
        }
        let held = skyward_backup::Manifest::read(&entry.path()).map(|m| held(&m)).map_err(|e| format!("{e:#}"));
        found.push(BackupEntry { place, name, held });
    }
    Ok(found)
}
备份“就位”：每个文件夹在其备份文件夹里，记录其///清单记录或原因，无法读取。没有备份的地方///文件夹没有备份。fn backups_at（place： Place） -> Result<Vec<BackupEntry>> {let folder = backups_in（place）; let entries = match fs：：read_dir（&folder） { Ok（entrys） =>条目，Err（e） if e.kind（） == std：：io：：ErrorKind：：NotFound => return Ok（Vec：：new（））， Err（e） => return Err（e）.with_context（|| format！（”read {}“， folder.display（））， };令 mut found = Vec：：new（）;对于条目 { let entry = entry.with_context（|| format！（”read {}“， folder.display（））？;设 name = entry.file_name（）.to_string_lossy（）.into_owned（）;如果 name.starts_with（'.'） ||！entry.file_type（）.with_context（|| 格式！（”请查看{}“，entry.path（）.display（））？。is_dir（） { 继续; } let held = skyward_backup：：Manifest：：read（&entry.path（））.map（|m| held （&m））.map_err（|e| format！（”{e：#}“））;found.push（BackupEntry { place， name， held }）;} Ok（发现） }

/// What a backup's manifest records, as the pages show it.
fn held(m: &skyward_backup::Manifest) -> Held {
    Held {
        made: m.made,
        size: m.size(),
        parts: m.parts(),
        device: m.device.clone(),
        system: m.system.clone(),
        slot: skyward_vab::Slot::from_suffix(&m.slot),
    }
}
备份清单记录了什么，正如页面所示。fn held（m： &skyward_backup：：Manifest） -> Held { Held { made： m.made， size： m.size（）， parts： m.parts（）， device： m.device.clone（）， system： m.system.clone（）， slot： skyward_vab：：Slot：：from_suffix（&m.slot）， }

/// Look for backups in the places asked, the restore page showing what is
/// found and saying why a place could not be looked in.
fn find_backups(app: &mut App, storage: bool, usb: bool, now: u64) {
    let mut found = Vec::new();
    for (place, look) in [(Place::Storage, storage), (Place::UsbDrive, usb)] {
        if look {
            match backups_at(place) {
                Ok(mut there) => found.append(&mut there),
                Err(e) => app.report(&format!("{e:#}"), now),
            }
        }
    }
    app.set_backups(found);
}
在被要求的地方找备份，比如显示发现了哪些///的恢复页面，并说明为什么某个地方无法被查看。fn find_backups（app： &mut 应用，存储：bool，usb： bool，now： u64） { let mut found = Vec：：new（）; for （place，look） in [（Place：：Storage， storage）， （Place：：UsbDrive， usb）] { if look { match backups_at（place） { Ok（mut there） => find.append（&mut there）， Err（e） => app.report（&format！（”{e：#}“）， now）， } } } app.set_backups（found）;}

/// Delete the backup in `folder` of `place`; a USB drive, shown read-only,
/// is made writable for it and read-only again after.
fn delete_backup(place: Place, folder: &Path) -> Result<()> {
    let drive = Path::new(USB_DRIVE);
    if place == Place::UsbDrive {
        skyward_block::mount::remount_writable(drive, true)?;
    }
    let deleted = fs::remove_dir_all(folder).with_context(|| format!("delete {}", folder.display()));
    if place == Place::UsbDrive {
        let read_only = skyward_block::mount::remount_writable(drive, false);
        deleted?;
        return read_only;
    }
    deleted
}
删除“place”文件夹中的备份;一个显示为只读的 USB 驱动器 /// 使其可写，之后又恢复为只读。fn delete_backup（place： Place， folder： &Path） -> Result<（）> {let drive = Path：：new（USB_DRIVE）; if place == Place：：UsbDrive { skyward_block：：mount：：remount_writable（drive， true）？; } let deleted = fs：：remove_dir_all（folder）.with_context（|| format！（”delete {}“， folder.display（））;if place == Place：：UsbDrive { let read_only = skyward_block：：mount：：remount_writable（驱动器，false）; deleted？; return read_only; } deleted }

/// How backing up or restoring ended, as its page tells it.
fn backup_end(end: backup::End) -> BackupEnd {
    match end {
        backup::End::Saved(folder) => {
            BackupEnd::Saved(folder.file_name().expect("a backup's folder has a name").to_string_lossy().into_owned())
        }
        backup::End::Restored { unmade } => BackupEnd::Restored { unmade: unmade.iter().map(|p| p.display().to_string()).collect() },
        backup::End::Stopped => BackupEnd::Stopped,
        backup::End::Refused(refusal) => BackupEnd::Refused(match refusal {
            backup::Refusal::Room { needed, free } => BackupRefusal::Room { needed, free },
            backup::Refusal::Update(state) => BackupRefusal::Update(state),
            backup::Refusal::OtherPhone(device) => BackupRefusal::OtherPhone(device),
            backup::Refusal::Resized(name) => BackupRefusal::Resized(name),
            backup::Refusal::DataReset => BackupRefusal::DataReset,
        }),
        backup::End::Failed(why) => BackupEnd::Failed(why),
    }
}
正如其页面所述，备份或恢复的结局。fn backup_end（end： backup：：End） -> BackupEnd { match end { backup：：End：：Saved（文件夹） => { BackupEnd：：Saved（folder.file_name（）.expect（“备份文件夹有名称”）.to_string_lossy（）.into_owned（）） } backup：：End：：Restored { unmade } => BackupEnd：：Restored { unmade： unmade.iter（）.map（|p| p.display（）.to_string（）.collect（） }， backup：：End：：Stopped => BackupEnd：：Stopped， backup：：End：：Refused（拒绝） => BackupEnd：：Refused（匹配拒绝 { backup：：拒绝：：房间 { 需要，空闲 } => 备份拒绝：：房间 { 需要，空闲 }， 备份：：拒绝：：更新（状态） => 备份拒绝：：更新（状态）， 备份：：拒绝：：其他电话（设备） => 备份拒绝：：其他电话（设备），备份：：拒绝：：缩小（姓名） => 备份拒绝：：调整大小（姓名），备份：：拒绝：:D ataReset => 备份拒绝：:D ataReset， }）， backup：：结束：：失败（wwhy） => 备份结束：：失败（ww），} }

/// Now, in Unix seconds.
fn unix_now() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH).context("the clock is before 1970")?.as_secs() as i64)
}
现在，用 Unix 秒数计算。fn unix_now（） -> 结果 <i64> { Ok（SystemTime：：now（）.duration_since（UNIX_EPOCH）.context（“时钟时间在 1970 年前”）？as_secs（） 作为 i64） }

/// Now, in Unix milliseconds.
fn unix_ms_now() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH).context("the clock is before 1970")?.as_millis() as i64)
}
现在，用 Unix 毫秒计。fn unix_ms_now（） -> 结果 <i64> { Ok（SystemTime：：now（）.duration_since（UNIX_EPOCH）.context（“时钟时间在 1970 年前”）？as_millis（） 作为 i64） }

/// The USB drive plugged in, and whether it is shown at [`USB_DRIVE`] or
/// why it could not be.
struct Plugged {
    drive: Drive,
    shown: Result<(), String>,
}
USB 驱动器插入，以及它是否显示在['USB_DRIVE']或///，为什么不可能。struct Plugged { drive： Drive， shown： Result<（）， String>， }

/// Look for the USB drive plugged in: one gone is taken away from where it
/// was shown, even with files on it still open, and one found in two looks
/// running is shown, the kernel having listed its partitions by then.
/// `seen` is a drive found by the look before that is not shown yet. What
/// the pages are told comes back when it changed.
fn look_for_drive(plugged: &mut Option<Plugged>, seen: &mut Option<String>) -> Result<Option<Option<UsbDrive>>> {
    let found = usb::drives()?.into_iter().find(Drive::ready);
    if let Some(p) = plugged {
        if found.is_some_and(|d| d.disk == p.drive.disk) {
            return Ok(None);
        }
        if p.shown.is_ok() {
            skyward_block::mount::umount_detach(Path::new(USB_DRIVE))?;
        }
        skyward_sys::kmsg(&format!("USB drive {} is gone", p.drive.disk))?;
        *plugged = None;
        *seen = None;
        return Ok(Some(None));
    }
    let Some(drive) = found else {
        *seen = None;
        return Ok(None);
    };
    if seen.as_deref() != Some(drive.disk.as_str()) {
        *seen = Some(drive.disk.clone());
        return Ok(None);
    }
    let shown = match usb::mount(&drive, Path::new(USB_DRIVE)) {
        Ok((part, filesystem)) => {
            skyward_sys::kmsg(&format!("USB drive {part} shown at {USB_DRIVE} as {filesystem}"))?;
            Ok(())
        }
        Err(e) => {
            skyward_sys::kmsg(&format!("USB drive {} not shown: {e:#}", drive.disk))?;
            Err(format!("{e:#}"))
        }
    };
    let told = match &shown {
        Ok(()) => UsbDrive::Shown,
        Err(why) => UsbDrive::Unreadable(why.clone()),
    };
    *plugged = Some(Plugged { drive, shown });
    *seen = None;
    Ok(Some(Some(told)))
}
找插着的 U 盘：一个丢失的被从显示的位置拿走了，即使文件还在开着，另一个在两个画面中出现///运行，内核已经列出了分区。“看到”是之前的视觉发现的驱动，尚未展示。页面所说的内容会随着内容变化而回归。fn look_for_drive（plugged： &mut Option<Plugged>， seen： &mut Option<String>） -> Result<Option<Option<UsbDrive>>> { let found = usb：:d rives（）？.into_iter（）.find（Drive：：ready）;if let Some（p） = plugged { if found.is_some_and（|d| d.disk == p.drive.disk） { return Ok（None）; } if p.shown.is_ok（） { skyward_block：：mount：：umount_detach（Path：：new（USB_DRIVE）？; } skyward_sys：：kmsg（&format！（”USB 驱动器{}已消失“，p.drive.disk）？;*已插 = 无;*已见 = 无;返回 Ok（Some（None））;} 让 Some（驱动） = 找到 否则 { *seen = 无;返回 Ok（None）; };如果 seen.as_deref（） ！= Some（drive.disk.as_str（）） { *seen = Some（drive.disk.clone（））; return Ok（None）; } let shown = match usb：：mount（&drive， Path：：new（USB_DRIVE）） { Ok（（part， filesystem）） => { skyward_sys：：kmsg（&format！（”USB 驱动器 {part} 在 {USB_DRIVE} 显示为 {filesystem}“）？;Ok（（）） } Err（e） => { skyward_sys：：kmsg（&format！（”USB 驱动器 {} 未显示： {e：#}“， drive.disk）？;Err（format！（”{e：#}“））} };let told = match &shown { Ok（（）） => U 盘：：Shown， Err（why） => U 盘：：Unreadable（why.clone（））， };*plugged = Some（Plugged { 驱动器，显示}）;*已见 = 无;好（Some（Some（told））））}

/// What a choice on the clash page asks of the work on files.
fn clash_rule(choice: ClashChoice) -> skyward_files::Clash {
    match choice {
        ClashChoice::Replace => skyward_files::Clash::Replace,
        ClashChoice::Skip => skyward_files::Clash::Skip,
        ClashChoice::KeepBoth => skyward_files::Clash::KeepBoth,
    }
}
冲突页面上的选择对文件工作提出了多么多么重要。fn clash_rule（选择：ClashChoice） -> skyward_files：：Clash { 比赛选择 { ClashChoice：：替换 => skyward_files：：冲突：：替换，冲突选择：：跳过 => skyward_files：：冲突：：跳过，冲突选择：：保持两者 => skyward_files：：冲突：：保持两者，} }

/// What a copy or move into the folder `into` gives the entries it makes.
/// On the data partition each takes the owner, group and label the folder
/// gives its own, as the system makes them there; in shared storage, the
/// folder's permissions as well. Elsewhere each keeps what it had.
fn ownership(into: &Path) -> Result<Ownership> {
    use skyward_crypto::data;
    let at = Path::new(data::MOUNT);
    if !skyward_block::mount::is_mount_point(at)? {
        return Ok(Ownership::Kept);
    }
    let here = fs::metadata(into).with_context(|| format!("look at {}", into.display()))?;
    if here.dev() != fs::metadata(at).with_context(|| format!("look at {}", at.display()))?.dev() {
        return Ok(Ownership::Kept);
    }
    let real = fs::canonicalize(into).with_context(|| format!("find {}", into.display()))?;
    let shared = [data::SHARED_STORAGE, data::SDCARD].iter().any(|s| real.starts_with(s));
    Ok(if shared { Ownership::Shared } else { Ownership::Folder })
}
复制或移动到文件夹“into”中所产生的条目。在数据分区上，每个文件夹会接收所有者、组和标签，文件夹 /// 会根据系统生成的标签;在共享存储中，/// 文件夹的权限也一样。其他地方则保留原有的。fn 所有权（into： &Path） -> 结果 <Ownership> { 使用 skyward_crypto：:d ata; let at = Path：：new（data：：MOUNT）; if ！skyward_block：：mount：：is_mount_point（at）？{ return Ok（Ownership：：Kept）; }令此 = FS：：metadata（into）.with_context（|| format！（”请查看{}“，into.display（））？;如果 here.dev（） ！= FS：：metadata（at）.with_context（|| 格式！（”请查看{}“，at.display（））？。dev（） { return Ok（Ownership：：Kept）; } let real = fs：：canonicalize（into）.with_context（|| format！（”find {}“， into.display（））？;让 shared = [data：：SHARED_STORAGE， data：：SDCARD].iter（）.any（|s| real.starts_with（s））;好（如果共享 { 所有权：：共享} 否则 { 所有权：：文件夹 }） }

/// The page's name for a part erasing goes through.
fn part(step: Step) -> Part {
    match step {
        Step::Data => Part::Data,
        Step::Cache => Part::Cache,
        Step::Metadata => Part::Metadata,
        Step::AppData => Part::AppData,
        Step::Media => Part::Media,
        Step::AppCaches => Part::AppCaches,
        Step::SystemCaches => Part::SystemCaches,
        Step::Art => Part::Art,
        Step::Logs => Part::Logs,
        Step::Updates => Part::Updates,
    }
}
页面的擦除名称通过了。fn part（step： Step） -> Part { match step { step：:D ata => Part：:D ata， Step：：Cache => Part：：Cache， step：：Metadata => Part：：Metadata， step：：AppData => Part：：AppData， step：：Media => Part：：Media， step：：AppCaches => Part：：AppCaches， step：：SystemCaches => Part：：SystemCaches， Step：：Art => Part：：Art， Step：：Logs => Part：：Logs， Step：：Updates => Part：：Updates， } }

/// The part of the phone a choice of the wipe page other than everything
/// erases.
fn step(choice: Wipe) -> Step {
    match choice {
        Wipe::AppData => Step::AppData,
        Wipe::Media => Step::Media,
        Wipe::AppCaches => Step::AppCaches,
        Wipe::SystemCaches => Step::SystemCaches,
        Wipe::Art => Step::Art,
        Wipe::Cache => Step::Cache,
        Wipe::Logs => Step::Logs,
        Wipe::Updates => Step::Updates,
        Wipe::All => unreachable!("everything is a factory reset, not a part of one"),
    }
}
手机上除了所有擦除页面外的部分，///会被删除。fn step（choice： Wipe） -> step { match choice { Wipe：：AppData => step：：AppData， Wipe：：Media => step：：Media， Wipe：：AppCaches => step：：AppCaches， Wipe：：SystemCaches => step：：SystemCaches， Wipe：：Art => step：：Art， Wipe：：Cache => step：：Cache， wipe：：Logs => Step：：Logs， Wipe：：Updates => Step：：Updates， Wipe：：All => 无法访问！（”一切都是出厂重置，而不是恢复出厂设置的一部分“），} }

/// How erasing stopped, as its page tells it.
fn erase_stop(stop: wipe::Stop) -> EraseStop {
    match stop {
        wipe::Stop::Refused(wipe::Refusal::Update(hold)) => EraseStop::Update(hold),
        wipe::Stop::Refused(wipe::Refusal::NotRebuilt) => EraseStop::NotRebuilt,
        wipe::Stop::Refused(wipe::Refusal::NoCache) => EraseStop::NoCache,
        wipe::Stop::Closed(why) => EraseStop::Closed(why),
        wipe::Stop::Locked(users) => EraseStop::Locked(users),
        wipe::Stop::Failed(step, e) => EraseStop::Failed(part(step), format!("{e:#}")),
    }
}
正如其页面所述，抹除是如何停止的。FN erase_stop（停止：清除：：：） 停止） -> EraseStop { 匹配停止 { wipe：：Stop：：Refused（wipe：：Refusal：：Update（hold））） => EraseStop：：Update（hold），wipe：：Stop：：Refused（wipe：Refusal：：NotRebuilt） => EraseStop：：NotRebuild，wipe：：Stop：：Refused（wipe：Refusal：：NoCache） => EraseStop：：NoCache，wipe：：Stop：：Closed（why） => EraseStop：：Closed（why），wipe：：Stop：：Locked（users） => EraseStop：：Locked（users），wipe：：Stop：：Failed（step， e） => EraseStop：：Failed（part（step）， 格式！（“{e：#}”））， } }

/// Start erasing all user data, its page following it; why it cannot
/// start ends it at once, on the same page. `asked` when the system asked
/// for it; `reformat` the filesystem every part is made afresh as, where
/// one was asked for.
fn factory_reset(app: &mut App, keep_memtag_mode: bool, asked: bool, reformat: Option<String>) -> Result<Eraser> {
    match wipe::factory_reset(reformat) {
        Ok(plan) => {
            app.erase_started(true, asked, plan.steps().into_iter().map(part).collect());
            Eraser::start(move |tell| plan.carry_out(keep_memtag_mode, tell))
        }
        Err(stop) => {
            app.erase_started(true, asked, Vec::new());
            Eraser::stopped(stop)
        }
    }
}
开始删除所有用户数据，其页面紧随其后;为什么它不能 /// 开始，这句话一结束，在同一页上。当系统请求///时“请求”;“重格式化”文件系统，每个部分都重新创建为 ///，其中一个是被要求的。fn factory_reset（app： &mut 应用，keep_memtag_mode： bool， asked： bool， reformat： Option<String>） -><Eraser> 结果 { match wipe：：factory_reset（reformat） { Ok（plan） => { app.erase_started（true， asked， plan.steps（）.into_iter（）.map（part）.collect（））;Eraser：：start（move |tell| plan.carry_out（keep_memtag_mode， tell）） } Err（stop） => { app.erase_started（true， asked， Vec：：new（））;Eraser：：stopped（stop） } } } }

/// Start erasing all user data for rescue, the installing page saying so
/// meanwhile: as a factory reset erases it, the memory tagging asked of the
/// bootloader forgotten too.
fn rescue_wipe(app: &mut App) -> Result<Eraser> {
    app.rescue_wiping();
    match wipe::factory_reset(None) {
        Ok(plan) => Eraser::start(move |tell| plan.carry_out(false, tell)),
        Err(stop) => Eraser::stopped(stop),
    }
}
开始清除所有用户数据以便救援，安装页面会显示这样做 /// 同时：随着恢复出厂设置，清除了数据，启动加载程序请求的内存标记也会被遗忘。fn rescue_wipe（app： &mut App） -> 结果 <Eraser> { app.rescue_wiping（）; 匹配清除：：factory_reset（无） { Ok（plan） => Eraser：：start（move |tell| plan.carry_out（false， tell））， Err（stop） => Eraser：：stopped（stop）， } }

/// Start erasing parts on their own, its page following it: parts of the
/// data partition, and the cache partition, made afresh as `reformat`
/// where the system asked for that filesystem; `logs` are the phone's own
/// log folders. `asked` when the system asked for it.
fn parts_wipe(app: &mut App, steps: Vec<Step>, logs: &[String], asked: bool, reformat: Option<String>) -> Result<Eraser> {
    match wipe::parts(steps, logs, reformat) {
        Ok(parts) => {
            app.erase_started(false, asked, parts.steps().into_iter().map(part).collect());
            Eraser::start(move |tell| parts.carry_out(tell))
        }
        Err(stop) => {
            app.erase_started(false, asked, Vec::new());
            Eraser::stopped(stop)
        }
    }
}
开始单独擦除部分，其页面紧随其后：/// 数据分区和缓存分区的部分，重新创建为“格式化”/，系统要求该文件系统;“日志”是手机自己的///日志文件夹。系统要求时才“问”。fn parts_wipe（app： &mut 应用，steps： <Step>Vec，logs： &[String]， asked： bool， reformat： Option<String>） -> Result<Eraser> { match wipe：:p arts（steps， logs， reformat） { Ok（parts） => { app.erase_started（false， asked， parts.steps（）.into_iter（）.map（part）.collect（））;Eraser：：start（move |tell| parts.carry_out（tell）） } Err（stop） => { app.erase_started（false， asked， Vec：：new（））;Eraser：：stopped（stop） } } } }

/// The reasons the bootloader gives for a start that followed the kernel
/// stopping, as Android's own recovery lists them; it does not start an
/// install the system asked for before such a start.
const PANICS: [&str; 2] = ["kernel_panic", "Panic"];
引导加载程序给出的启动原因跟随内核 /// 停止，正如 Android 自身的恢复系统所列出的;它不会启动///安装系统要求的启动。const panics： [&str; 2] = [“kernel_panic”， “Panic”];

/// Whether the phone came to the recovery after its kernel stopped.
pub fn panicked() -> bool {
    skyward_sys::prop("ro.boot.bootreason").is_some_and(|reason| PANICS.iter().any(|p| p.eq_ignore_ascii_case(&reason)))
}
手机是否在内核停止后进入恢复状态。Pub fn panicked（） -> bool { skyward_sys：:p rop（“ro.boot.bootreason”）.is_some_and（|reason|PANICS.iter（）.any（|p| p.eq_ignore_ascii_case（&reason）））}

/// An install the system asked for that waits for the data partition to
/// open, or for the user whose own files hold the package to unlock them.
struct Pending {
    package: asked::Asked,
    needs: Needs,
    retry: bool,
}
系统要求安装，等待数据分区 /// 打开，或者等待持有该包文件的用户解锁。struct Pending { package： asked：：Asked， needs： Needs， retry： bool， }

/// Start installing the package the system asked for.
fn install_asked(cfg: &DeviceConfig, app: &App, pending: Pending) -> Result<Installer> {
    if pending.needs == Needs::Cache {
        let cache = wipe::cache()?.context("the phone has no cache partition")?;
        let at = Path::new(&cache.mount_point);
        if !mount::is_mount_point(at)? {
            fs::create_dir_all(at).with_context(|| format!("make {}", at.display()))?;
            mount::mount_ro(Path::new(&fstab::device(&cache)?), at)?;
        }
    }
    Installer::asked(pending.package, pending.retry, detach(cfg.services.clone()), install_options(app))
}
开始安装系统要求的软件包。fn install_asked（cfg： &DeviceConfig， app： &App， pending： Pending） -> 结果 <Installer> { if pending.needs == Needs：：Cache { let cache = wipe：：cache（）？.上下文（“手机没有缓存分区”）？令 = 路径：：new（&cache.mount_point）;如果 ！mount：：is_mount_point（at）？{ fs：：create_dir_all（at）.with_context（|| format！（”make {}“， at.display（））？;mount：：mount_ro（Path：：new（&fstab：:d evice（&cache）？）， at）？;} } Installer：：asked（pending.package， pending.retry， detach（cfg.services.clone（））， install_options（app）） }

/// Where a reboot rescue asked for goes: the target, and whether it is
/// rescue again, which the recovery is told in misc.
fn rescue_target(target: &str) -> (Target, bool) {
    match target {
        "bootloader" => (Target::Bootloader, false),
        "fastboot" => (Target::Fastboot, false),
        "recovery" => (Target::Recovery, false),
        "rescue" => (Target::Recovery, true),
        // As minadbd has it, anything else starts the system.
        _ => (Target::System, false),
    }
}
请求重启救援的顺序为：目标，以及是否再次是 /// 救援，恢复用 misc. fn rescue_target（target： &str） -> （Target， bool） { match target { “bootloader” = > （Target：：Bootloader， false）， “fastboot” => （Target：：Fastboot， false）， “recovery” => （Target：：Recovery， false）， “rescue” => （Target：：Recovery， true）， // 根据 minadbd 的设定，任何其他情况都会启动系统。_ => （Target：：系统，false）， } }

/// How a partition's work ended, as its page tells it.
fn item_end(item: partwork::Item) -> ItemEnd {
    match item {
        partwork::Item::Erased => ItemEnd::Erased,
        partwork::Item::Unshared => ItemEnd::Unshared,
        partwork::Item::NotShared(filesystem) => ItemEnd::NotShared(filesystem),
        partwork::Item::Fixed(Fixed::Clean) => ItemEnd::Clean,
        partwork::Item::Fixed(Fixed::Fixed) => ItemEnd::Fixed,
        partwork::Item::Made => ItemEnd::Made,
        // What the program said shows below.
        partwork::Item::Fixed(Fixed::Failed) => ItemEnd::Failed(None),
        partwork::Item::Failed(why) => ItemEnd::Failed(Some(why)),
    }
}
分区的工作如何结束，正如其页面所述。FN item_end（项目：partwork：：） Item） -> ItemEnd { match item { partwork：：Item：：Erased => ItemEnd：：Erased， partwork：：Item：：Unshared => ItemEnd：：Unshared， partwork：：Item：：NotShared（文件系统） => ItemEnd：：NotShared（文件系统），partwork：：Item：：Fixed（Fixed：：Clean） => ItemEnd：：Clean， partwork：：Item：：Fixed（Fixed：：Fixed） => ItemEnd：：Fixed， partwork：：Item：：Made => ItemEnd：：Made， // 程序所说如下所示。partwork：：Item：：Fixed（Fixed：：Failed） => ItemEnd：：失败（无），partwork：：Item：：Failed（why） => ItemEnd：：Failed（Some（why））， } }

/// Why work on whole partitions stopped, as its page tells it.
fn part_stop(stop: partwork::Stop) -> PartStop {
    match stop {
        partwork::Stop::Refused(refusal) => PartStop::Refused(refusal),
        partwork::Stop::Unmounted(base) => PartStop::Unmounted(base),
        partwork::Stop::Failed(why) => PartStop::Failed(why),
    }
}
为什么整个分区的工作停止了，正如其页面所说。fn part_stop（stop： partwork：：Stop） -> PartStop { match stop { partwork：：Stop：：Refused（拒绝） => PartStop：：Refused（拒绝），partwork：：Stop：：Unmounted（base） => PartStop：：Unmounted（base），partwork：：Stop：：Failed（why） => PartStop：：Failed（why）， } }

/// The partitions the filesystem page lists, in this order, where the
/// phone's table has them: the data partition, /metadata and the cache
/// partition. Each comes with what it holds now, its size, and the other
/// filesystems the table lists for it that the recovery makes. The data
/// partition reads only through its decryption, so what it holds is known
/// only once it is open; until then, any of the table's filesystems may be
/// it, and where the table lists one, it is that. /metadata keeps the key
/// the data partition opens with, which making it afresh would lose, so it
/// is offered no other.
fn read_filesystems() -> Result<Vec<FsRow>> {
    let mut rows = Vec::new();
    for (name, point) in [(DATA_PARTITION, "/data"), ("metadata", "/metadata"), ("cache", "/cache")] {
        let entries = fstab::read(point)?;
        let Some(entry) = entries.first() else { continue };
        let device = fstab::device(entry)?;
        let size = skyward_block::size_bytes(&device)?;
        let mut listed: Vec<String> = Vec::new();
        for e in &entries {
            if REFORMATS.contains(&e.filesystem.as_str()) && !listed.contains(&e.filesystem) {
                listed.push(e.filesystem.clone());
            }
        }
        let filesystem = match name {
            DATA_PARTITION => mount::visible(Path::new(point))?.map(|v| v.filesystem),
            _ => probe::filesystem(Path::new(&device))?,
        };
        let others = match (name, &filesystem) {
            ("metadata", _) => Vec::new(),
            (_, Some(held)) => listed.into_iter().filter(|fs| fs != held).collect(),
            (_, None) if listed.len() > 1 => listed,
            (_, None) => Vec::new(),
        };
        rows.push(FsRow { name: name.to_string(), mount_point: point.to_string(), filesystem, size, others });
    }
    Ok(rows)
}
文件系统页面列出的分区按顺序排列，按 /// 电话表中出现的分区：数据分区、/metadata 和缓存 /// 分区。每个文件都包含当前存储的内容、大小，以及恢复时表列出的其他 /// 文件系统。数据分区 /// 只读取解密后的内容，因此它所持有的内容只有在打开时才会知道 /// ;在此之前，表中的任何文件系统都可以是 ///，表中列出的就是 ///。/元数据保留了数据分区开启时使用的密钥 ///，重新创建会失去密钥，因此 /// 没有其他密钥。fn read_filesystems（） -> 结果 <Vec<FsRow>> { let mut 行 = Vec：：new（）; 对于 [（DATA_PARTITION， “/data”）中的（name，point），（“metadata”， “/metadata”）（“cache”， “/cache”）] { let entry = fstab：：read（point）？; let Some（entry） = entry.first（） 否则 { continue }; let device = fstab：:d evice（entry）？; let size = skyward_block：：size_bytes（&device）？;let mut listed： Vec<String>= Vec：：new（）;对于 e in &entries { 如果 REFORMATS.contains（&e.filesystem.as_str（）） && ！listed.contains（&e.filesystem） { listed.push（e.filesystem.clone（））; } } let filesystem = match name { DATA_PARTITION => mount：：visible（Path：：new（point））？.map（|v| v.filesystem）， _ => probe：：filesystem（Path：：new（&device））？， };让 others = match （name， &filesystem） {（“metadata”， _） => Vec：：new（）， （_， Some（held d）） = > listed.into_iter（）.filter（|fs| fs ！= held d）.collect（）， （_， None） if listed.len（） > 1 => listed， （_， None） => Vec：：new（）， };rows.push（FsRow { 名称：name.to_string（）， mount_point： point.to_string（）， filesystem， size， others }）;} 好的（行） }

/// The filesystem page's partition `name` for its work: the data partition,
/// or the table's first entry for another.
fn fs_part(name: &str) -> Result<partwork::Part> {
    if name == DATA_PARTITION {
        return Ok(partwork::Part::Data);
    }
    let point = format!("/{name}");
    let entry = fstab::read(&point)?.into_iter().next().with_context(|| format!("the phone's table has no {point}"))?;
    Ok(partwork::Part::Entry(entry))
}
文件系统页面的分区“名称”用于其工作：数据分区，/// 或表中另一个条目的第一条目。fn fs_part（name： &str） -> Result<partwork：:P art> { if name == DATA_PARTITION { return Ok（partwork：:P art：:D ata）; } let point = format！（”/{name}“）;让 entry = fstab：：read（&point）？.into_iter（）.next（）.with_context（|| format！（”电话桌没有{point}“））？;Ok（partwork：:P art：：Entry（entry）） }

/// What the pages act on: a touch or a key, what a program asked on the
/// recovery socket, or a USB drive plugged in or pulled out.
enum Incoming {
    /// The page shown first, which may ask for work as it shows.
    Arrived,
    Input(Event),
    Asked(Request),
    Drive(Option<UsbDrive>),
}
这些页面的作用对象是：触摸或按键，程序在////恢复插座上请求的内容，或者插入或拔出的 USB 驱动器。enum Incoming { /// 页面最先显示，可能会要求工作，如图所示。到达，输入（事件），询问（请求），驱动（选项 <UsbDrive>），}

/// What a request on the recovery socket comes to. adbd asks for fastboot
/// while the slot's partitions are up, which fastbootd cannot work with, so
/// the phone restarts into fastboot as from the reboot page; fastbootd asks
/// for the recovery it runs in. A request that finds the recovery already
/// where it asks to be, as when the keys left fastboot first, needs nothing.
fn answer(request: Request, fastboot: bool) -> Vec<Effect> {
    match (request, fastboot) {
        (Request::Fastboot, false) => vec![Effect::Reboot(Target::Fastboot)],
        (Request::Recovery, true) => vec![Effect::EnterRecovery],
        _ => Vec::new(),
    }
}
恢复插座的请求真是太棒了。adbd 在插槽分区开启时要求 fastboot /// ，但 fastbootd 无法使用，因此手机会根据重启页面重启进入 fastboot;Fastbootd 会询问 /// 运行的恢复程序。如果请求已经在它要求的位置找到恢复，比如密钥先离开 fastboot 时，不需要任何信息。fn 回答（请求：请求，fastboot： bool） -> Vec<Effect> { match （request， fastboot） { （请求：：fastboot， false） => vec！[效果：：Reboot（Target：：Fastboot）]， （请求：：恢复，true） => vec！[效果：：进入恢复]， _ => Vec：：new（）， } }

/// The time of day for the status bar, in the zone the settings chose.
fn clock(zone: usize) -> Result<String> {
    Ok(skyward_ui::fmt::clock(unix_now()?, zone))
}
状态栏的时间，在设置选择的区域里。fn clock（区域：usize） -> 结果 <String> { Ok（skyward_ui：：fmt：：clock（unix_now（）？， zone）） }

/// Tell the kernel log how the data partition was mounted, where opening
/// it mounted it: the check made first, in full where it found something,
/// and the device it is read through.
fn log_mounted(mounted: &Opened) -> Result<()> {
    if let Some(checked) = &mounted.checked {
        skyward_sys::kmsg(&format!("the check of {} ended with {}", mounted.mapped.display(), checked.status))?;
        if checked.status != 0 {
            for line in checked.said.lines() {
                skyward_sys::kmsg(line)?;
            }
        }
    }
    skyward_sys::kmsg(&format!("{} holds {} at {}", mounted.mapped.display(), mounted.filesystem, skyward_crypto::data::MOUNT))
}
告诉内核日志数据分区是如何挂载的，打开 /// 挂载时：先检查，完整检查发现了什么，/// 以及读取的设备。fn log_mounted（mounted： &Opened） -> Result<（）> { if let Some（checked） = &mounted.checked { skyward_sys：：kmsg（&format！（”{}的检查以{}“结尾，mounted.mapped.display（）， checked.status）？;if checked.status ！= 0 { 用于 checked.said.lines（） { skyward_sys：：kmsg（line）？; } } }skyward_sys：：kmsg（&format！（”{} 将 {} 保持在 {}“，mounted.mapped.display（），mounted.filesystem，skyward_crypto：:d ata：：MOUNT）） }

/// Show what the data partition still keeps closed: the owner's screen lock,
/// where the owner's own files wait on one, with the control it is answered
/// with, or nothing. Users whose credential the phone made up open behind
/// the owner and are never asked; one that did not open goes to the kernel
/// log with why.
///
/// A further user's own files — a second space's, a cloned app's — are left
/// closed without being asked about: the owner came to open the owner's
/// files, and holding the recovery at such a user's prompt while the
/// owner's files are open behind it helps nobody. The file manager offers to
/// open one when a folder of theirs is opened instead.
fn show_closed(app: &mut App, closed: &[Closed]) -> Result<()> {
    use skyward_crypto::user::Lock;
    skyward_sys::kmsg(&format!("the users whose own files are still closed: {:?}", closed.iter().map(|c| c.user).collect::<Vec<_>>()))?;
    for c in closed {
        if let Some(why) = &c.why {
            skyward_sys::kmsg(&format!("the files of user {} stay closed: {why}", c.user))?;
        }
    }
    let asking = skyward_crypto::users::ask(&waiting_of(closed));
    let waiting = asking.and_then(|user| {
        closed.iter().find(|c| c.user == user).map(|c| {
            let keys = match c.lock {
                Lock::PATTERN => Keys::Pattern,
                Lock::PIN => Keys::Digits,
                _ => Keys::Letters,
            };
            (c.user, keys)
        })
    });
    // Those the page offers by name rather than asks for: every user still
    // closed that the phone did not make a credential up for, but not the
    // one being asked for.
    let others = closed
        .iter()
        .filter(|c| !c.made_up && Some(c.user) != asking)
        .map(|c| {
            let keys = match c.lock {
                Lock::PATTERN => Keys::Pattern,
                Lock::PIN => Keys::Digits,
                _ => Keys::Letters,
            };
            (c.user, keys)
        })
        .collect();
    app.data_waiting(waiting, others);
    Ok(())
}
显示数据分区仍然关闭的部分：拥有者屏幕锁、/// 拥有者自己的文件等待的区域，以及用 /// 回应的控制，或者什么都没有。手机虚构凭证的用户在///后方打开，且从未被询问;未打开的那个会被内核 /// 日志记录，原因如下。/// 另一个用户自己的文件——第二个空间的，一个克隆的应用——被关闭了，没有人询问：所有者来打开拥有者的///文件，而在////所有者文件打开时，将恢复文件按在用户提示下，且////所有者的文件还在后面打开，这对任何人都没有帮助。文件管理器会在打开他们文件夹时提供 /// 打开一个。fn show_closed（app： &mut 应用，关闭： &[Closed]） -> Result<（）> { 使用 skyward_crypto：：user：：Lock; skyward_sys：：kmsg（&format！（”那些自己文件仍然关闭的用户：{：？}“，closed.iter（）.map（|c| c.user）.collect：：<Vec<_>>（）））？;对于闭合的 c { 如果让 Some（why） = &c.why { skyward_sys：：kmsg（&format！（”用户 {} 的文件保持关闭：{why}“，c.user））？;} } 让 asking = skyward_crypto：：users：ask（&waiting_of（关闭））;令等待 = asking.and_then（|用户|{ 关闭。iter（）.find（|c| c.user == user）.map（|c|{ let keys = match c.lock { Lock：:P ATTERN => Keys：:P attern， Lock：:P IN => Keys：:D igits， _ => Keys：：Letters， };（c.user，键）}）});页面会按名字提供，而不是直接要求：每个用户仍然关闭了手机没有创建凭证的用户，但没有关闭被要求的//凭证。让 others = 关闭 .iter（） .filter（|c| ！c.made_up & Some（c.user） ！= 询问） .map（|c|{ let keys = match c.lock { Lock：:P ATTERN => Keys：:P attern， Lock：:P IN => Keys：:D igits， _ => Keys：：Letters， };（c.user，键）}）.collect（）;app.data_waiting（等待，其他人）;Ok（（）） }

/// The users `closed` names, as much of each as the choice between them
/// needs.
fn waiting_of(closed: &[Closed]) -> Vec<skyward_crypto::users::Waiting> {
    closed.iter().map(|c| skyward_crypto::users::Waiting { user: c.user, made_up: c.made_up }).collect()
}
用户“关闭”了所有名字，根据 /// 需要选择每个名字。fn waiting_of（关闭：&[关闭]） -> Vec<skyward_crypto：：users：：等待> { closed.iter（）.map（|c| skyward_crypto：：users：：等待 { 用户： c.user， made_up： c.made_up }）.collect（） }

/// What the file manager lists for a folder: its entries, folders and
/// links to folders first, then by name ignoring case, each folder with how
/// many entries it holds. An entry gone by the time it is looked at, as
/// processes in /proc go, is left out.
fn read_folder(path: &Path) -> Result<Vec<FileRow>> {
    let gone = |e: &std::io::Error| e.kind() == std::io::ErrorKind::NotFound;
    let mut rows = Vec::new();
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let full = entry.with_context(|| format!("read {}", path.display()))?.path();
        let meta = match fs::symlink_metadata(&full) {
            Ok(meta) => meta,
            Err(e) if gone(&e) => continue,
            Err(e) => return Err(e).with_context(|| format!("stat {}", full.display())),
        };
        let kind = if meta.file_type().is_symlink() {
            let target = fs::read_link(&full).with_context(|| format!("read the link {}", full.display()))?;
            // A link to nothing leads to no folder.
            let folder = match fs::metadata(&full) {
                Ok(to) => to.is_dir(),
                Err(e) if gone(&e) => false,
                Err(e) => return Err(e).with_context(|| format!("follow {}", full.display())),
            };
            FileKind::Link { target: target.to_string_lossy().into_owned(), folder }
        } else if meta.is_dir() {
            // A folder that may not be read shows that in place of a count.
            let items = match fs::read_dir(&full) {
                Ok(inside) => Some(inside.count()),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => None,
                Err(e) if gone(&e) => continue,
                Err(e) => return Err(e).with_context(|| format!("read {}", full.display())),
            };
            FileKind::Folder { items }
        } else if meta.is_file() {
            FileKind::File
        } else {
            FileKind::Special
        };
        let name = full.file_name().with_context(|| format!("{} has no name", full.display()))?.to_string_lossy().into_owned();
        rows.push(FileRow { name, kind, size: meta.len(), modified: meta.mtime(), mode: meta.mode(), uid: meta.uid(), gid: meta.gid() });
    }
    rows.sort_by_key(|r| (!matches!(r.kind, FileKind::Folder { .. } | FileKind::Link { folder: true, .. }), r.name.to_lowercase()));
    Ok(rows)
}
文件管理器列出的文件夹内容：它的条目、文件夹和 /// 先链接到文件夹，然后按名称（不考虑大小写）按每个文件夹的 /// 条目数量。在查看时已消失的条目，如 go 中的 /proc /// 进程，则会被省略。fn read_folder（path： &Path） -> Result<Vec<FileRow>> { let gone = |e： &std：：io：：Error| e.kind（） == std：：io：：ErrorKind：：NotFound; let mut rows = Vec：：new（）; 用于 fs：：read_dir（path）.with_context（|| format！（”读 {}“，path.display（）））？{ let full = entry.with_context（|| 格式！（”读 {}“， path.display（））？。path（）;让 meta = 匹配 FS：：symlink_metadata（&full） { Ok（meta） =>元，Err（e） 如果消失（&e） =>继续，Err（e） =>返回 Err（e）.with_context（|| 格式！（”stat {}“， full.display（）））， };令 kind = 如果 meta.file_type（）.is_symlink（） { let target = fs：：read_link（&full）.with_context（|| 格式！（”请阅读链接 {}“，full.display（））？;一个链接到什么都没有，也不会指向任何文件夹。let folder = match fs：：metadata（&full） { Ok（to） => to.is_dir（）， Err（e） if gone（&e） => false， Err（e） => return Err（e）.with_context（|| format！（”跟随 {}“， full.display（）））， };FileKind：：Link { target： target.to_string_lossy（）.into_owned（）， folder } } else if if  meta.is_dir（） { // 一个文件夹可能无法读取，显示该文件夹代替计数。let items = match fs：：read_dir（&full） { Ok（inside） => Some（inside.count（））， Err（e） if e.kind（） == std：：io：：ErrorKind：:P ermissionDenied => 无，Err（e） 如果消失（&e） => 继续，Err（e） =>返回 Err（e）.with_context（|| format！（”read {}“， full.display（））， };FileKind：：Folder { items } } 否则 如果 meta.is_file（） { FileKind：：File } 否则 { FileKind：：Special };令 name = full.file_name（）.with_context（|| 格式！（”{} 没有名称“，full.display（））？。to_string_lossy（）.into_owned（）;排。push（FileRow { 名称， 类型， 大小： meta.len（）， 修改： meta.mtime（）， mode： meta.mode（）， uid： meta.uid（）， gid： meta.gid（） }）;} rows.sort_by_key（|r|（！火柴！（r.kind， FileKind：：Folder { .. } |FileKind：：Link { 文件夹： true， .. }）， r.name。to_lowercase（）））;Ok（rows） }

/// The slot's logical partitions and how each shows now, for the mount page.
/// A mounted partition is in use when a program holds it, or when one of
/// `services` this phone's build of the hardware runs is run from it: those
/// are started again from there whenever they end.
fn read_mounts(services: &[Service]) -> Result<Vec<skyward_ui::app::MountRow>> {
    use skyward_ui::app::MountRow;
    let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
    let shown = skyward_vab::mounted_partitions(slot)?;
    let holders = skyward_block::mount::holders()?;
    let props = skyward_info::installed_props(slot)?;
    let services: Vec<&Service> = services.iter().filter(|s| s.runs_on(&props)).collect();
    let mut rows = Vec::new();
    for base in skyward_vab::slot_bases(slot)? {
        let point = skyward_vab::mount_point(&base);
        let name = format!("{base}{}", slot.suffix());
        let mounted = shown.iter().find(|m| m.partition == name);
        let mut using = BTreeSet::new();
        if let Some(m) = mounted {
            using.extend(holders.get(&skyward_block::devnum(&m.device)?).into_iter().flatten().cloned());
            for s in services.iter().filter(|s| !s.oneshot && s.uses(&point)) {
                let program = Path::new(&s.path).file_name().with_context(|| format!("{} names no program", s.path))?;
                using.insert(program.to_string_lossy().into_owned());
            }
        }
        rows.push(MountRow {
            base,
            point: point.display().to_string(),
            mounted: mounted.is_some(),
            holders: using.into_iter().collect(),
        });
    }
    Ok(rows)
}
插槽的逻辑分区以及现在挂载页面的显示方式。当程序持有该分区，或者当该手机构建的硬件运行的////“服务”运行时，已挂载分区即被使用;这些///在分区结束时会从那里重新启动。fn read_mounts（services： &[Service]） -> Result<Vec<skyward_ui：：app：：MountRow>> { use skyward_ui：：app：：MountRow; let slot = skyward_vab：：current_slot（）？.上下文（“不是 A/B 电话”）？设所示 = skyward_vab：：mounted_partitions（槽）？;令持有者 = skyward_block 1：：mount：：holders（）？;设 props = skyward_info：：installed_props（槽）？;let services： Vec<&Service> = services.iter（）.filter（|s| s.runs_on（&props））.collect（）; let mut rows = Vec：：new（）; for base in skyward_vab：：slot_bases（slot）？{ let point =  skyward_vab：：mount_point（&base）; let name = format！（“{base}{}”，slot.suffix（））;设 mount = shown.iter（）.find（|m| m.partition == name）;令 mut 使用=BTreeSet：：new（）;如果让 Some（m） = mounted，{ 使用.extend（holders.get（&skyward_block：:d evnum（&m.device）？）。into_iter（）.flatten（）.cloneed（））;对于 services.iter（）.filter（|s| ！s.oneshot && s.uses（&point）） { let program = Path：：new（&s.path）.file_name（）.with_context（|| format！（”{} 不命名程序“，s.path））？;using.insert（program.to_string_lossy（）.into_owned（））;} } rows.push（MountRow { base， point： point.display（）.to_string（）， mounted： mounted.is_some（）， holders： using.into_iter（）.collect（）， }）;} 好的（行） }

/// What the phone's boot control holds of the slots, or why it cannot be
/// asked.
fn read_slots() -> Result<skyward_vab::BootSlots, String> {
    let read = || -> Result<skyward_vab::BootSlots> {
        let (binder, hardware) = (skyward_binder::driver::Binder::open()?, skyward_binder::driver::Binder::hardware()?);
        let control = skyward_vab::boot::BootControl::open(&binder, &hardware)?;
        control.slots()
    };
    read().map_err(|e| format!("{e:#}"))
}
手机的启动控制对插槽有什么限制，或者为什么不能用 /// 来询问。fn read_slots（） -> 结果 <skyward_vab：：：BootSlots， String> {let read = || -> 结果 <skyward_vab：：：BootSlots> {let （binder， hardware） = （skyward_binder：:d river：：Binder：：open（）？， skyward_binder：:d river：：Binder：：硬件（）？）; let control = skyward_vab：：boot：：BootControl：：open（&binder， &hardware）？; control.slots（） }; read（）.map_err（|e| format！（”{e：#}“）） }

/// Have the bootloader start `slot` next and give what the boot control
/// holds of the slots then, unless an update is in flight: it set the
/// slot it needs itself.
fn set_slot(slot: skyward_vab::Slot) -> Result<skyward_vab::BootSlots, String> {
    let set = || -> Result<skyward_vab::BootSlots> {
        let state = skyward_vab::snapshot_state()?;
        ensure!(state == skyward_vab::SnapshotState::None, "an update is in flight ({state:?}), and it set the slot it needs");
        let (binder, hardware) = (skyward_binder::driver::Binder::open()?, skyward_binder::driver::Binder::hardware()?);
        let control = skyward_vab::boot::BootControl::open(&binder, &hardware)?;
        control.set_active(slot)?;
        skyward_sys::kmsg(&format!("slot {} starts next", slot.letter()))?;
        control.slots()
    };
    set().map_err(|e| format!("{e:#}"))
}
让引导加载程序接着启动“slot”，然后输入启动控制 /// 对插槽的控制，除非正在进行更新：它自己设置了需要的 /// 槽。fn set_slot（slot： skyward_vab：：Slot） -> 结果 <skyward_vab：：BootSlots， String> {let set = || -> Result<skyward_vab：：BootSlots> {let state = skyward_vab：：snapshot_state（）？; ensure！（state == skyward_vab 1：：SnapshotState：：None，“更新正在执行中（{state：？}），并设置了所需的槽位”）;let （binder， hardware） = （skyward_binder：:d river：：Binder：：open（）？， skyward_binder：:d river：：Binder：：hardware（）？）;let control = skyward_vab：：boot：：BootControl：：open（&binder， &hardware）？;control.set_active（slot）？;skyward_sys：：kmsg（&format！（”槽位 {} 下一个开始“，slot.letter（））？;control.slots（） };set（）.map_err（|e| 格式！（”{e：#}“）） }

/// What has to happen before an update writes over the installed system's
/// partitions: the files the phone's own programs run from stay in memory,
/// and the booted slot's partitions come off.
fn detach(services: Vec<Service>) -> Detach {
    Box::new(move || {
        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
        let mounted = skyward_vab::mounted_partitions(slot)?;
        let busy = services::pin(&services, &mounted)?;
        skyward_vab::release_partitions(slot, &busy)
    })
}
在更新覆盖已安装系统的 /// 分区之前必须做的事情：手机程序运行的文件仍留在内存中，/// 和启动插槽的分区被移除。fn detach（services： Vec<Service>） -> Detach { Box：：new（move ||{ let Slot = skyward_vab：：current_slot（）？.上下文（“不是 A/B 电话”）？设已装 = skyward_vab：：mounted_partitions（槽）？;let busy = services：:p in（&services， &mounted）？;skyward_vab：：release_partitions（时段，且忙中）}） }

/// What an install does besides installing, as the install page's switches
/// are set: puts Skyward back into the recovery partitions it writes over,
/// on a phone that has them, and checks a zip's signature before its
/// install script runs.
fn install_options(app: &App) -> Options {
    Options { keep: app.state.keep_recovery && app.state.recovery_partitions, signed: app.state.check_signature }
}
安装除了安装之外，作为安装页面开关///的设置：将 Skyward 重新放回它写入的恢复分区，在有恢复分区的手机上///，并在///安装脚本运行前检查 zip 的签名。fn install_options（app： &App） -> Options { Options { keep： app.state.keep_recovery & app.state.recovery_partitions，签名：app.state.check_signature } }

/// How often the WLAN page scans while it shows.
const SCAN_EVERY: Duration = Duration::from_secs(10);
WLAN 页面显示时扫描频率。cont SCAN_EVERY：持续时间 = 持续时间：：from_secs（10）;

fn wlan_worker(w: &Option<wlan::Worker>) -> Result<&wlan::Worker> {
    w.as_ref().context("the phone's WLAN is not brought up")
}
fn wlan_worker（w： &Option<wlan：：Worker>） -> Result<&wlan：：Worker> { w.as_ref（）.context（“手机的 WLAN 未被调出”）}

/// The installed system, as the update server is told of it.
fn installed(cfg: &DeviceConfig, info: &skyward_info::DeviceInfo) -> Result<Installed> {
    let slot = info.slot.context("only an A/B phone is updated")?;
    let props = skyward_info::installed_props(slot)?;
    let prop = |name: &str| props.get(name).cloned().with_context(|| format!("the installed system says no {name}"));
    Ok(Installed {
        device: prop("ro.product.mod_device")?,
        version: prop(&cfg.device.os_version_prop)?,
        android: prop("ro.build.version.release")?,
        unlocked: skyward_block::boot_param("androidboot.flash.locked")?.as_deref() == Some("0"),
    })
}
安装的系统，正如更新服务器被告知的那样。fn install（cfg： &DeviceConfig， info： &skyward_info：:D eviceInfo） -> 结果 <Installed> { let slot = info.slot.context（“只有 A/B 手机被更新”）？; let props = skyward_info：：installed_props（slot）？; let prop = |name： &str| props.get（name）.cloneed（）.with_context（|| format！（”已安装的系统显示没有 {name}“））;Ok（Installed（Installed { device： prop（“ro.product.mod_device”）？， version： prop（&cfg.device.os_version_prop）？， android： prop（“ro.build.version.release”）？， unlocked： skyward_block：：boot_param（“androidboot.flash.locked”）？.as_deref（） == Some（“0”）， }） }

/// Where `release`'s full package is downloaded to.
fn downloaded(release: &Release) -> PathBuf {
    Path::new(skyward_crypto::data::SDCARD).join(DOWNLOADS).join(&release.name)
}
“release”的完整包下载到那里。fn downloaded（release： &Release） -> PathBuf { Path：：new（skyward_crypto：:d ata：：SDCARD）.join（DOWNLOADS）.join（&release.name） }

/// What internal storage holds of `release`'s full package; nothing while it
/// cannot be read.
fn have(app: &App, release: &Release) -> Have {
    if !app.state.data.storage_open() {
        return Have::Nothing;
    }
    let path = downloaded(release);
    let size = |p: &Path| match fs::metadata(p) {
        Ok(m) => Some(m.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("read {}: {e}", p.display()));
            None
        }
    };
    match (size(&path), size(&download::partial(&path))) {
        (Some(whole), _) => Have::Whole(whole),
        (None, Some(part)) => Have::Part(part),
        (None, None) => Have::Nothing,
    }
}
“release”的完整包装内部存储了多少;只要 /// 无法读取，什么都不行。fn have（app： &App， release： &Release） -> Have { if ！app.state.data.storage_open（） { return Have：：Nothing; } let path = downloaded（release）; let size = |p： &Path| match fs：：metadata（p） { Ok（m） => Some（m.len（））， Err（e） if e.kind（） == std：：io：：ErrorKind：：NotFound => None， Err（e） => { let _ = skyward_sys：：kmsg（&format！（”读 {}： {e}“， p.display（））;无 } };match （size（&path）， size（&download：:p artial（&path）））） { （Some（whole）， _） => Have：：Whole（整体）， （None， Some（part）） => Have：:P art（part）， （None， None） => Have：：nothing， } }

/// Wait until a device, the installer or the recovery socket has something,
/// or `timeout_ms` passes.
fn wait(fds: &[i32], timeout_ms: i32) -> Result<()> {
    let mut polls: Vec<libc::pollfd> = fds.iter().map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 }).collect();
    let n = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, timeout_ms) };
    if n < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err).context("poll input devices");
        }
    }
    Ok(())
}
等设备、安装人员或恢复插座有问题时，/// 或“timeout_ms”通过。fn wait（fds： &[i32]， timeout_ms： i32） -> Result<（）> { let mut polls： Vec<libc：:p ollfd> = fds.iter（）.map（|&fd| libc：:p ollfd { fd， events： libc：:P OLLIN， revents： 0 }）.collect（）; 令 n = unsafe { libc：:p oll（polls.as_mut_ptr）， polls.len（） 作为 libc：：nfds_t， timeout_ms） }; if n < 0 { let err = std：：io：：Error：：last_os_error（）; if err.kind（） ！; if err.kind（） ！; = std：：io：：ErrorKind：：Interrupted { 返回 Err（err）.context（“poll input devices”）; } }Ok（（）） }

/// Run the pages. What `request`, read from misc, asks for decides the
/// first of them; asking for nothing, they start at `start`.
pub fn run(cfg: &DeviceConfig, font: &Path, mono: &Path, request: bcb::Request, start: Screen, dump: Option<&Path>) -> Result<()> {
    if cfg.display.density == 0 {
        return Err(anyhow!("{}: [display] density is required to lay out the UI", cfg.device.codename));
    }
    let mut info = skyward_info::collect(cfg)?;
    let fonts = Fonts::load(font, 0, mono)?;
    let mut surface = KmsSurface::open("/dev/dri/card0")?;
    let (w, h) = surface.size();
    let mut inputs = Inputs::open(w, h, cfg.touch.writes.clone())?;
    let started = Instant::now();
    let mut touch_known = false;
    // adbd and fastbootd ask over it to go over to fastboot and back.
    let control = Control::open()?;
    // Neither the motor nor the backlight is what shows the pages: a phone
    // whose motor driver is not loaded, or whose panel backlight is not
    // reached where the phone's own driver keeps it, still works, and why
    // one is not used goes to the kernel log.
    let mut haptics = match cfg.haptics.as_ref().map(Haptics::open).transpose() {
        Ok(haptics) => haptics,
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("the motor is not used: {e:#}"));
            None
        }
    };
    let mut backlight = match Backlight::open() {
        Ok(backlight) => Some(backlight),
        Err(e) => {
            let _ = skyward_sys::kmsg(&format!("the panel's backlight is not reached: {e:#}"));
            None
        }
    };
    let first = match &request.work {
        Work::Fastboot => Screen::Fastboot,
        Work::PromptWipeData => Screen::CantStart,
        Work::Unsupported(_) => Screen::Unhandled,
        Work::Menu
        | Work::Exit
        | Work::Update(_)
        | Work::WipeData { .. }
        | Work::WipeCache { .. }
        | Work::WipeAb { .. }
        | Work::Sideload { .. }
        | Work::Rescue
        | Work::UnshareBlocks => start,
    };
    let mut app = App::new(first, w, h, cfg.display.density as f32 / 160.0);
    app.set_screen_corners(cfg.display.corner_radius as f32);
    // The pages speak the language the system named with the work it asks
    // for, or else the one the installed system is built to start in.
    if let Some(locale) = request.locale.as_deref().or(info.locale.as_deref()) {
        app.state.lang = Lang::for_locale(locale)?;
        skyward_sys::kmsg(&format!("the pages speak {:?}, for the locale {locale}", app.state.lang))?;
    }
    // The home page tells of a restart into the slot the system does not
    // run from now: an update or a switch left for the next boot.
    if info.slot.is_some() {
        app.set_boot_slots(read_slots());
    }
    app.set_asked(asked(&request));
    // Opening the data partition, or a user's own files in it, under way,
    // and the users whose own files are still closed once it is open.
    let mut decryptor: Option<Decryptor> = None;
    let mut closed: Vec<Closed> = Vec::new();
    let mut arriving = Some(Incoming::Arrived);
    // The mount page shows the partitions as they are when it opens and
    // after each change; why they cannot be read is the page's to say.
    let mounts = || read_mounts(&cfg.services).map_err(|e| format!("{e:#}"));
刷页。从 misc 读来，“请求”的请求先决定了///;他们什么都不要求，从“开始”开始。pub fn run（cfg： &DeviceConfig， font： &Path， mono： &Path， request： bcb：：Request， start： Screen， dump： Option<&Path>） -> Result<（）> { if cfg.display.density == 0 { return Err（anyhow！（”{}： [display] 密度是对界面布局的要求“，cfg.device.codename））;} 让 mut info = skyward_info：：collect（cfg）？;let fonts = Fonts：：load（font， 0， mono）？;let mut surface = KmsSurface：：open（“/dev/dri/card0”）？;设 （w， h） = surface.size（）;令 mut inputs = Inputs：：open（w， h， cfg.touch.writes.clone（）？;let started = Instant：：now（）;设 mut touch_known = 假;adbd 和 fastbootd 会通过它请求通过 fastboot 再返回。让 control = Control：：open（）？;无论是电机还是背光都不是显示页面的关键：一部手机 // 电机驱动未加载，或者面板背光未被手机驱动放置的方向，仍然能工作，且为何 // 未使用其中一个，则会被提交内核日志。let mut haptics = match  cfg.haptics.as_ref（）.map（Haptics：：open）.transpose（） { Ok（haptics） => haptics s， Err（e） => { let _ = skyward_sys：：kmsg（&format！（”电机未使用：{e：#}“））;无 } };let mut backlight = match Backlight：：open（） { Ok（backlight） => Some（backlight）， Err（e） => { let _ = skyward_sys：：kmsg（&format！（”面板背光未被触及：{e：#}“））;无 } };let first = 匹配 &request.work { Work：：Fastboot => 屏幕：：Fastboot， Work：:P romptWipeData => 屏幕：：无法启动，Work：：Unsupported（_） => Screen：：Unhandled， Work：：Menu |工作：：出口 |作品：：更新（_） |工作：：清除数据 { .. } |工作：：WipeCache { .. } |工作：：WipeAb { .. } |工作：：侧载 { .. } |工作：：救援 |工作：：UnshareBlocks => start， };令 mut app = App：：new（first， w， h， cfg.display.density as f32 / 160.0）;app.set_screen_corners（cfg.display.corner_radius 为 f32）;这些页面会说系统命名的语言，并附带它要求的工作 //，或者是安装系统构建时用来启动的语言。如果让 Some（locale） = request.locale.as_deref（）.or（info.locale.as_deref（）） { app.state.lang = Lang：：for_locale（locale）？; skyward_sys：：kmsg（&format！（”页面表示{：？}，表示 locale {locale}“，app.state.lang））？;} // 主页会显示重启到系统不运行的插槽 // 从现在开始：更新或为下一次启动留下的开关。如果 info.slot.is_some（） { app.set_boot_slots（read_slots（））; } app.set_asked（asked（&request））;正在打开数据分区，或用户自己的文件，// 以及那些文件仍然关闭的用户。令 mut 解密器：Option<Decryptor> = 无;令 mut 闭合：Vec<Closed> = Vec：：new（）;令 mut artriving = Some（Incoming：：Arrived）;// 挂载页显示分区开启时的状态，// 每次更改后;为何无法读取，由页面说明。令挂载 = || read_mounts（&cfg.services）.map_err（|e| 格式！（“{e：#}”））;

    let mut screen = Canvas::new(w, h);
    // A page sliding over another draws on three screens' worth of canvas
    // besides the panel's: made ready now, the first slide finds them so.
    Canvas::keep_ready(w, h, 3);
    let mut shown_clock = String::new();
    // The readings are taken again on a thread of their own; those taken
    // here as work ends say how it left the phone, and readings started
    // before them are out of date.
    let readings = Readings::start(cfg)?;
    let mut read_at = Instant::now();
    let mut installer: Option<Installer> = None;
    // Whether the panel's frame memory has been blacked out for an off
    // panel; cleared once a real frame is shown again.
    let mut blanked = false;
    // Started for fastboot, the port goes to the phone's own fastbootd. The
    // page says whether it came up and how far a computer has the port;
    // why it did not come up goes to the kernel log and on screen.
    let mut fastboot = first == Screen::Fastboot;
    let mut port_at = Instant::now();
    // File transfer runs while its switch is on, and offers internal
    // storage while that is open; one that does not start says why on
    // screen, and the recovery carries on.
    let mut transfer: Option<Transfer> = None;
    if app.state.file_transfer {
        match Transfer::start(cfg, &info, app.state.time_zone) {
            Ok(t) => transfer = Some(t),
            Err(e) => {
                skyward_sys::kmsg(&format!("file transfer did not start: {e:#}"))?;
                app.file_transfer_failed(false, &skyward_ui::fmt::transfer_failed(app.state.lang, &format!("{e:#}")), monotonic_ms());
            }
        }
    }
    // A sideload has the port, which goes back to rest once it ends.
    let mut sideloading = false;
    if fastboot {
        if let Err(e) = skyward_sys::usb::switch("fastboot", USB_TIMEOUT) {
            skyward_sys::kmsg(&format!("fastbootd has not taken the port: {e:#}"))?;
            app.report(&format!("{e:#}"), monotonic_ms());
        }
        let (running, link) = fastboot_status()?;
        app.set_fastboot(running, link);
    }
    // Work the request asks for that goes on keeps the request in misc,
    // so a restart halfway comes back to it, until it is done or the user
    // leaves it for somewhere else.
    let misc = bcb::misc();
    let mut kept = request.work.lasts();
    // The wipe page offers the cache partition where the phone's table has
    // one; a table that does not read leaves it out, and the page still
    // works.
    app.state.cache_partition = match wipe::cache() {
        Ok(cache) => cache.is_some(),
        Err(e) => {
            skyward_sys::kmsg(&format!("whether the phone has a cache partition is not known: {e:#}"))?;
            false
        }
    };
    app.state.phone_logs = cfg.wipe.logs.len();
    // Skyward lives in a recovery partition of each slot where the phone
    // has them, and the install page then offers to keep it there.
    let by_name = Path::new(skyward_block::BY_NAME);
    app.state.recovery_partitions = by_name.join("recovery_a").try_exists()? && by_name.join("recovery_b").try_exists()?;
    let mut eraser: Option<Eraser> = None;
    // Erasing the system asked for leaves on its own once it is done, as
    // does erasing an update asked for, sideloaded with a restart after it;
    // what the user chose waits for them.
    let mut erase_leaves = false;
    // Erasing all user data rescue asked for, whose end goes back to rescue
    // rather than to a page of its own.
    let mut rescue_erasing = false;
    // Where rescue asked the phone to leave for, once it lets USB go.
    let mut rescue_leaves: Option<(Target, bool)> = None;
    // The install the system asked for, until the data partition it is in
    // opens, or the user whose own files hold it unlocks them.
    let mut pending: Option<Pending> = None;
    // Work on whole partitions under way, and whether the system asked for
    // it before the recovery started.
    let mut part_worker: Option<partwork::Worker> = None;
    match &request.work {
        Work::WipeData { reformat } => {
            eraser = Some(factory_reset(&mut app, request.keep_memtag_mode, true, reformat.clone())?);
            erase_leaves = true;
        }
        Work::WipeCache { reformat } => {
            eraser = Some(parts_wipe(&mut app, vec![Step::Cache], &cfg.wipe.logs, true, reformat.clone())?);
            erase_leaves = true;
        }
        Work::Sideload { .. } => {
            app.start_sideload();
            installer = Some(Installer::start(detach(cfg.services.clone()), install_options(&app))?);
            sideloading = true;
        }
        Work::Rescue => {
            app.start_rescue();
            installer = Some(Installer::rescue(detach(cfg.services.clone()), install_options(&app))?);
            sideloading = true;
        }
        Work::Update(update) => {
            app.start_asked(update.security, update.package.clone());
            let retry = update.retry > 0;
            if !retry && panicked() {
                skyward_sys::kmsg("the phone came here after its kernel stopped, so the update asked for is not installed")?;
                app.install(skyward_install::Event::Finished(skyward_install::Outcome::Refused(skyward_install::Refusal::Panicked)), monotonic_ms());
                bcb::clear(&misc)?;
                kept = false;
            } else {
                let (package, needs) = asked::locate(&update.package, app.state.cache_partition);
                skyward_sys::kmsg(&format!("the update asked for is {package:?}, which needs {needs:?}"))?;
                let wanted = Pending { package, needs, retry };
                match needs {
                    Needs::Data | Needs::User(_) => {
                        app.asked_opening();
                        decryptor = Some(Decryptor::start(decrypt::Job::Open)?);
                        pending = Some(wanted);
                    }
                    Needs::Nothing | Needs::Cache => installer = Some(install_asked(cfg, &app, wanted)?),
                }
            }
        }
        Work::WipeAb { package_size } => {
            app.part_work_started(PartKind::Disable, true);
            part_worker = Some(partwork::Worker::start(partwork::Task::Disable { package_size: *package_size })?);
        }
        Work::UnshareBlocks => {
            let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
            app.part_work_started(PartKind::Unshare, true);
            part_worker = Some(partwork::Worker::start(partwork::Task::Unshare { slot })?);
        }
        Work::Menu | Work::Exit | Work::Fastboot | Work::PromptWipeData | Work::Unsupported(_) => {}
    }
    // The port came up with adb alone; at rest it offers file transfer
    // beside it.
    if transfer.is_some() && !fastboot && !sideloading {
        if let Err(e) = skyward_sys::usb::switch(skyward_sys::usb::resting(), USB_TIMEOUT) {
            skyward_sys::kmsg(&format!("the port did not take file transfer: {e:#}"))?;
            app.report(&format!("{e:#}"), monotonic_ms());
        }
    }
    // Once the work the system asked for is done, the page shows it for a
    // moment; then the phone starts the system, or powers off where the
    // request says to.
    let leave = if request.shutdown_after { Target::PowerOff } else { Target::System };
    let mut leave_at: Option<u64> = None;
    // The terminal page's shell, from when the page first asks for one.
    let mut shell: Option<pty::Shell> = None;
    // Copying, moving or deleting under way.
    let mut worker: Option<filework::Worker> = None;
    // Giving the data partition's files their SELinux labels under way.
    let mut relabeler: Option<filework::Worker> = None;
    // Backing up or restoring under way.
    let mut backuper: Option<backup::Worker> = None;
    // An update's merge followed: one the recovery found under way, which
    // the kernel took up again as the snapshots were mapped, or one started
    // on the update page.
    let mut merger: Option<merge::Merger> = None;
    if let Some(slot) = info.slot {
        let standing = skyward_vab::standing(slot)?;
        if matches!(standing, Standing::Merging(_)) && skyward_vab::snapshots_mapped(slot)? {
            skyward_sys::kmsg("the update's merge goes on")?;
            merger = Some(merge::Merger::start(slot, false)?);
        }
        app.set_update(standing);
    }
    // The DNS servers the network joined gave, for looking names up.
    let servers: Servers = Arc::new(Mutex::new(Vec::new()));
    // The WLAN, on a phone whose driver the recovery brings up; it starts
    // off. Without the bundle of the vendor interface's libraries the
    // supplicant does not start, and the WLAN page says why.
    let wlan = match &cfg.wlan {
        Some(w) => {
            app.state.wlan = Some(Wlan::default());
            let modules = cfg.modules.sources.iter().map(|s| skyward_modules::source_dir(s)).collect::<Result<Vec<_>>>()?;
            let libraries = secure::libraries().unwrap_or_else(|e| {
                let _ = skyward_sys::kmsg(&format!("the vendor interface's libraries are not open: {e:#}"));
                None
            });
            Some(wlan::Worker::start(w.clone(), modules, libraries, servers.clone())?)
        }
        None => None,
    };
    let mut scanned_at = Instant::now();
    app.state.update_server = cfg.update.is_some();
    // Asking the update server, and downloading the full package.
    let mut checker: Option<updates::Worker> = None;
    let mut downloader: Option<updates::Worker> = None;
    // The USB drive plugged in, looked for each second.
    let mut plugged: Option<Plugged> = None;
    let mut seen: Option<String> = None;
    let mut drive_at = Instant::now();
    loop {
        let now = monotonic_ms();
        if leave_at.is_some_and(|at| now >= at) {
            leave_at = None;
            power(leave)?;
        }
        if !touch_known && started.elapsed() >= TOUCH_WAIT {
            touch_known = true;
            if !inputs.has_panel() {
                skyward_sys::kmsg("no touch panel came up; the pages are worked with the keys")?;
                app.touchscreen_missing();
            }
        }
        for fx in app.tick(now) {
            match fx {
                Effect::Haptic => {
                    if let Some(h) = &mut haptics {
                        h.click(app.state.vibration)?;
                    }
                }
                other => unreachable!("time passing asks for nothing but a tap of the motor, not {other:?}"),
            }
        }
        let time = clock(app.state.time_zone)?;
        if time != shown_clock {
            shown_clock = time;
            app.redraw();
        }
        for reading in readings.taken() {
            match reading {
                Reading::Battery(battery) => {
                    let battery = battery?;
                    if battery != info.battery {
                        info.battery = battery;
                        app.refresh();
                    }
                }
                Reading::All(all, at) => {
                    let all = (*all)?;
                    if at >= read_at && all != info {
                        info = all;
                        app.refresh();
                    }
                }
            }
        }
        // In fastboot the page follows fastbootd and the cable each second.
        if fastboot && port_at.elapsed().as_secs() >= 1 {
            let (running, link) = fastboot_status()?;
            app.set_fastboot(running, link);
            port_at = Instant::now();
        }
        let panel = app.panel(now);
        let mut shown = false;
        if panel.is_some() {
            if app.take_dirty() {
                let t0 = Instant::now();
                app.compose(&mut screen, &fonts, &info, &shown_clock, now);
                surface.present(&screen)?;
                eprintln!("frame {:?} in {} ms", app.screen(), t0.elapsed().as_millis());
                if let Some(path) = dump {
                    fs::write(path, &screen.data).with_context(|| format!("write {}", path.display()))?;
                }
                shown = true;
            }
            blanked = false;
        } else if !blanked {
            // Black out the panel's frame memory before the backlight goes
            // to 0, so an OLED that floors the backlight above zero rather
            // than cutting it shows nothing and cannot burn in.
            screen.data.fill(0);
            surface.present(&screen)?;
            surface.settle()?;
            blanked = true;
        }
        // After the frame is on the panel, so a panel coming back on lights
        // with what it should show now, never with what it showed when it
        // went off.
        if backlight.as_ref().is_some_and(|b| b.level == 0) && panel.is_some() {
            surface.settle()?;
        }
        // The pages are what this loop is for; a panel whose backlight
        // cannot be written still shows them, and why it could not goes to
        // the kernel log once rather than ending the recovery.
        let lit = match backlight.as_mut() {
            Some(b) => b.show(panel).map_err(|e| format!("{e:#}")),
            None => Ok(()),
        };
        if let Err(e) = lit {
            let _ = skyward_sys::kmsg(&format!("the panel's backlight is not set: {e}"));
            backlight = None;
        }
        // A page at rest draws the rows it has yet to while nothing else
        // is to be done, a band at a time, looking for input between bands.
        let ahead = panel.is_some() && !shown && app.draw_ahead(&fonts, &info);
        // A frame shown has waited for the one before it to reach the
        // panel, which keeps the loop to the panel's pace; otherwise the CPU
        // gets at least a millisecond.
        let floor = if shown { 0 } else { 1 };
        let until_due = app.deadline(now).map(|d| d.saturating_sub(now) as i32);
        let timeout = if ahead { 0 } else { until_due.map_or(1000, |t| t.clamp(floor, 1000)) };
        let mut fds = inputs.fds();
        if let Some(i) = &installer {
            fds.push(i.fd());
        }
        if let Some(c) = &control {
            fds.push(c.fd());
        }
        if let Some(e) = &eraser {
            fds.push(e.fd());
        }
        if let Some(sh) = &shell {
            fds.extend(sh.fds());
        }
        if let Some(w) = &worker {
            fds.push(w.fd());
        }
        if let Some(r) = &relabeler {
            fds.push(r.fd());
        }
        if let Some(b) = &backuper {
            fds.push(b.fd());
        }
        if let Some(m) = &merger {
            fds.push(m.fd());
        }
        if let Some(d) = &decryptor {
            fds.push(d.fd());
        }
        if let Some(p) = &part_worker {
            fds.push(p.fd());
        }
        if let Some(w) = &wlan {
            fds.push(w.fd());
        }
        if let Some(c) = &checker {
            fds.push(c.fd());
        }
        if let Some(d) = &downloader {
            fds.push(d.fd());
        }
        fds.push(readings.fd());
        wait(&fds, timeout)?;
        let now = monotonic_ms();
        let mut drive_told = None;
        if drive_at.elapsed().as_secs() >= 1 {
            drive_told = look_for_drive(&mut plugged, &mut seen)?;
            drive_at = Instant::now();
        }
        // What the installer and the eraser told comes first, so a touch
        // acts on the page as it now stands.
        {
            let told = installer.as_ref().map(Installer::events).unwrap_or_default();
            for ev in told {
                use skyward_install::{Event as Told, Outcome};
                let finished = matches!(ev, Told::Finished(_));
                let installed = matches!(ev, Told::Finished(Outcome::Installed { .. } | Outcome::Ran { .. }));
                let wipe_after = matches!(ev, Told::Finished(Outcome::Installed { wipe: true, .. }));
                let script_cache = matches!(ev, Told::Finished(Outcome::Ran { wipe_cache: true, .. }));
                eprintln!("install {ev:?}");
                match &ev {
                    // Rescue's own erasing tells rescue how it went once it
                    // ends; file transfer that will not let internal
                    // storage go keeps it from starting.
                    Told::Wipe => {
                        ensure!(eraser.is_none(), "erasing is under way already");
                        if withhold(&mut transfer, &mut app, now)? {
                            eraser = Some(rescue_wipe(&mut app)?);
                            rescue_erasing = true;
                        } else {
                            installer.as_ref().context("rescue asked for erasing")?.wiped(false);
                        }
                    }
                    Told::Reboot(target) => rescue_leaves = Some(rescue_target(target)),
                    _ => {}
                }
                app.install(ev, now);
                if finished {
                    installer = None;
                    sideloading = false;
                    // A sideload or an update the system asked for is done
                    // with however it went, and so is rescue, once its page
                    // is left or the computer asks the phone to leave. A
                    // sideload asked to leaves on its own after it, and an
                    // update that went in does, once what the package or the
                    // request asked to be erased is.
                    if matches!(request.work, Work::Sideload { .. } | Work::Update(_) | Work::Rescue) && kept {
                        bcb::clear(&misc)?;
                        kept = false;
                    }
                    let (leaves, cache_after) = match &request.work {
                        Work::Sideload { auto_reboot } => (*auto_reboot, script_cache),
                        Work::Update(update) => (installed, script_cache || (installed && update.wipe_cache)),
                        _ => (false, script_cache),
                    };
                    if cache_after && !app.state.cache_partition {
                        skyward_sys::kmsg("the install asked for the cache partition to be erased, and the phone has none")?;
                    }
                    if wipe_after {
                        ensure!(eraser.is_none(), "erasing is under way already");
                        eraser = Some(factory_reset(&mut app, false, false, None)?);
                        erase_leaves = leaves;
                    } else if cache_after && app.state.cache_partition {
                        ensure!(eraser.is_none(), "erasing is under way already");
                        eraser = Some(parts_wipe(&mut app, vec![Step::Cache], &cfg.wipe.logs, false, None)?);
                        erase_leaves = leaves;
                    } else if leaves {
                        leave_at = Some(now + LEAVE_AFTER_MS);
                    }
                    // Rescue asked to come back is told so in misc, as
                    // Android's own recovery tells it.
                    if let Some((target, again)) = rescue_leaves.take() {
                        if again {
                            bcb::write(&misc, &bcb::Bcb { command: "boot-rescue".into(), args: Vec::new() })?;
                        }
                        power(target)?;
                    }
                }
            }
        }
        let mut erased = false;
        if let Some(e) = &eraser {
            for ev in e.events() {
                match ev {
                    // Rescue's page says only that erasing is under way.
                    wipe::Event::At(_) | wipe::Event::Progress(..) if rescue_erasing => {}
                    wipe::Event::At(step) => app.erase_at(part(step)),
                    wipe::Event::Progress(done, total) => app.erase_progress(done, total),
                    // Rescue stays on: its request stays in misc, and the
                    // computer hears how erasing went.
                    wipe::Event::Ended(outcome) if rescue_erasing => {
                        let said = outcome.as_ref().map_or_else(wipe::said, |_| "done".to_string());
                        skyward_sys::kmsg(&format!("erasing for rescue ended: {said}"))?;
                        installer.as_ref().context("rescue waits for erasing to end")?.wiped(outcome.is_ok());
                        app.rescue_wiped(outcome.map_err(erase_stop), now);
                        if app.state.data == Data::Erased {
                            closed.clear();
                        }
                        rescue_erasing = false;
                        erased = true;
                    }
                    wipe::Event::Opened(opening) => {
                        if let Some(mounted) = &opening.mounted {
                            log_mounted(mounted)?;
                        }
                        closed = opening.closed;
                        show_closed(&mut app, &closed)?;
                    }
                    wipe::Event::Ended(outcome) => {
                        let said = outcome.as_ref().map_or_else(wipe::said, |_| "done".to_string());
                        skyward_sys::kmsg(&format!("erasing ended: {said}"))?;
                        // As with the phone's own recovery, the request is
                        // done with either way, so a restart starts the
                        // system rather than erasing again. A factory
                        // reset done is what an update that asked for one
                        // asked, too.
                        if kept {
                            bcb::clear(&misc)?;
                            kept = false;
                        }
                        if outcome.is_ok() && app.state.erase.reset {
                            bcb::clear(&misc)?;
                        }
                        if outcome.is_ok() && erase_leaves {
                            leave_at = Some(now + LEAVE_AFTER_MS);
                        }
                        app.erase_ended(outcome.map_err(erase_stop), now);
                        // Users of a data partition erased whole have no
                        // files left to open.
                        if app.state.data == Data::Erased {
                            closed.clear();
                        }
                        erased = true;
                    }
                }
            }
        }
        if erased {
            eraser = None;
        }
        let job_end = |end: filework::End| match end {
            filework::End::Done => JobEnd::Done,
            filework::End::Stopped => JobEnd::Stopped,
            filework::End::Failed(why) => JobEnd::Failed(why),
        };
        let mut worked = false;
        if let Some(w) = &worker {
            for ev in w.events() {
                match ev {
                    filework::Event::Progress(p) => app.file_progress((p.bytes, p.total_bytes), (p.entries, p.total_entries)),
                    filework::Event::Relabeling(_) => unreachable!("work on files gives no labels"),
                    filework::Event::Ended(end) => {
                        app.file_job_ended(job_end(end), now);
                        worked = true;
                    }
                }
            }
        }
        let mut relabeled = false;
        if let Some(r) = &relabeler {
            for ev in r.events() {
                match ev {
                    filework::Event::Relabeling(p) => app.relabel_progress(p.checked, p.total, p.changed),
                    filework::Event::Progress(_) => unreachable!("relabeling moves no bytes"),
                    filework::Event::Ended(end) => {
                        app.relabel_ended(job_end(end), now);
                        relabeled = true;
                    }
                }
            }
        }
        if relabeled {
            relabeler = None;
        }
        let mut parted = false;
        if let Some(p) = &part_worker {
            for ev in p.events() {
                match ev {
                    partwork::Event::Listed(names) => app.part_listed(names),
                    partwork::Event::At(index) => app.part_at(index),
                    partwork::Event::Ended(index, item, said) => {
                        skyward_sys::kmsg(&format!("partition work: item {index} ended as {item:?}"))?;
                        app.part_ended(index, item_end(item), &said);
                    }
                    partwork::Event::Done(done) => {
                        skyward_sys::kmsg(&format!("partition work ended: {done:?}"))?;
                        let work = app.state.part_work.as_ref().context("partition work shows on its page")?;
                        let whole = done.is_ok() && !work.items.iter().any(|(_, s)| matches!(s, ItemState::Ended(ItemEnd::Failed(_))));
                        let data = work.items.iter().any(|(name, _)| name == DATA_PARTITION);
                        // What the system asked for is done with either way,
                        // and the phone leaves on its own once it went
                        // through every partition.
                        if work.asked {
                            if kept {
                                bcb::clear(&misc)?;
                                kept = false;
                            }
                            if whole {
                                leave_at = Some(now + LEAVE_AFTER_MS);
                            }
                        }
                        // Work on the data partition leaves it closed.
                        if data {
                            closed.clear();
                        }
                        app.part_work_ended(done.map_err(part_stop), now);
                        parted = true;
                    }
                }
            }
        }
        if parted {
            part_worker = None;
        }
        // The folder shown is read again for what the work changed.
        if worked {
            worker = None;
            let folder = app.state.files.path.clone();
            list(&mut app, folder, &closed)?;
        }
        let mut backed = false;
        if let Some(b) = &backuper {
            for ev in b.events() {
                match ev {
                    backup::Event::Plan(plan) => app.backup_plan(plan),
                    backup::Event::At { item, done, of, checking } => app.backup_at(BackupAt { item, done, of, checking }),
                    backup::Event::Ended(end) => {
                        app.backup_ended(backup_end(end), now);
                        backed = true;
                    }
                }
            }
        }
        if backed {
            backuper = None;
        }
        if let Some(m) = &merger {
            let slot = info.slot.context("only an A/B phone merges an update")?;
            for ev in m.events() {
                match ev {
                    merge::Event::Looked(look) => {
                        eprintln!("merge {look:?}");
                        app.set_update(match look {
                            Merge::Going(done) => Standing::Merging(done),
                            Merge::Done | Merge::NeedsReboot => Standing::Merged,
                            Merge::Failed(code) => Standing::Failed(code),
                            Merge::Cancelled | Merge::Idle(_) => skyward_vab::standing(slot)?,
                        });
                    }
                    merge::Event::Failed(why) => {
                        skyward_sys::kmsg(&format!("the update did not merge: {why}"))?;
                        app.report(&skyward_ui::fmt::merge_failed(app.state.lang, &why), now);
                        app.set_update(skyward_vab::standing(slot)?);
                    }
                }
            }
            // Merged or not, the phone's readings say where the update
            // stands now.
            if m.ended() {
                merger = None;
                read_at = Instant::now();
                info = skyward_info::collect(cfg)?;
                app.refresh();
            }
        }
        if let Some((told, traffic)) = wlan.as_ref().map(wlan::Worker::told) {
            if let Some(w) = told {
                app.set_wlan(w);
            }
            if let Some(t) = traffic {
                app.set_traffic(t);
            }
        }
        // The WLAN page scans again now and then while it shows, as the
        // stock settings do.
        let wlan_on = app.state.wlan.as_ref().is_some_and(|w| w.power == WlanPower::On);
        if app.screen() == Screen::Wlan && wlan_on && scanned_at.elapsed() >= SCAN_EVERY {
            scanned_at = Instant::now();
            wlan_worker(&wlan)?.send(wlan::Command::Scan);
        }
        let mut checked = false;
        if let Some(c) = &checker {
            for ev in c.events() {
                let updates::Event::Checked(answer) = ev else { unreachable!("asking the server downloads nothing") };
                let have = match &answer {
                    Ok(Some(release)) => have(&app, release),
                    _ => Have::Nothing,
                };
                app.update_checked(answer, have);
                checked = true;
            }
        }
        if checked {
            checker = None;
        }
        let mut fetched = false;
        if let Some(d) = &downloader {
            for ev in d.events() {
                match ev {
                    updates::Event::Progress(progress, rate) => app.download_progress(progress, rate),
                    updates::Event::Ended(end) => {
                        let release = app.state.newest().context("a download is of the newest system")?.clone();
                        let end = match end {
                            updates::End::Downloaded => DownloadEnd::Downloaded,
                            updates::End::Stopped => DownloadEnd::Stopped,
                            updates::End::Failed(why) => DownloadEnd::Failed(why),
                        };
                        skyward_sys::kmsg(&format!("downloading {} ended: {end:?}", release.name))?;
                        app.download_ended(end, have(&app, &release), now);
                        fetched = true;
                    }
                    updates::Event::Checked(_) => unreachable!("a download asks nothing of its own"),
                }
            }
        }
        if fetched {
            downloader = None;
        }
        if let Some(ended) = decryptor.as_ref().and_then(Decryptor::ended) {
            decryptor = None;
            match ended {
                decrypt::Ended::Opened(Ok(opening)) => {
                    if let Some(mounted) = &opening.mounted {
                        log_mounted(mounted)?;
                    }
                    closed = opening.closed;
                    show_closed(&mut app, &closed)?;
                }
                decrypt::Ended::Opened(Err(why)) => {
                    skyward_sys::kmsg(&format!("the data partition stays closed: {why}"))?;
                    // The update the system asked for cannot be read, and
                    // is done with.
                    if pending.take().is_some() {
                        let failed = skyward_install::Outcome::Failed(skyward_install::Failure::Unreadable(why.clone()));
                        app.install(skyward_install::Event::Finished(failed), now);
                        if kept {
                            bcb::clear(&misc)?;
                            kept = false;
                        }
                    }
                    app.data_failed(why);
                }
                decrypt::Ended::Unlocked(Ok(still)) => {
                    closed = still;
                    show_closed(&mut app, &closed)?;
                }
                decrypt::Ended::Unlocked(Err(why)) => {
                    // Which user the credential was for, so that a page
                    // holding the recovery at a second space's prompt is not
                    // read as the owner's own credential being wrong.
                    let who = match app.state.data {
                        Data::Locked { user, .. } => format!("the files of user {user}"),
                        _ => "the user's own files".to_string(),
                    };
                    skyward_sys::kmsg(&format!("what was entered did not open {who}: {why}"))?;
                    app.unlock_failed(why, now);
                }
            }
            // Screenshots taken while internal storage was closed go into
            // the gallery's once it opens.
            if app.state.place_open(Place::Storage) {
                if let Err(e) = keep_held_screenshots() {
                    let why = format!("{e:#}");
                    skyward_sys::kmsg(&format!("the screenshots held did not go into the gallery's: {why}"))?;
                    app.screenshots_not_kept(&why, now);
                }
            }
            // The update the system asked for starts once what holds it is
            // open; until the user whose own files hold it unlocks them,
            // its page says to.
            if let Some(wanted) = pending.take() {
                match wanted.needs {
                    Needs::User(user) if closed.iter().any(|c| c.user == user) => {
                        app.asked_locked(user, now);
                        pending = Some(wanted);
                    }
                    _ => installer = Some(install_asked(cfg, &app, wanted)?),
                }
            }
        }
        if let Some(sh) = &mut shell {
            let wrote = sh.read()?;
            if !wrote.is_empty() {
                app.term_output(&wrote);
            }
            if let Some(status) = sh.ended()? {
                // What it wrote last shows before its end does.
                let last = sh.read()?;
                if !last.is_empty() {
                    app.term_output(&last);
                }
                app.term_ended(pty::how(status));
                shell = None;
            }
        }
        // Then what programs asked on the recovery socket, each answered as
        // the recovery stands when its turn comes.
        let requests = match &control {
            Some(c) => c.requests()?,
            None => Vec::new(),
        };
        let incoming = arriving
            .take()
            .into_iter()
            .chain(drive_told.map(Incoming::Drive))
            .chain(requests.into_iter().map(Incoming::Asked))
            .chain(inputs.read()?.into_iter().map(Incoming::Input));
        for came in incoming {
            let effects = match came {
                Incoming::Arrived => app.arrive(),
                Incoming::Drive(drive) => app.set_usb_drive(drive),
                Incoming::Asked(asked) => {
                    skyward_sys::kmsg(&format!("asked on the recovery socket: {asked:?}"))?;
                    answer(asked, fastboot)
                }
                Incoming::Input(Event::Touch(t, at)) => app.touch(t, &fonts, at),
                Incoming::Input(Event::Key(k, true)) => app.key(k, now),
                Incoming::Input(Event::Key(k, false)) => app.key_up(k, now),
            };
            // What the pages ask for in answer to the work done goes on
            // after it.
            let mut queue = VecDeque::from(effects);
            while let Some(fx) = queue.pop_front() {
                eprintln!("effect {fx:?}");
                match fx {
                    // Leaving is the user's answer to whatever the request
                    // still kept in misc asked.
                    Effect::Reboot(target) => {
                        if kept {
                            bcb::clear(&misc)?;
                            kept = false;
                        }
                        power(target)?
                    }
                    Effect::Dismiss => {
                        if kept {
                            bcb::clear(&misc)?;
                            kept = false;
                        }
                    }
                    Effect::FactoryReset => {
                        ensure!(eraser.is_none(), "erasing is under way already");
                        if !withhold(&mut transfer, &mut app, now)? {
                            continue;
                        }
                        eraser = Some(factory_reset(&mut app, request.keep_memtag_mode, true, None)?);
                        erase_leaves = true;
                    }
                    // Opening the data partition and erasing it are not to
                    // cross; the page says to wait for the one under way.
                    Effect::Wipe(chosen) => {
                        ensure!(eraser.is_none(), "erasing is under way already");
                        if decryptor.is_some() {
                            app.report(app.state.lang.pick(DATA_OPENING), now);
                            continue;
                        }
                        if !withhold(&mut transfer, &mut app, now)? {
                            continue;
                        }
                        erase_leaves = false;
                        if chosen.contains(&Wipe::All) {
                            eraser = Some(factory_reset(&mut app, false, false, None)?);
                        } else if chosen.iter().any(|&w| w != Wipe::Cache) && app.state.data == Data::Erased {
                            app.erase_started(false, false, Vec::new());
                            app.erase_ended(Err(EraseStop::Erased), now);
                        } else {
                            let steps = chosen.into_iter().map(step).collect();
                            eraser = Some(parts_wipe(&mut app, steps, &cfg.wipe.logs, false, None)?);
                        }
                    }
                    Effect::Install => installer.as_ref().context("no installer holds an update")?.install(),
                    // A shell that will not start, or will not take keys,
                    // is the page's to report; the recovery carries on.
                    Effect::StartShell(folder) => {
                        ensure!(shell.is_none(), "a shell runs already");
                        let (rows, cols) = app.term_size(&fonts);
                        match pty::Shell::start(rows, cols, &cfg.device.codename, Path::new(&folder)) {
                            Ok(sh) => {
                                shell = Some(sh);
                                app.term_started(rows, cols);
                            }
                            Err(e) => {
                                skyward_sys::kmsg(&format!("no shell for the terminal: {e:#}"))?;
                                app.report(&format!("{e:#}"), now);
                            }
                        }
                    }
                    Effect::TermInput(bytes) => {
                        let sh = shell.as_mut().context("keys went to a shell that has ended")?;
                        if let Err(e) = sh.write(&bytes) {
                            app.report(&format!("{e:#}"), now);
                        }
                    }
                    Effect::ShellTo(folder) => {
                        let sh = shell.as_mut().context("a folder went to a shell that has ended")?;
                        let went = match sh.waiting() {
                            Ok(true) => sh.change_to(&folder),
                            Ok(false) => {
                                app.report(app.state.lang.pick(skyward_ui::fmt::TERMINAL_BUSY), now);
                                Ok(())
                            }
                            Err(e) => Err(e),
                        };
                        if let Err(e) = went {
                            app.report(&format!("{e:#}"), now);
                        }
                    }
                    Effect::Haptic => {
                        if let Some(h) = &mut haptics {
                            h.click(app.state.vibration)?;
                        }
                    }
                    // The panel shows the last frame drawn, which is what is
                    // saved; one that cannot be is the page's to report.
                    Effect::Screenshot { storage } => {
                        let t0 = Instant::now();
                        let saved = save_screenshot(&screen, storage, app.state.time_zone).map_err(|e| format!("{e:#}"));
                        eprintln!("screenshot {saved:?} in {} ms", t0.elapsed().as_millis());
                        app.screenshot_saved(saved, now);
                    }
                    Effect::Sideload => {
                        ensure!(installer.is_none(), "an installer is running already");
                        if !withhold(&mut transfer, &mut app, now)? {
                            continue;
                        }
                        installer = Some(Installer::start(detach(cfg.services.clone()), install_options(&app))?);
                        sideloading = true;
                    }
                    Effect::InstallFile(place, path) => {
                        ensure!(installer.is_none(), "an installer is running already");
                        if !withhold(&mut transfer, &mut app, now)? {
                            continue;
                        }
                        installer = Some(Installer::open(in_place(place, &path), detach(cfg.services.clone()), install_options(&app))?);
                    }
                    Effect::InstallPath(path) => {
                        ensure!(installer.is_none(), "an installer is running already");
                        if !withhold(&mut transfer, &mut app, now)? {
                            continue;
                        }
                        installer = Some(Installer::open(PathBuf::from(path), detach(cfg.services.clone()), install_options(&app))?);
                    }
                    Effect::Flash { partition, pick } => {
                        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                        installer.as_ref().context("no installer holds an image")?.flash(partition, pick.slots(slot));
                    }
                    Effect::Cancel => match &installer {
                        Some(i) => i.cancel()?,
                        // The update the system asked for, waiting on the
                        // lock of the user whose files hold it, is given up
                        // as its page is left, and is done with.
                        None => {
                            ensure!(pending.take().is_some(), "no installer to cancel");
                            skyward_sys::kmsg("the update asked for is given up, its page left")?;
                            if kept {
                                bcb::clear(&misc)?;
                                kept = false;
                            }
                            let cancelled = skyward_install::Event::Finished(skyward_install::Outcome::Cancelled);
                            app.install(cancelled, now);
                        }
                    },
                    Effect::ReadMounts => app.set_mounts(mounts()),
                    Effect::ReadSlots => app.set_boot_slots(read_slots()),
                    Effect::SetSlot(slot) => app.slot_switched(set_slot(slot), now),
                    // The decrypt page shows meanwhile that the work is
                    // under way, and takes nothing more until it is done.
                    Effect::OpenData => {
                        ensure!(decryptor.is_none(), "the data partition is being opened already");
                        decryptor = Some(Decryptor::start(decrypt::Job::Open)?);
                    }
                    Effect::Unlock { user, code } => {
                        ensure!(decryptor.is_none(), "the data partition is being opened already");
                        let job = decrypt::Job::Unlock { closed: closed.clone(), user, credential: code.into_bytes() };
                        decryptor = Some(Decryptor::start(job)?);
                    }
                    Effect::ReadDir(folder) => list(&mut app, folder, &closed)?,
                    Effect::ReadPlace(place, folder) => list_place(&mut app, place, folder)?,
                    // A name taken where an entry goes is the user's to
                    // decide on before anything is written.
                    Effect::Paste { op, path, into, clash } => {
                        ensure!(worker.is_none(), "work on files is under way already");
                        let clash = match clash {
                            Some(choice) => clash_rule(choice),
                            None => match skyward_files::clashes(&[PathBuf::from(&path)], Path::new(&into)) {
                                Ok(taken) if taken.is_empty() => skyward_files::Clash::Refuse,
                                Ok(taken) => {
                                    let names = taken.iter().map(|t| t.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())).collect();
                                    app.file_clash(op, path, into, names);
                                    continue;
                                }
                                Err(e) => {
                                    app.report(&format!("{e:#}"), now);
                                    continue;
                                }
                            },
                        };
                        let ownership = match ownership(Path::new(&into)) {
                            Ok(ownership) => ownership,
                            Err(e) => {
                                app.report(&format!("{e:#}"), now);
                                continue;
                            }
                        };
                        let (from, to) = (PathBuf::from(&path), PathBuf::from(&into));
                        let (task, shown) = match op {
                            FileOp::Copy => (filework::Task::Copy { from, into: to, clash, ownership }, FileTask::Copy { path, into }),
                            FileOp::Move => (filework::Task::Move { from, into: to, clash, ownership }, FileTask::Move { path, into }),
                        };
                        worker = Some(filework::Worker::start(task)?);
                        app.file_job_started(shown);
                    }
                    Effect::Delete(path) => {
                        ensure!(worker.is_none(), "work on files is under way already");
                        worker = Some(filework::Worker::start(filework::Task::Delete { path: PathBuf::from(&path) })?);
                        app.file_job_started(FileTask::Delete { path });
                    }
                    Effect::StopJob => worker.as_ref().context("there is no work on files to stop")?.stop(),
                    // Rules kept on a partition that is not mounted are the
                    // page's to report, with what to do first.
                    Effect::Relabel => {
                        ensure!(relabeler.is_none(), "relabeling is under way already");
                        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                        match relabel::rule_files(slot) {
                            Ok(rules) => {
                                relabeler = Some(filework::Worker::start(filework::Task::Relabel { rules })?);
                                app.relabel_started();
                            }
                            Err(e) => match e.downcast_ref::<relabel::Unmounted>() {
                                Some(relabel::Unmounted(base)) => app.report(&skyward_ui::fmt::mount_first(app.state.lang, base), now),
                                None => app.report(&skyward_ui::fmt::rules_unread(app.state.lang, &format!("{e:#}")), now),
                            },
                        }
                    }
                    Effect::StopRelabel => relabeler.as_ref().context("there is no relabeling to stop")?.stop(),
                    // Partitions that cannot be read are the page's to say.
                    Effect::ReadFilesystems => app.set_filesystems(read_filesystems().map_err(|e| format!("{e:#}"))),
                    // The data partition made afresh as another filesystem
                    // is a factory reset, as the system asks for one; the
                    // rest is work on one partition. Opening the data
                    // partition and working on it are not to cross, and file
                    // transfer lets internal storage go first.
                    Effect::Filesystem { name, action } => {
                        ensure!(eraser.is_none() && part_worker.is_none(), "work on partitions is under way already");
                        if name == DATA_PARTITION && decryptor.is_some() {
                            app.report(app.state.lang.pick(DATA_OPENING), now);
                            continue;
                        }
                        if name == DATA_PARTITION && !withhold(&mut transfer, &mut app, now)? {
                            continue;
                        }
                        let part = fs_part(&name)?;
                        let (kind, task) = match action {
                            FsAction::Remake(filesystem) if name == DATA_PARTITION => {
                                erase_leaves = false;
                                eraser = Some(factory_reset(&mut app, false, false, Some(filesystem))?);
                                continue;
                            }
                            FsAction::Repair => (PartKind::Repair, partwork::Task::Repair(part)),
                            FsAction::Remake(filesystem) => (PartKind::Remake(filesystem.clone()), partwork::Task::Remake(part, filesystem)),
                        };
                        skyward_sys::kmsg(&format!("{kind:?} the filesystem of {name}, as the filesystem page asked"))?;
                        app.part_work_started(kind, false);
                        part_worker = Some(partwork::Worker::start(task)?);
                    }
                    // An install still stopping may yet write; a backup or
                    // restore waits for it.
                    Effect::Backup { parts, place, compress, verify } => {
                        ensure!(backuper.is_none(), "a backup or restore is under way already");
                        if installer.is_some() {
                            app.report(app.state.lang.pick(INSTALL_UNFINISHED), now);
                            continue;
                        }
                        let into = backups_in(place);
                        let there = into.ancestors().find(|p| p.exists()).context("the root is always there")?;
                        let ownership = match ownership(there) {
                            Ok(ownership) => ownership,
                            Err(e) => {
                                app.report(&format!("{e:#}"), now);
                                continue;
                            }
                        };
                        let made = unix_now()?;
                        let task = backup::BackupTask {
                            parts,
                            stamp: skyward_ui::fmt::stamp(made, app.state.time_zone),
                            into,
                            ownership,
                            compress,
                            verify,
                            remount: (place == Place::UsbDrive).then(|| PathBuf::from(USB_DRIVE)),
                            made,
                            device: cfg.device.codename.clone(),
                            system: info.os_version.clone(),
                            efs: cfg.backup.efs.clone(),
                            skipped: backup::data_skipped(&cfg.backup.data_skipped),
                        };
                        backuper = Some(backup::Worker::start(backup::Task::Backup(task))?);
                        app.backup_started(false, place, verify);
                    }
                    Effect::Restore { place, name, parts } => {
                        ensure!(backuper.is_none(), "a backup or restore is under way already");
                        if installer.is_some() {
                            app.report(app.state.lang.pick(INSTALL_UNFINISHED), now);
                            continue;
                        }
                        let task = backup::RestoreTask {
                            folder: backups_in(place).join(&name),
                            parts,
                            device: cfg.device.codename.clone(),
                            skipped: backup::data_skipped(&cfg.backup.data_skipped),
                            detach: detach(cfg.services.clone()),
                        };
                        backuper = Some(backup::Worker::start(backup::Task::Restore(task))?);
                        app.backup_started(true, place, true);
                    }
                    Effect::FindBackups { storage, usb } => find_backups(&mut app, storage, usb, now),
                    Effect::DeleteBackup { place, name } => {
                        let deleted = delete_backup(place, &backups_in(place).join(&name)).map_err(|e| format!("{e:#}"));
                        queue.extend(app.backup_deleted(deleted, now));
                    }
                    Effect::StopBackup => backuper.as_ref().context("there is no backup or restore to stop")?.stop(),
                    Effect::ReadUpdate => {
                        let slot = info.slot.context("only an A/B phone has an update to merge")?;
                        // A merge followed tells where it stands itself.
                        if merger.is_none() {
                            app.set_update(skyward_vab::standing(slot)?);
                        }
                        // An update in the other slot starts with that slot,
                        // which a system's updater may not have set yet.
                        app.set_boot_slots(read_slots());
                    }
                    Effect::Merge => {
                        ensure!(merger.is_none(), "the update is being merged already");
                        let slot = info.slot.context("only an A/B phone merges an update")?;
                        skyward_sys::kmsg("merging the update, as the update page asked")?;
                        merger = Some(merge::Merger::start(slot, true)?);
                    }
                    Effect::Wlan(on) => wlan_worker(&wlan)?.send(if on { wlan::Command::On } else { wlan::Command::Off }),
                    Effect::ScanWlan => {
                        scanned_at = Instant::now();
                        wlan_worker(&wlan)?.send(wlan::Command::Scan);
                    }
                    Effect::JoinWlan { ssid, name, security, password } => {
                        wlan_worker(&wlan)?.send(wlan::Command::Join { ssid, name, security, password: password.0 })
                    }
                    Effect::LeaveWlan => wlan_worker(&wlan)?.send(wlan::Command::Leave),
                    // What the installed system is called is the page's to
                    // report when it cannot be read.
                    Effect::CheckUpdate => {
                        ensure!(checker.is_none(), "the update server is being asked already");
                        let server = cfg.update.as_ref().context("the phone's update server is not known")?.server.clone();
                        let probe = cfg.wlan.as_ref().context("the phone's WLAN is not brought up")?.probe.clone();
                        match installed(cfg, &info) {
                            Ok(installed) => {
                                checker = Some(updates::Worker::start(updates::Task::Check { server, probe, installed }, servers.clone())?)
                            }
                            Err(e) => app.update_checked(Err(Unanswered::Failed(format!("{e:#}"))), Have::Nothing),
                        }
                    }
                    Effect::Download(release) => {
                        ensure!(downloader.is_none(), "the full package is being downloaded already");
                        let server = cfg.update.as_ref().context("the phone's update server is not known")?.server.clone();
                        let path = downloaded(&release);
                        match installed(cfg, &info) {
                            Ok(installed) => {
                                let task = updates::Task::Download { server, installed, release, path };
                                downloader = Some(updates::Worker::start(task, servers.clone())?);
                            }
                            Err(e) => app.download_ended(DownloadEnd::Failed(Failure::Other(format!("{e:#}"))), have(&app, &release), now),
                        }
                    }
                    Effect::StopDownload => downloader.as_ref().context("there is no download to stop")?.stop(),
                    // Logs that cannot be saved are the page's to report;
                    // the recovery carries on.
                    Effect::ExportLogs { recovery, kernel, storage } => {
                        let into = match storage {
                            true => Path::new(skyward_crypto::data::SDCARD).join(STORAGE_LOGS),
                            false => PathBuf::from(TMP_LOGS),
                        };
                        match export_logs(recovery, kernel, &into, app.state.time_zone) {
                            Ok(folder) => app.report(&skyward_ui::fmt::exported(app.state.lang, &folder.display().to_string()), now),
                            Err(e) => app.report(&skyward_ui::fmt::export_failed(app.state.lang, &format!("{e:#}")), now),
                        }
                    }
                    Effect::SetMode { path, mode } => {
                        let outcome = skyward_files::set_mode(Path::new(&path), mode).map_err(|e| format!("{e:#}"));
                        app.mode_set(mode, outcome, now);
                    }
                    Effect::OpenText(path) => {
                        let read = skyward_files::read_up_to(Path::new(&path), skyward_ui::editor::MAX_BYTES).map_err(|e| format!("{e:#}"));
                        app.text_opened(&fonts, path, read, now);
                    }
                    // The folder is listed again, for the file's new size
                    // and time.
                    Effect::SaveText { path, text } => match skyward_files::replace(Path::new(&path), text.as_bytes()) {
                        Ok(()) => {
                            app.text_saved(Ok(()), now);
                            let folder = app.state.files.path.clone();
                            list(&mut app, folder, &closed)?;
                        }
                        Err(e) => app.text_saved(Err(format!("{e:#}")), now),
                    },
                    Effect::Rename { path, name } => match skyward_files::rename(Path::new(&path), &name) {
                        Ok(_) => {
                            app.renamed(Ok(()), now);
                            let folder = app.state.files.path.clone();
                            list(&mut app, folder, &closed)?;
                        }
                        Err(e) if e.is::<skyward_files::Taken>() => app.renamed(Err(RenameRefused::Taken), now),
                        Err(e) => app.renamed(Err(RenameRefused::Failed(format!("{e:#}"))), now),
                    },
                    // A partition that will not mount, or is in use, is the
                    // page's to report; the recovery carries on.
                    Effect::Mount(base) => {
                        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                        if let Err(e) = skyward_vab::mount_one(slot, &base) {
                            app.mount_failed(MountFailure { mounting: true, base, detail: format!("{e:#}") });
                        }
                        app.set_mounts(mounts());
                    }
                    Effect::Unmount(base) => {
                        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                        if let Err(e) = skyward_vab::unmount_one(slot, &base) {
                            app.mount_failed(MountFailure { mounting: false, base, detail: format!("{e:#}") });
                        }
                        app.set_mounts(mounts());
                    }
                    // init stops fastbootd as the port goes through "none";
                    // then the slot's partitions come up as at boot.
                    Effect::EnterRecovery => {
                        skyward_sys::usb::switch(skyward_sys::usb::resting(), USB_TIMEOUT)?;
                        let slot = skyward_vab::current_slot()?.context("not an A/B phone")?;
                        crate::prepare(cfg, slot)?;
                        read_at = Instant::now();
                        info = skyward_info::collect(cfg)?;
                        fastboot = false;
                        app.enter_recovery();
                    }
                    // A port that something else has takes file transfer
                    // on, or leaves it, when it comes back to rest.
                    Effect::FileTransfer(on) => {
                        let rests = !fastboot && !sideloading;
                        match (on, transfer.take()) {
                            (true, None) => match Transfer::start(cfg, &info, app.state.time_zone) {
                                Ok(t) => {
                                    transfer = Some(t);
                                    if rests {
                                        if let Err(e) = skyward_sys::usb::switch(skyward_sys::usb::resting(), USB_TIMEOUT) {
                                            skyward_sys::kmsg(&format!("the port did not take file transfer: {e:#}"))?;
                                            app.report(&format!("{e:#}"), now);
                                        }
                                    }
                                }
                                Err(e) => {
                                    skyward_sys::kmsg(&format!("file transfer did not start: {e:#}"))?;
                                    app.file_transfer_failed(false, &skyward_ui::fmt::transfer_failed(app.state.lang, &format!("{e:#}")), now);
                                }
                            },
                            (false, Some(t)) => {
                                if let Err(e) = t.stop(rests) {
                                    skyward_sys::kmsg(&format!("file transfer did not stop cleanly: {e:#}"))?;
                                    app.report(&skyward_ui::fmt::transfer_left_on(app.state.lang, &format!("{e:#}")), now);
                                }
                            }
                            (on, t) => unreachable!("the switch turned file transfer {on} while it was {}", t.is_some()),
                        }
                    }
                }
            }
        }
        // Internal storage goes to the computer while it is open, and no
        // erasing or installing could take it away. File transfer that does
        // not follow, or whose service ended, stops, and the page says why.
        let kept = match &mut transfer {
            Some(t) if !t.service.running() => Err(anyhow!("the file transfer service ended")),
            Some(t) => {
                let offer = app.state.data.storage_open() && eraser.is_none() && installer.is_none();
                t.offer(offer, app.state.lang).and_then(|()| {
                    if t.zone != app.state.time_zone {
                        t.service.set_zone(skyward_ui::fmt::rules(app.state.time_zone).clone())?;
                        t.zone = app.state.time_zone;
                    }
                    Ok(())
                })
            }
            None => Ok(()),
        };
        if let Err(e) = kept {
            // An ended service's own error says more than its ending does.
            let why = match transfer.take().expect("only file transfer that runs fails").stop(!fastboot && !sideloading) {
                Err(cause) => cause,
                Ok(()) => e,
            };
            skyward_sys::kmsg(&format!("file transfer stops: {why:#}"))?;
            app.file_transfer_failed(false, &skyward_ui::fmt::transfer_stopped(app.state.lang, &format!("{why:#}")), now);
        }
    }
}

 令 mut screen = Canvas：：new（w， h）;一页滑过另一页，画上了三块屏幕的画布 // 除了画格之外：现在准备好了，第一张幻灯片就找到了它们。画布：：keep_ready（w， h， 3）;令 mut shown_clock = String：：new（）;这些读数会在独立的线索上再次进行;这里拍摄的//工作结束时会说手机离开时的样子，而在它们之前开始的//读数已过时。令 readings = Readings：：start（cfg）？;设 mut read_at = 瞬时：：now（）;let mut 安装程序：选项 <Installer> = 无;// 面板的帧内存是否因关闭而被黑屏 // 面板;当再次显示真实帧时清除。let mut blanked = false;// 启动 fastboot，端口会连接到手机自身的 fastbootd。//页面会显示是否出现过，以及电脑端口的距离;为什么没弹出来，会去内核日志和屏幕上。let mut fastboot = first == Screen：：Fastboot;令 mut port_at = 瞬时：：now（）;文件传输在开关开启时运行，并在打开时提供内部 // 存储;一个没有启动的屏幕会显示原因，然后恢复继续。let mut transfer： Option<Transfer> = None;如果 app.state.file_transfer { match Transfer：：start（cfg， &info， app.state.time_zone） { Ok（t） = > transfer = Some（t）， Err（e） => { skyward_sys：：kmsg（&format！（”文件传输未启动：{e：#}“）？;app.file_transfer_failed（false， &skyward_ui：：fmt：：transfer_failed（app.state.lang， &format！（”{e：#}“）），monotonic_ms（））;} } } // 侧载有端口，端口结束后返回休止。设 mut 侧载 = false;if fastboot { if let Err（e） = skyward_sys：：usb：：switch（“fastboot”， USB_TIMEOUT） { skyward_sys：：kmsg（&format！（”Fastbootd 尚未移植：{e：#}“）？;App.report（&format！（“{e：#}”）， monotonic_ms（））;} 令（运行，链接） = fastboot_status（）？;app.set_fastboot（跑步，链接）;} // 请求进行的工作会让请求保持在 misc，// 所以重启会回到中间，直到任务完成或用户 // 把它留给别处。令 misc = bcb：：misc（）;让 mut 保持 = request.work.lasts（）;擦除页面会显示手机表为 // 的缓存分区;不读取的表格会省略该页面，页面仍然能使用 // 。app.state.cache_partition = 匹配清除：：cache（） { Ok（cache） => cache.is_some（）， Err（e） => { skyward_sys：：kmsg（&format！（”手机是否有缓存分区尚不得而知：{e：#}“）？;false } };app.state.phone_logs = cfg.wipe.logs.len（）;Skyward 存在于手机 // 所在插槽的恢复分区中，安装页面会提供保留它的位置。设 by_name = Path：：new（skyward_block：：BY_NAME）;app.state.recovery_partitions = by_name.join（“recovery_a”）.try_exists（）？& & by_name.join（“recovery_b”）.try_exists（）？;let mut eraser： Option<Eraser> = 无;// 删除请求的系统完成后自动离开，就像删除请求的更新并侧载后重启一样;// 用户选择的内容等待他们。让 mut erase_leaves = false;// 删除所有用户数据救援请求，其终点回到救援 // 而不是独立页面。让 mut rescue_erasing = false;// 救援请求手机等它放手 USB 后再走。let mut rescue_leaves： Option<（Target， bool）> = 无;// 系统请求安装，直到其所在的数据分区打开，或其文件中持有该分区的用户解锁该分区。 let mut pending： Option<Pending> = None;// 正在处理整个分区，以及系统是否在恢复开始前要求 // 。let mut part_worker： Option<partwork：：Worker> = None; match &request.work { Work：：WipeData { reformat } => { eraser = Some（factory_reset（&mut app， request.keep_memtag_mode， true， reformat.clone（）？）; erase_leaves = true; }Work：：WipeCache { reformat } => { eraser = Some（parts_wipe（&mut app， vec！[step：：Cache]， &cfg.wipe.logs， true， reformat.clone（）？）;erase_leaves = 真;} Work：：Sideload { .. } => { app.start_sideload（）; installer = Some（Installer：：start（detach（cfg.services.clone（））， install_options（&app））？）; sideloading = true; }Work：：Rescue => { app.start_rescue（）; installer = Some（Installer：：rescue（detach（cfg.services.clone（））， install_options（&app））？）; sideloading = true; }Work：：Update（update） => { app.start_asked（update.security， update.package.clone（））; let retry = update.retry > 0; if ！retry && panicked（） { skyward_sys：：kmsg（“手机在内核停止后才来到这里，所以请求的更新未安装”）？app.install（skyward_install：：Event：：Finished（skyward_install：：Outcome：：Refused（skyward_install：：Refusal：:P anicked））， monotonic_ms（））; bcb：：clear（&misc）？; kept = false; } 否则 { let （package， needs） = asked：：locate（&update.package， app.state.cache_partition）;skyward_sys：：kmsg（&format！（”请求的更新是 {package：？}，需要 {needs：？}“）？;let want = 待处理 { package， needs， retry };匹配需求 { 需求：:D ata |Needs：：User（_） => { app.asked_opening（）; decryptor = Some（Decryptor：：start（decrypt：：Job：：Open）？）; pending = Some（wanted）; }需求：：无 |需要：：Cache =>安装器 = Some（install_asked（cfg， &app， wanted？）， } } } Work：：WipeAb { package_size } => { app.part_work_started（PartKind：:D isable， true）;part_worker = Some（partwork：：Worker：：start（partwork：：Task：:D isable { package_size： *package_size }）？）;} Work：：UnshareBlocks => { let slot = skyward_vab：：current_slot（）？.上下文（“不是 A/B 电话”）？app.part_work_started（PartKind：：Unshare，真）;part_worker = Some（partwork：：Worker：：start（partwork：：Task：：Unshare { slot }）？）;} 工作：：菜单 |工作：：出口 |工作：：快启动 |作品：:P romptWipeData |Work：：Unsupported（_） => {} } // 移植时只显示了 adb;静止时旁边有文件传输 // 。如果 transfer.is_some（） & & ！fastboot && ！sideloading { if let Err（e） = skyward_sys：：USB：：switch（skyward_sys：：usb：：resting（）， USB_TIMEOUT） { skyward_sys：：kmsg（&format！（”该移植版未进行文件传输：{e：#}“）？;app.report（&format！（”{e：#}“）， monotonic_ms（））;} } // 系统要求的工作完成后，页面会显示该工作 // 片刻;然后手机启动系统，或者在//请求指示时关机。let leave = 如果 request.shutdown_after { Target：:P owerOff } 否则 { Target：：System };let mut leave_at： Option<u64> = None;// 终端页面的 shell，从页面首次请求时起。let mut shell： Option<pty：：Shell> = 无;// 正在复制、移动或删除。let mut worker： Option<filework：：Worker> = None;// 正在为数据分区的文件赋予 SELinux 标签。let mut retager： Option<filework：：Worker> = None;// 正在备份或恢复中。让 mut 备份： Option<backup：：Worker> = 无;// 更新合并随后发生：恢复正在进行中，内核在快照映射时重新处理，或在更新页面开始合并 // 。让 mut 合并：Option<merge：：Merger> = 无;如果 Some（slot） = info.slot { let standing = skyward_vab：：standing（slot）？;如果匹配的话！（站立，站立：：合并中（_））& skyward_vab：：snapshots_mapped（槽）？{ skyward_sys：：kmsg（“更新的合并继续”）？merger = Some（merge：：Merger：：start（slot， false）？）;app.set_update（站立）;} // 网络加入的 DNS 服务器，用于查找名称。让服务器：服务器 = Arc：：new（Mutex：：new（Vec：：new（））;WLAN，在恢复时显示的驱动驱动的手机上;它以 // 开头。没有供应商接口库的捆绑，// 请求者不会启动，WLAN 页面说明了原因。let wlan = match &cfg.wlan { Some（w） => { app.state.wlan = Some（Wlan：:d efault（））; let modules = cfg.modules.sources.iter（）.map（|s| skyward_modules：：source_dir（s））.collect：：<Result<Vec<_>>>（）？; let libraries = secure：：libraries（）.unwrap_or_else（|e|{ 让 _ = skyward_sys：：kmsg（&format！（”供应商接口的库未开放：{e：#}“））;无 }）;Some（wlan：：Worker：：start（w.clone（）， modules， libraries， servers.clone（）？） }无 => 无，};设 mut scanned_at = 瞬时：：now（）;app.state.update_server = cfg.update.is_some（）;向更新服务器请求，并下载完整软件包。let mut checker： Option<updates：：Worker> = None; let mut downloader： Option<updates：：Worker> = None; // 插入的 USB 驱动器，每秒查找。let mut plugged： Option<Plugged> = None; let mut seeen： Option<String> = None; let mut drive_at = Instant：：now（）; loop { let now = monotonic_ms（）; if leave_at.is_some_and（|at| now >= at） { leave_at = None; power（leave）？; } if ！touch_known && started.elapsed（） >= TOUCH_WAIT { touch_known = true; if ！inputs.has_panel（） { skyward_sys：：KMSG（“未出现触摸面板;页面是用按键来处理的）？app.touchscreen_missing（）;} } 用于应用中的 FX。tick（now） { match fx { Effect：：触觉 => { 如果 让 Some（h） = &mut 触觉 { h.click（app.state.vibration）？; } } other => 无法到达！（”时间流逝只需轻敲电机，而非{other：？}“），} } 令时间=时钟（app.state.time_zone）？;如果时间 ！= shown_clock { shown_clock = 时间; app.redraw（）; } 用于读取阅读。taken（） { match reading { Reading：：Battery（battery） => { let battery = battery？; if battery ！= info.battery { info.battery = battery; app.refresh（）; } }Reading：：All（all， at） => { let all = （*all）？; if at >= read_at & all ！= info { info = all; app.refresh（）; } } } }} // 在 fastboot 中，页面紧跟 fastbootd，线缆每秒跟随。如果 fastboot & & port_at.elapsed（）.as_secs（）  >= 1 { let （running， link） = fastboot_status（）？; app.set_fastboot（running， link）; port_at = Instant：now（）; } let panel = app.panel（now）;设 mut = 假;if panel.is_some（） { if  app.take_dirty（） { let t0 = Instant：：now（）; app.compose（&mut screen， &fonts， &info， &shown_clock， now）; surface.present（&screen）？; eprintln！（”frame {：？} in {} ms“， app.screen（）， t0.elapsed（）.as_millis（））;如果让 Some（path） = dump { fs：：write（path， &screen.data）.with_context（|| format！（”write {}“， path.display（））？;} 显示 = 真;} 空白 = 假;} 否则如果 ！blanked { // 在背光变为 // 到 0 之前，面板的帧内存会被黑屏，所以如果 OLED 把背光降到零以上而不是切断，则什么都看不见，也无法烧屏。screen.data.fill（0）; surface.present（&screen）？; surface.settle（）？; blanked = true; } // 当画面进入面板后，面板重新亮起 // 显示现在应该显示的光， 从未因为它响起时显示的效果而出现 // 。如果 backlight.as_ref（）.is_some_and（|b| b.level == 0） & & panel.is_some（） { surface.settle（）？;} // 页面就是这个循环的用途;一个背光面板 // 无法写入，仍然显示这些记录，以及为什么不能直接进入内核日志一次，而不是停止恢复。let lit = 匹配 backlight.as_mut（） { Some（b） => b.show（panel）.map_err（|e| format！（”{e：#}“）），None => Ok（（））， };如果设 Err（e） = lit { let _ = skyward_sys：：kmsg（&format！（”面板背光未设置：{e}“））;背光 = 无;} // 静止的页面绘制尚未绘制的行，而其他工作则不做，一次只画一带，寻找带间的输入。let ahead = panel.is_some（） && ！shown & & app.draw_ahead（&fonts， &info）;画面中的一帧等待前一个进入//面板，保持循环与面板同步;否则 CPU 至少会有 1 毫秒。设 floor = 如果 显示 { 0 } 否则 { 1 };设 until_due = app.deadline（now）.map（|d| d.saturating_sub（now） as i32）;设超时 = 如果前进 { 0 } 否则 { until_due.map_or（1000， |t| t.clamp（floor， 1000））};令 mut fds = inputs.fds（）;如果让 Some（i） = &installer { fds.push（i.fd（））; } 如果让 Some（c） = &control { fds.] push（c.fd（））; } 如果让 Some（e） = &eraser { fds.push（e.fd（））; } 如果让 Some（sh） = &shell { fds.extend（sh.fds（））; } if let Some（w） = &worker { fds.push（w.fd（））; } if let Some（r） = &retager { fds.push（r.fd（））; } if let Some（b） = &backuper { fds.push（b.fd（））; } if let Some（m） = &merger { fds.push（m.fd（））; } if let Some（d） = & 解密器 { fds.push（d.fd（））; } if let Some（p） = &part_worker { fds.push（p.fd（））; } 如果 letSome（w） = &wlan { fds.push（w.fd（））; } if let Some（c） = &checker { fds.push（c.fd（））; } if let Some（d） = &downloader { fds.push（d.fd（））; } fds.push（readings.fd（））;wait（和 FDS，暂停）？;令 = monotonic_ms（）;设 mut drive_told = 无;如果 drive_at。已经过（）。as_secs（） >= 1 { drive_told = look_for_drive（&mut plugged， &mut seen）？; drive_at = 瞬时：：now（）; } // 安装程序和橡皮擦的指示先行，因此触摸 // 对页面的现状起作用。{ let told = installer.as_ref（）.map（Installer：：events）.unwrap_or_default（）; ev in told { 使用 skyward_install：：{事件如述，结果}; let finished = matches！（ev， Told：：Finished（_））;安装 = 匹配！（ev， Told：：Finished（结果：：已安装 { .. } |结果：：Ran { .. }））;让 wipe_after=火柴！（ev， Told：：Finished（Outcome：：Installed { wipe： true， .. }））;让 script_cache=火柴！（ev， Told：：Finished（结果：：Ran { wipe_cache： true， .. }））;EprintLN！（“安装 {EV：？}”）;匹配 & { // 救援队自己的抹除告诉救援队在结束后情况;文件传输不会让内部存储丢失，导致无法启动。Told：：Wipe => { 确保！（eraser.is_none（），“抹除已经开始”）;如果扣款（并立即转账和 MUT 应用）？{ eraser = Some（rescue_wipe（&mut app）？）; rescue_erasing = true; }否则 { installer.as_ref（）.context（“救援请求删除”）？wiped（false）;} } Told：：Reboot（target） => rescue_leaves = Some（rescue_target（target））， _ => {} } app.install（ev， now）;如果完成 { installer = None;sideloading = false;// 系统请求的侧载或更新会随之完成 // 无论运行如何，救援也随之完成，只要页面被移除或电脑请求手机离开。// 侧载请求在它之后自动离开，而一个已进入的 // 更新也会被删除，而包裹或请求删除的内容一旦被删除。如果匹配的话！（request.work，工作：：侧载 { .. } |作品：：更新（_） |工作：：救援） & 保持 { bcb：：clear（&misc）？; kept = false; } let （leaves， cache_after） = 匹配与请求。work { Work：：Sideload { auto_reboot } => （*auto_reboot， script_cache）， Work：：Update（update） => （已安装，script_cache ||（安装 & & update.wipe_cache））， _ => （false， script_cache）， };如果 cache_after & & ！app.state.cache_partition { skyward_sys：：KMSG（“安装请求删除缓存分区，而手机没有缓存”）？; } 如果 wipe_after { 确保！（eraser.is_none（），“抹除已经开始”）;擦除器 = Some（factory_reset（&mut app， false， false， None）？）;erase_leaves = 叶子;} 否则 如果 cache_after & & app.state.cache_partition { ensure！（eraser.is_none（），“抹除已经开始”）;eraser = Some（parts_wipe（&mut app， vec！[Step::Cache], &cfg.wipe.logs, false, None)?);erase_leaves = 叶子;} 否则如果离开 { leave_at = Some（现在 + LEAVE_AFTER_MS）; } // 救援被要求回来的消息在杂项中被告知，正如 // 安卓自己的恢复记录所说。if let Some（（target， again）） = rescue_leaves.take（） { if again { bcb：：write（&misc， &bcb：：Bcb { 命令： “boot-rescue”.into（）， args： Vec：：new（） }）？; } power（target）？; } }} } 令 mut 被抹除 = false;如果让 Some（e） = &eraser { 用于 e.events（） { match ev { // 救援的页面只显示正在进行抹除。wipe：：Event：：At（_） | wipe：：Event：:P rogress（..） 如果 rescue_erasing => {} wipe：：Event：：At（step） => app.erase_at（part（step））， wipe：：Event：:P rogress（完成，total） => app.erase_progress（完成，total），// 救援保持在线：其请求保持在 misc，// 电脑读取擦除结果。清除：：事件：：结束（结果） 如果 rescue_erasing => { let said = outcome.as_ref（）.map_or_else（wipe：：said， |_|“完成”.to_string（））;skyward_sys：：kmsg（&format！（”为救援而抹除已结束：{说}“）？;installer.as_ref（）.context（“救援等待抹除结束”）？wiped（outcome.is_ok（））;app.rescue_wiped（结果）。map_err（erase_stop），现）;if app.state.data == Data：：Erased { closed.clear（）; } rescue_erasing = false;抹除 = 真;} wipe：：Event：：Opened（opened） => { if let Some（mounted） = &opening.mounted { log_mounted（mounted）？; } closed = opening.closed; show_closed（&mut app， &closed）？; } wipe：：Event：：Ended（outcome） => {let said = outcome.as_ref（）.map_or_else（wipe：：said， |_|“完成”.to_string（））;skyward_sys：：kmsg（&format！（”抹除结束：{说}“）？;和手机自身的恢复一样，无论哪种方式，请求都会完成//，所以重启系统会启动，而不是再次擦除。恢复出厂设置是更新时要求的 // 恢复。if keep { bcb：：clear（&misc）？; kept = false; } 如果 outcome.is_ok（） && app.state.erase.reset { bcb：：clear（&misc）？; } if outcome.is_ok（） && erase_leaves { leave_at = Some（now + LEAVE_AFTER_MS）; } app.erase_ended（outcome.map_err（erase_stop）， now）;数据分区被完全清除的用户将无 // 文件可打开。if app.state.data == Data：：Erased { closed.clear（）; } erased = true;} } } } 如果被抹除 { eraser = None; } 令 job_end = |end： filework：：End|match end { filework：：End：:D one => JobEnd：:D one， filework：：End：：Stopped => JobEnd：：Stopped， filework：：End：：Failed（wwhy） => JobEnd：：Failed（why）， };令 mut work=false;如果让 Some（w） = &worker { for ev in w.events（） { match ev { filework：：Event：:P rogress（p） = > app.file_progress（（p.bytes， p.total_bytes）， （p.entries， p.total_entries））， filework：：Event：：Relabeling（_） => 不可达！（”文件工作不给标签“），filework：：Event：：Ended（结束） => { app.file_job_ended（job_end（结束），现在;worked = true; } }} } 令 mut 重新标记为 = false;如果让 Some（r） = &relabeler { for ev in r.events（） { match ev { filework：：Event：：Relabeling（p） => app.relabel_progress（p.checked， p.total， p.changed）， filework：：Event：:P rogress（_） => 无法访问！（“重新标记移动无字节”），filework：：Event：：Ended（结束） = > { app.relabel_ended（job_end（结束），现在）; relabeled = true; } }} } 如果重新标记 { relabeler = 无; } 令 mut parted = false;如果让 Some（p） = &part_worker { for ev in p.events（） { match ev { partwork：：Event：：Listed（names） => app.part_listed（names）， partwork：：Event：：At（index） => app.part_at（index）， partwork：：Event：：Ended（index， item， said） => { skyward_sys：：kmsg（&format！（”分割工作：项 {index} 结尾为 {item：？}“）？;app.part_ended（索引，item_end（条目），&said）;} partwork：：Event：:D one（done） => { skyward_sys：：kmsg（&format！（”分割工作结束：{完成：？}“）？;让工作 = app.state.part_work.as_ref（）.context（“分区工作显示在其页面上”）？;设 whole = done.is_ok（） & ！work.items.iter（）.any（|（_， s）|火柴！（s， ItemState：：Ended（ItemEnd：：Failed（_））））;let data = work.items.iter（）.any（|（姓名，_）|名称 == DATA_PARTITION）;系统要求的操作无论哪种方式都做完了，// 手机在通过 // 所有分区后会自动离开。如果 work.asked { if kept { bcb：：clear（&misc）？; kept = false; } if whole { leave_at = Some（now + LEAVE_AFTER_MS）; } } // Work on the data partition 会关闭该分区。如果数据 { 关闭.clear（）; } app.part_work_ended（done.map_err（part_stop）， 现在）;parted = true;} } } 如果分开 { part_worker = 无; } // 显示的文件夹会根据作品所更改的内容再次读取。如果 work，{ worker = 无;let folder = app.state.files.path.clone（）; list（&mut app， folder， &closed）？; } let mut backed = false;如果让 Some（b） = &backuper { for ev in b.events（） { match ev { backup：：Event：:P lan（plan） => app.backup_plan（plan）， backup：：Event：：At { item， done， of， checking } => 应用。backup_at（BackupAt { item， done， of， checking }）， backup：：Event：：Ended（end） => { app.backup_ended（backup_end（end）， now）; backed = true; } }} } 如果支持 { backuper = 无; } if let Some（m） = &merger { let slot = info.slot.context（“只有 A/B 手机会合并更新”）？;对于 ev in m.events（） { match ev { merge：：Event：：Looked（look） => { eprintln！（”merge {look：？}“）;app.set_update（match look { 合并：：Going（完成） = > 站立：：合并（完成），合并：:D one |合并：：需要重启 => 存档：：合并，合并：：失败（代码） >= 立场：：失败（代码），合并：：取消 |Merge：：Idle（_） => skyward_vab：：standing（slot）？， }）;} merge：：Event：：Failed（why） => { skyward_sys：：kmsg（&format！（”更新未合并：{why}“）？;app.report（&skyward_ui：：fmt：：merge_failed（app.state.lang， &wwhy）， now）;app.set_update（skyward_vab：：standing（slot）？）;} } } // 无论是否合并，手机的读数显示更新当前位置 // 。if m.ended（） { merger = 无; read_at = Instant：：now（）; info = skyward_info：：collect（cfg）？; app.refresh（）; } }if let Some（（told， traffic）） = wlan.as_ref（）.map（wlan：：Worker：：told） { if let Some（w） = told { app.set_wlan（w）; } if let Some（t） = traffic { app.set_traffic（t）; } } // WLAN 页面在显示时会不时再次扫描，就像//库存设置一样。让 wlan_on = app.state.wlan.as_ref（）.is_some_and（|w| w.power == WlanPower：：On）;if app.screen（） == Screen：：Wlan && wlan_on && scanned_at.elapsed（） >= SCAN_EVERY { scanned_at = Instant：：now（）; wlan_worker（&wlan）？.发送（wlan：：Command：：Scan）;} 令 mut 检查 = false;如果让 Some（c） = &checker { for ev in c.events（） { let updates：：Event：：Checked（answer） = ev 否则 { 不可达！（”向服务器请求不会下载任何东西“） };让 have = match & answer { Ok（Some（release）） => have（&app， release）， _ => Have：：Nothing， };应用程序。update_checked（回答，有）;检查 = 真;} } 如果检查 { checker = 无; } 令 mut 取 = false;if let Some（d） = &downloader { for ev in d.events（） { match ev { updates：：Event：:P rogress（progress， rate） => app.download_progress（progress， rate）， updates：：Event：：Ended（end） => { let release = app.state.newest（）.context（“a download is of the newest system”）？.clone（）;let end = match end { 更新：：End：:D ownloaded => DownloadEnd：:D ownloaded， updates：：End：：Stopped => DownloadEnd：：Stopped， updates：：End：：Failed（wwhy） => DownloadEnd：：Failed（why）， };skyward_sys：：kmsg（&format！（”下载 {} 结束： {end：？}“， release.name））？;app.download_ended（结束，拥有（&应用，释放），现在）;取回 = 真;} 更新：：事件：：已检查（_） =>无法访问！（“下载本身无需要求”），} } } 若被获取 { downloader = None; } 若 let Some（ended） = decryptor.as_ref（）.and_then（Decryptor：：ended） { decryptor = None; match ended { decrypt：：Ended：：Opened（OK（opening）） => { if let Some（mounted） = &opening.mounted { log_mounted（mounted）？; } closed = opening.closed; show_closed（&mut app， &closed）？; } decrypt：：Ended：：Opened（Err（why）） => { skyward_sys：：kmsg（&format！（”数据分区保持关闭：{why}“）？;系统要求的更新无法读取，// 已经完成。if pending.take（）.is_some（） { let failed =  skyward_install：：Outcome：：Failed（skyward_install：：Failure：：Unreadable（why.clone（）））; app.install（skyward_install：：Event：：Finished（failed）， now）; if kept { bcb：：clear（&misc）？; kept = false; } } app.data_failed（why）;} decrypt：：Ended：：Unlocked（Ok（still）） => { closed = still; show_closed（&mut app， &closed）？;} 解密：：结束：：解锁（Err（why）） => { // 凭证属于哪个用户，这样持有恢复文件的页面 // 在第二个空格提示时不会被误读为拥有者自己的凭证错误。让谁 = 匹配 app.state.data { Data：：Locked { 用户， .. } =>格式！（”用户 {user} 的文件“），_ => ”用户自己的文件“.to_string（）， };skyward_sys：：kmsg（&format！（”输入的内容未打开{who}： {why}“）？;app.unlock_failed（为什么，现在）;} } // 在内部存储关闭时截图会在相册打开后进入 // 。如果 app.state.place_open（Place：：Storage） { 如果让 Err（e） = keep_held_screenshots（） { 让 why = 格式！（”{e：#}“）;skyward_sys：：kmsg（&format！（”所持有的截图未进入画廊：{why}“）？;app.screenshots_not_kept（以及为什么，现在）;} } // 系统请求的更新一旦保持该更新的 // 打开时开始;直到拥有自己文件的用户解锁，// 页面会说要解锁。if let Some（wanted） = pending.take（） { match wanted.needs { Needs：：User（user） if closed.iter（）.any（|c| c.user == user） => { app.asked_locked（user， now）; pending = Some（wanted）; } _ => installer = Some（install_asked（cfg， &app， wanted）？）， } } } 如果 let Some（sh） = &mut shell { let wrote = sh.read（）？; if ！wrote.is_empty（） { app.term_output（&wrote）; } if let Some（status） = sh.ended（）？{ // 它最后写的内容显示在结束前。let last = sh.read（）？如果 ！last.is_empty（） { app.term_output（&last）; } app.term_ended（pty：：how（status））; shell = 无; }} // 然后程序在恢复插槽上问了什么，每个程序都回答为 // 恢复在它轮到时保持。让请求 = 匹配 & 控制 { Some（c） => c.requests（）？， None = > Vec：：new（）， };令 incoming = 到达 .take（） .into_iter（） .chain（drive_told.map（Incoming：:D rive）） .chain（requests.into_iter（）.map（Incoming：：Asked）） .chain（inputs.read（）？.into_iter（）.map（Incoming：：Input））;for came in incoming { let effects = match came { Incoming：：Arrived => app.arrive（）， Incoming：:D rive（drive） => app.set_usb_drive（drive）， Incoming：：Asked（asked） => { skyward_sys：：kmsg（&format！（”在恢复插槽上问：{asked：？}“）？;answer（asked， fastboot） } Incoming：：Input（Event：：Touch（t， at）） => app.touch（t， &fonts， at）， Incoming：：Input（Event：：Key（k， true）） => app.key（k， now）， Incoming：：Input（Event：：Key（k， false）） => app.key_up（k， now）， };页面对工作内容的回答会在后面继续 // 。令 mut queue = VecDeque：：from（effects）;而令 Some（fx） = queue.pop_front（） { eprintln！（”效果 {FX：？}“）;匹配效果 { // 离开是用户对仍保留在杂项中请求的回答。效果：：Reboot（目标） => { 如果保持 { bcb：：clear（&misc）？; kept = false; } power（target）？} power（target）？ }效果：:D ismiss => { 如果保持 { bcb：：clear（&misc）？; kept = false; } }效果：：FactoryReset => { ensure！（eraser.is_none（），“抹除已经开始”）;if ！withhold（并且立即进行 MUT 转移和 MUT 应用）？{ 继续; }eraser = Some（factory_reset（&mut app， request.keep_memtag_mode， true， None）？）;erase_leaves = 真;} // 打开数据分区并擦除它并非 // 交叉;页面上说要等正在进行的那辆车。效果：：清除（选择） => { 确保！（eraser.is_none（），“抹除已经开始”）;如果 decryptor.is_some（） { app.report（app.state.lang.pick（DATA_OPENING）， now）; continue; } 如果 ！withhold（&mut transfer， &mut app， now）？{ 继续; }erase_leaves = 假;if chosen.contains（&Wipe：：All） { eraser = Some（factory_reset（&mut app， false， false， None）？）; } else if chosen.iter（）.any（|&w| w ！= Wipe：：Cache） && app.state.data == Data：：Erased { app.erase_started（false， false， Vec：：new（））;app.erase_ended（Err（ErseStop：：Erased），现在）;} 否则 { let steps = chosen.into_iter（）.map（step）.collect（）; eraser = Some（parts_wipe（&mut app， steps， &cfg.wipe.logs， false， None）？）; } }Effect：：Install => installer.as_ref（）.context（“没有安装程序保留更新”）？install（）， // 一个无法启动或无法接受密钥的 shell，// 是要报告的页面;恢复工作仍在继续。Effect：：StartShell（folder） => { ensure！（shell.is_none（）， “一个壳已经跑了”）;令（行，孔）= app.term_size（&fonts）;match pty：：Shell：：start（rows， cols， &cfg.device.codename， Path：：new（&folder）） { Ok（sh） => { shell = Some（sh）; app.term_started（rows， cols）; }Err（e） => { skyward_sys：：kmsg（&format！（”终端无壳：{e：#}“）？;app.report（&format！（”{e：#}“），现在）;} } } Effect：：TermInput（bytes） => { let sh = shell.as_mut（）.context（“键已访问已结束的 shell”）？; if let Err（e） = sh.write（&bytes） { app.report（&format！（”{e：#}“），现在）;} } Effect：：ShellTo（folder） => { let sh = shell.as_mut（）.context（“a folder went to a shell that have ended”）？; let went = match sh.waiting（） { Ok（true） => sh.change_to（&folder）， Ok（false） => { app.report（app.state.lang.pick（skyward_ui：：fmt：：TERMINAL_BUSY）， now）;Ok（（）） } Err（e） => Err（e）， };如果让 Err（e） = went { app.report（&format！（”{e：#}“），现在）;} } 效果：：触觉 => { 如果让 Some（h） = &mut 触觉 { h.click（app.state.vibration）？; } } // 面板显示最后绘制的帧，即保存的内容;其中一个不能被报告的就是页面。Effect：：Screenshot { storage} => { let t0 = Instant：：now（）; let saved = save_screenshot（&screen， storage， app.state.time_zone）.map_err（|e| format！（”{e：#}“））;EprintLN！（“截图 {已保存：？}在 {} ms“中，t0.elapsed（）.as_millis（））;应用程序。screenshot_saved（已保存）;} Effect：：Sideload => { ensure！（installer.is_none（）， “安装程序已经在运行”）;if ！withhold（并且立即进行 MUT 转移和 MUT 应用）？{ 继续; }installer = Some（Installer：：start（detach（cfg.services.clone（））， install_options（&app））？）;侧载 = true;} Effect：：InstallFile（place， path） => { ensure！（installer.is_none（）， “安装程序已经在运行”）;if ！withhold（并且立即进行 MUT 转移和 MUT 应用）？{ 继续; }installer = Some（Installer：：open（in_place（place， &path）， detach（cfg.services.clone（））， install_options（&app））？）;} Effect：：InstallPath（path） => { ensure！（installer.is_none（）， “安装程序已经在运行”）;if ！withhold（并且立即进行 MUT 转移和 MUT 应用）？{ 继续; }installer = Some（Installer：：open（PathBuf：：from（path）， detach（cfg.services.clone（））， install_options（&app）？）;} Effect：：Flash { partition， pick } => {let slot = skyward_vab：：current_slot（）？.上下文（“不是 A/B 电话”）？installer.as_ref（）.context（“no installer holds a image”）？flash（partition， pick.slots（slot））;} Effect：：Cancel => match & installer { Some（i） => i.cancel（）？， // 系统请求的更新，等待持有该文件的用户的//锁，在页面被关闭时被放弃/，然后完成。None => { ensure！（pending.take（）.is_some（）， “无安装程序可取消”）;skyward_sys：：kmsg（“请求的更新已放弃，其页面已离开）？;如果保持 { bcb：：clear（&misc）？; kept = false; } 让取消 = skyward_install：：Event：：Finished（skyward_install：：Outcome：：Cancelled）;app.install（取消，现）;} }， Effect：：ReadMounts => app.set_mounts（mounts（））， Effect：：ReadSlots => app.set_boot_slots（read_slots（））， Effect：：SetSlot（slot） => 应用。slot_switched（set_slot（（slot），现在），// 解密页面显示工作正在进行中 // 完成前不会再多花时间。Effect：：OpenData => { ensure！（decryptor.is_none（）， “数据分区正在被打开”）;decryptor = Some（Decryptor：：start（decrypt：：Job：：Open）？）;} 效果：：解锁 { 用户，代码 } => { ensure！（decryptor.is_none（）， “数据分区正在被打开”）;let job = decrypt：：Job：：Unlock { closed： closed.clone（）， user， credential： code.into_bytes（） };decryptor = Some（Decryptor：：start（job）？）;} Effect：：ReadDir（folder） =>列表（&mut app， folder， &closed）？， Effect：：ReadPlace（place， folder） => list_place（&mut app， place， folder）？， // 条目位置的名称由用户在写入前决定。效果：:P aste { op， path， into， clash } => { ensure！（worker.is_none（），“文件工作已在进行中”）;让 clash = 匹配冲突 { Some（choice） => clash_rule（choice）， None => match skyward_files：：clashes（&[PathBuf：：from（&path）]， Path：：new（&into））） { Ok（taken） 如果 taken.is_empty（） => skyward_files：：Clash：：Refuse， Ok（taken） => { let names = taken.iter（）.map（|t| t.file_name（）.map_or_else（String：：new， |n| n.to_string_lossy（）.into_owned（）））.collect（）; app.file_clash（op， path， into， names）; continue; }Err（e） => { app.report（&format！（”{e：#}“），现在）;继续;} }, };let ownership = match ownership（Path：：new（&into）） { Ok（ownership） => ownership， Err（e） => { app.report（&format！（“{e：#}”），现在）;继续;} };令 （from， to） = （PathBuf：：from（&path）， PathBuf：：from（&into））;let（任务，显示） = 匹配 op { FileOp：：Copy => （filework：：Task：：Copy { from， into： to， clash， ownership }， FileTask：：Copy { path， into }）， FileOp：：Move => （filework：：Task：：Move { from， into： to， clash， ownership }， FileTask：：Move { path， into }）， };worker = Some（filework：：Worker：：start（task）？）;app.file_job_started（画面）;} 效果：:D elete（path） => { ensure！（worker.is_none（），“文件工作已在进行中”）;worker = Some（filework：：Worker：：start（filework：：Task：:D elete { path： PathBuf：：from（&path） }）？）;app.file_job_started（FileTask：:D elete { path }）;} Effect：：StopJob => worker.as_ref（）.context（“文件上没有工作可停止”）？stop（）， // 未挂载的分区保留规则是要报告的//页面，并说明先应做什么。效果：：重新标签 => { ensure！（relabeler.is_none（），“重新标签已经在进行中”）;设 Slot = skyward_vab：：current_slot（）？。上下文（“不是 A/B 电话”）？match relabel：：rule_files（slot） { Ok（rules） => { relabeler = Some（filework：：Worker：：start（filework：：Worker：：start（filework：：Task：：Relabel { rules }）？）; app.relabel_started（）; }Err（e） => match e.downcast_ref：：<relabel：：Unmounted>（） { Some（relabel：：Unmounted（base）） => app.report（&skyward_ui：：fmt：：mount_first（app.state.lang， base）， now）， none => app.report（&skyward_ui：：fmt：：rules_unread（app.state.lang， &format！（”{e：#}“）），现在）， }， } } Effect：：StopRelabel => relabeler.as_ref（）.context（”停止没有重新标签）？“。stop（）， // 无法读取的分区是页面的。Effect：：ReadFilesystems => app.set_filesystems（read_filesystems（）.map_err（|e| 格式！（“{e：#}”）））， // 作为另一个文件系统重新创建的数据分区 // 是出厂重置，因为系统要求恢复出厂设置;// 余音表示在一个分区上工作。打开数据分区并处理它不可交叉，文件//传输允许内部存储优先。Effect：：Filesystem { 名称，操作 } => { ensure！（eraser.is_none（） & & part_worker.is_none（）， “分区工作正在进行中”）;if name == DATA_PARTITION && decryptor.is_some（） { app.report（app.state.lang.pick（DATA_OPENING）， now）; continue; } if name == DATA_PARTITION && ！withhold（&mut transfer， &mut app， now）？{ 继续; }设部分 = fs_part（&name）？;let （kind， task） = match action { FsAction：：Remake（filesystem） if name == DATA_PARTITION => { erase_leaves = false; eraser = Some（factory_reset（&mut app， false， false， Some（filesystem））？）; continue; }FsAction：：Repair => （PartKind：：Repair， partwork：：Task：：Repair（part））， FsAction：：Remake（文件系统） => （PartKind：：Remake（filesystem.clone（））， partwork：：Task：：Remake（part， filesystem））， };skyward_sys：：kmsg（&format！（”{kind：？}{name}的文件系统，正如文件系统页面所问））？;app.part_work_started（同类，虚假）;part_worker = Some（partwork：：Worker：：start（task）？）;} // 仍然停止的安装仍可写入;备份或//恢复会等着。Effect：：Backup { parts， place， compress， verify } => { ensure！（backuper.is_none（）， “备份或恢复正在进行中”）;如果 installer.is_some（） { app.report（app.state.lang.pick（INSTALL_UNFINISHED）， now）; continue; } let into = backups_in（place）;设 there = into.ancestors（）.find（|p| p.exists（））.context（“根总是在那里”）？;让所有权 = 匹配所有权（在那里） { Ok（所有权） =>拥有权，Err（e） => { app.report（&format！（”{e：#}“），现在）;继续;} };让 made = unix_now（）？;let task = backup：：BackupTask { parts， stamp： skyward_ui：：fmt：：stamp（made， app.state.time_zone）， into， ownership， compress， verify， remount： （place == Place：：UsbDrive）.then（||PathBuf：：from（USB_DRIVE））， made， device： cfg.device.codename.clone（）， system： info.os_version.clone（）， efs： cfg.backup.efs.clone（）， skipped： backup：:d ata_skipped（&cfg.backup.data_skipped）， };backuper = Some（backup：：Worker：：start（backup：：Task：：Backup（task））？）;app.backup_started（false， place， verify）;} 效果：：恢复 { 地点，姓名，部分 } => { 确保！（backuper.is_none（）， “备份或恢复正在进行中”）;如果 installer.is_some（） { app.report（app.state.lang.pick（INSTALL_UNFINISHED）， now）; continue; } let task = backup：：RestoreTask { 文件夹： backups_in（place）.join（&name）， parts， device： cfg.device.codename.clone（）， skipped： backup：:d ata_skipped（&cfg.backup.data_skipped）， detach： detach（cfg.services.clone（））， };backuper = Some（backup：：Worker：：start（backup：：Task：：Restore（task））？）;app.backup_started（真，地点，真）;} Effect：：FindBackups { storage， usb } => find_backups（&mut app， storage， usb， now）， Effect：:D eleteBackup { place， name } => {let deleted = delete_backup（place， &backups_in（place）.join（&name））.map_err（|e| format！（”{e：#}“））;queue.extend（app.backup_deleted（已删除，现已删除））;} Effect：：StopBackup => backuper.as_ref（）.context（“没有备份或恢复可以停止）？”。stop（）， Effect：：ReadUpdate => { let slot = info.slot.context（“只有 A/B 手机有合并更新”）？; // 合并后会显示其自身处于何处。if merger.is_none（） { app.set_update（skyward_vab：：standing（slot）？）; } // 另一个槽位的更新从该槽位开始，// 系统更新器可能未设置 yet. app.set_boot_slots（read_slots（））; }效果：：合并 => { 确保！（合并。is_none（）， “更新正在合并中”）;let slot = info.slot.context（“只有 A/B 手机合并更新”）？skyward_sys：：kmsg（“合并更新，正如更新页面要求的”）？;merger = Some（merge：：Merger：：start（slot， true）？）;} Effect：：Wlan（on） => wlan_worker（&wlan）？.send（if on { wlan：：Command：：On } 否则 { wlan：：Command：：Off }）， Effect：：ScanWlan => { scanned_at = Instant：：now（）; wlan_worker（&wlan）？.发送（wlan：：Command：：Scan）;} Effect：：JoinWlan { ssid， name， security， password } => { wlan_worker（&wlan）？.send（wlan：：Command：：Join { ssid， name， security， password： password.0 }） } Effect：：LeaveWlan => wlan_worker（&wlan）？.send（wlan：：Command：：Leave）， // 安装系统的名称是页面的 // 报告，当页面无法读取时。效果：：检查更新 => { ensure！（checker.is_none（）， “更新服务器已经被请求”）;让 server = cfg.update.as_ref（）.context（“手机的更新服务器未知”）？server.clone（）;让 probe = cfg.wlan.as_ref（）.context（“手机的 WLAN 未被调出”）？probe.clone（）;match installed（cfg， &info） { Ok（installed） => { checker = Some（updates：：Worker：：start（updates：：Task：：Check { server， probe， installed }， servers.clone（）？） }Err（e） => app.update_checked（Err（Unanswered：：Failed（format！（”{e：#}“）））， Have：：Nothing）， } } Effect：:D ownload（release） => { ensure！（downloader.is_none（）， “完整包正在下载中”）;让 server = cfg.update.as_ref（）.context（“手机的更新服务器未知”）？server.clone（）;设路径 = 下载（&释放）;match installed（cfg， &info） { Ok（installed） => { let task = updates：：Task：:D ownload { server， installed， release， path }; downloader = Some（updates：：Worker：：start（task， servers.clone（）？）; }Err（e） => app.download_ended（DownloadEnd：：Failed（Failure：：Other（format！（“{e：#}”）），have（&app， &release）， now）， } } Effect：：StopDownload => downloader.as_ref（）.context（“没有下载可停止”）？stop（）， // 无法保存的日志是要报告的页面;恢复工作仍在继续。Effect：：ExportLogs { recovery， kernel， storage } => { let into = match storage { true => Path：：new（skyward_crypto：:d ata：：SDCARD）.join（STORAGE_LOGS）， false => PathBuf：：from（TMP_LOGS）， }; match export_logs（recovery， kernel， &into， app.state.time_zone） { Ok（folder） => app.report（&skyward_ui：：fmt：：exported（app.state.lang， &folder.display（）.to_string（））， now）， Err（e） => app.report（&skyward_ui：fmt：：export_failed（app.state.lang， &format！（”{e：#}“）），now）， } } Effect：：SetMode { path， mode } => { let outresult = skyward_files：：set_mode（Path：：new（&path）， mode）.map_err（|e| format！（”{e：#}“））;app.mode_set（模式，结果，现在）;} 效果：：OpenText（path） => { let read = skyward_files：：read_up_to（路径：：new（&path）， skyward_ui：：editor：：MAX_BYTES）.map_err（|e| format！（”{e：#}“））;app.text_opened（及字体，路径，阅读，现在）;} // 文件夹再次列出，显示文件的新大小 // 和时间。Effect：：SaveText { path， text } => match skyward_files：：replace（Path：：new（&path）， text.as_bytes（）） { Ok（（）） => { app.text_saved（Ok（（））， now）; let folder = app.state.files.path.clone（）; list（&mut app， folder， &closed）？; }Err（e） => app.text_saved（Err（format！（”{e：#}“））， now）， }， Effect：：Rename { path， name } => match skyward_files：：rename（Path：：new（&path）， &name） { Ok（_） => { app.renamed（Ok（（））， now）; let folder = app.state.files.path.clone（）; list（&mut app， folder， &closed）？; }Err（e） 如果 e.is：：<skyward_files：：Taken>（） = > app.renamed（Err（RenameRefused：：Taken）， now）， Err（e） => app.renamed（Err（RenameRefused：：Failed（format！（“{e：#}”）），现在）， }， // 一个无法挂载或正在使用的分区是要报告的//页面;恢复工作仍在继续。效果：：坐骑（基础） => { let slot = skyward_vab：：current_slot（）？。上下文（“不是 A/B 电话”）？如果让 Err（e） = skyward_vab：：mount_one（槽位，底座） { app.mount_failed（MountFailure） { mounting： true， base， detail： format！（”{e：#}“） }）;} app.set_mounts（mounts（））;} 效果：：Unmount（base） => {let slot = skyward_vab：：current_slot（）？.上下文（“不是 A/B 电话”）？如果 Err（e） = skyward_vab：：unmount_one（槽位， &base） { app.mount_failed（MountFailure） { mounting： false， base， detail： format！（”{e：#}“） }）;} app.set_mounts（mounts（））;} // 当端口通过“无”时，init 停止快速启动;然后插槽的分区会显示为启动时。Effect：：EnterRecovery => { skyward_sys：：usb：：switch（skyward_sys：：usb：：resting（）， USB_TIMEOUT）？; let slot = skyward_vab：：current_slot（）？.上下文（“不是 A/B 电话”）？箱子：:p Repare（CFG，槽位）？;read_at = 瞬时：：now（）;info = skyward_info：：collect（cfg）？;fastboot = false;app.enter_recovery（）;} // 其他端口在返回休止时会进行文件传输 // 或保持文件传输。效果：：FileTransfer（on） => { let rests = ！fastboot && ！sideloading; match （on， transfer.take（）） { （true， None） = > match Transfer：：start（cfg， &info， app.state.time_zone） { Ok（t） => { transfer = Some（t）; 如果休止 { if let Err（e） = skyward_sys：：usb：：switch（skyward_sys：：usb：： ：resting（）， USB_TIMEOUT） { skyward_sys：：kmsg（&format！（”该移植版未进行文件传输：{e：#}“）？;app.report（&format！（”{e：#}“），现在）;} } } Err（e） => { skyward_sys：：kmsg（&format！（”文件传输未启动：{e：#}“）？;app.file_transfer_failed（false， &skyward_ui：：fmt：：transfer_failed（app.state.lang， &format！（“{e：#}”）），现在）;} }， （false， Some（t）） => { 如果让 Err（e） = t.stop（休止） { skyward_sys：：kmsg（&format！（”文件传输未顺利停止：{e：#}“）？;App.report（&skyward_ui：：fmt：：transfer_left_on（app.state.lang， &format！（”{e：#}“）），现在）;} } （开，t） => 无法接通！（“开关在{}状态下将文件传输{开}，t.is_some（））， } } // 内部存储在打开状态时会进入电脑，无论擦除或安装都无法移除。如果文件传输不跟进，或者服务已经结束，就会停止，页面会说明原因。let keep = match &mut transfer { Some（t） 如果 ！t.service.running（） => Err（anyhow！（”文件传输服务已结束“）），Some（t） => { let offer = app.state.data.storage_open（） & eraser.is_none（） & & installer.is_none（）; t.offer（offer， app.state.lang）.and_then（|（）|{ 如果 t.zone ！= app.state.time_zone { t.service.set_zone（skyward_ui：：fmt：：rules（app.state.time_zone）.clone（）？; t.zone = app.state.time_zone; }Ok（（）） }） } 无 => Ok（（））， };如果让 Err（e） = 保留 { // 终止服务自身的错误表示比其结尾多。让 why = match transfer.take（）.expect（“只有运行的文件传输失败”）.stop（！fastboot && ！sideloading） { Err（cause） => 原因，Ok（（）） = > e， }; skyward_sys：：kmsg（&format！（”文件传输停止：{why：#}“）？;app.file_transfer_failed（false， &skyward_ui：：fmt：：transfer_stopped（app.state.lang， &format！（“{why：#}”）），现在）;} } }

loading ...  加载中......
