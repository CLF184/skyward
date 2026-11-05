use crate::canvas::Canvas;
use anyhow::Result;
使用笼子：：canvas：：Canvas;无论如何：：结果;

/// A present target. The device backend scans out through KMS; the host
/// preview backend encodes PNGs. Keeping the renderer behind this trait is
/// what lets the whole UI be built and reviewed without the phone.
pub trait Surface {
    fn size(&self) -> (u32, u32);
    /// Hand `canvas` to the display. It returns once the frame before it is
    /// on the panel and this one is on its way, so the next frame is drawn
    /// while this one goes out.
    fn present(&mut self, canvas: &Canvas) -> Result<()>;
    /// Wait until the frame presented last is on the panel.
    fn settle(&mut self) -> Result<()>;
}
一个在场的目标。设备后端通过 KMS 进行扫描;主机 /// 预览后端编码 PNG 。保持渲染器支持这个特性，是 /// 让整个界面可以独立构建和审核的原因。pub trait Surface { fn size（&self） -> （u32， u32）; /// 将“画布”手放到显示屏上。它会在面板上////前一帧回来，而这帧正在路上，所以下一帧画///，而这帧则会消失。fn present（&mut self， canvas： &Canvas） -> 结果 <（）>; /// 等最后展示的帧出现在面板上。fn settle（&mut self） -> 结果 <（）>; }

/// Byte order of the scanout buffer against the canvas's RGBA.
#[cfg(any(target_os = "android", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Order {
    Rgba,
    Bgra,
}
扫描缓冲区与画布 RGBA 的字节顺序。#[cfg（any（target_os = “android”， test））] #[derive（Clone， Copy， Debug， PartialEq， Eq）] pub（crate） enum Order { Rgba， Bgra， }

/// The order the canvas's red, green, blue and alpha have to be written in
/// for a scanout buffer of `fourcc`.
///
/// DRM names a format by its bits from the most significant down, so a
/// little-endian machine lays the bytes out the other way: `Xrgb8888` is
/// `[B, G, R, X]` in memory, and the canvas's `[R, G, B, A]` reaches it only
/// by swapping the two end bytes. `Xbgr8888` is `[R, G, B, X]`, and what the
/// canvas holds goes straight in.
///
/// The high byte is dropped by the display controller either way: a mode
/// that reads `X` ignores it, and one that reads `A` takes the canvas's
/// opaque 255, which is what these pages are.
#[cfg(any(target_os = "android", test))]
pub(crate) fn order_of(fourcc: u32) -> Order {
    // The fourcc codes the `drm` crate carries, named here so that the choice
    // is made and tested without it. Each spells its bytes the way the format
    // is written: 'XB24' is 0x34324258.
    const XB24: u32 = 0x3432_4258; // Xbgr8888: [R, G, B, X] in memory
    const AB24: u32 = 0x3432_4241; // Abgr8888: [R, G, B, A]
    const XR24: u32 = 0x3432_5258; // Xrgb8888: [B, G, R, X]
    const AR24: u32 = 0x3432_5241; // Argb8888: [B, G, R, A]
    match fourcc {
        XB24 | AB24 => Order::Rgba,
        XR24 | AR24 => Order::Bgra,
        // A format this does not know is written as the one the canvas's
        // bytes reach by turning them round, which is what the panel shows
        // most often; a buffer of another format is a fault of its own.
        _ => Order::Bgra,
    }
}
扫描缓冲区的“fourcc”需要用 /// 写入画布的红、绿、蓝和 alpha 顺序。/// DRM 按格式从高到低的位命名格式，因此小端机将字节排成相反方向：'Xrgb8888'在内存中表示/// 'B， G， R， X]'，画布的'[R， G， B， A]'仅通过交换两个端字节到达///。'Xbgr8888' 是 '[R， G， B， X]'，/// 画布里的 /// 字体直接放进去。/// 显示控制器无论如何都会丢弃高字节：读取“X”的///模式忽略它，读取“A”的模式则接收画布的不透明 255，这正是这些页面的定义。#[cfg（any（target_os = “android”，测试））] pub（crate） fn order_of（fourcc： u32） -> 顺序 { // fourcc 是“drm”箱子携带的 fourcc 代码，此处命名是为了让选择 // 在没有它的情况下做出和测试。每个字节的拼写方式都像格式 // 一样写：“XB24”是 0x34324258。变数 XB24：u32 = 0x3432_4258;Xbgr8888： [R， G， B， X] 内存条件 AB24： u32 = 0x3432_4241;Abgr8888： [R， G， B， A] 构件 XR24： u32 = 0x3432_5258;Xrgb8888： [B， G， R， X] const AR24： u32 = 0x3432_5241;Argb8888： [B， G， R， A] 比赛 fourcc { XB24 |AB24 => 顺序：：Rgba， XR24 |AR24 => 顺序：：Bgra， // 它不知道的格式被写成画布的//字节通过旋转它们所到达的格式，面板中最常显示的//;另一种格式的缓冲区本身就是一个故障。_ => 顺序：：Bgra， } }

/// The 32-bit RGB formats a scanout buffer may be made in, the ones whose
/// pixels the canvas reaches by turning nothing or two bytes round.
///
/// `XB24` first: it is the order the canvas already holds its bytes in, and
/// what a haotian's own recovery drives its panel with.
#[cfg(any(target_os = "android", test))]
pub(crate) const SCANOUT: [u32; 4] = [0x3432_4258, 0x3432_4241, 0x3432_5258, 0x3432_5241];
扫描缓冲区可以使用 32 位 RGB 格式，即画布通过将“无”或“两个字节”旋转后达到的///像素。/// 先说“XB24”：它是画布已经存储字节的顺序，以及昊天自身恢复驱动面板的顺序。#[cfg（any（target_os = “android”， test））] pub（crate） const SCANOUT： [u32; 4] = [0x3432_4258， 0x3432_4241， 0x3432_5258， 0x3432_5241];

/// How Qualcomm's display driver drives a mode, from the text of the
/// connector's mode_properties blob: key=value lines, a block per mode_name.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, PartialEq, Eq)]
struct SdeMode {
    /// Layer mixers in the mode's topology, each blending one vertical
    /// strip of the screen.
    mixers: u32,
    dsc: bool,
    /// The DSC rate this mode is made for, as the connector's `bpp_mode`
    /// property numbers it: 0 for 8 bits a pixel, 1 for 10, 2 for 12.
    ///
    /// A mode driven with a rate other than its own is decompressed with the
    /// wrong arithmetic, and the panel shows the picture with its channels
    /// mangled rather than its geometry wrong.
    bpp: Option<u64>,
}
高通显示驱动如何驱动模式，从///连接器 mode_properties 块文字中看：key=值行，每 mode_name 一个区块。#[cfg（any（target_os = “android”， test））] #[derive（Debug， PartialEq， Eq）] struct SdeMode { /// 在模式拓扑中叠加混合器，每个混合器混合一条垂直的 /// 屏幕条。混频器：U32，DSC：bool，/// 该模式的 DSC 速率，因为连接器的“bpp_mode”属性编号：每像素 8 位为 0,10 位为 1,12 位为 2。/// // 以非自身速率驱动的模式会用错误的运算解压， 画面显示的是通道///错乱，而不是几何形状错误。BPP：选项 <u64>，}

#[cfg(any(target_os = "android", test))]
impl SdeMode {
    fn find(text: &str, name: &str) -> Result<SdeMode> {
        let mut current = false;
        let (mut topology, mut bpp) = (None, None);
        let mut dsc = false;
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            if key == "mode_name" {
                if current {
                    break;
                }
                current = value == name;
            } else if current {
                match key {
                    "topology" => topology = Some(value),
                    "dsc_mode" => dsc = value == "1",
                    "bpp_mode" => bpp = value.parse().ok(),
                    _ => {}
                }
            }
        }
        let topology = topology.ok_or_else(|| anyhow::anyhow!("mode_properties describes no topology for {name}"))?;
        let mixers = if topology.contains("quadpipe") {
            4
        } else if topology.contains("dualpipe") {
            2
        } else {
            1
        };
        Ok(SdeMode { mixers, dsc, bpp })
    }
}
#[cfg（any（target_os = “android”， test））] impl SdeMode { fn find（text： &str， name： &str） -> 结果 <SdeMode> { let mut current = false; let （mut topology， mut bpp） = （None， None）; let mut dsc = false;对于 text.lines（） { let Some（（key， value）） = line.split_once（'='） 否则 { 继续 }; if key == “mode_name” { 如果当前 { break; } current = value == name; } 否则如果当前 { match key { “topology” => topology = Some（value）， “dsc_mode” => dsc = value == “1”， “bpp_mode” => bpp = value.parse（）.ok（）， _ => {} } } } 让拓扑 = topology.ok_or_else（|| anyhow：：anyhow！（”mode_properties 描述了 {name} 的无拓扑结构）？;设 mixers = 如果 topology.contains（“quadpipe”） { 4 } 否则 如果 topology.contains（“dualpipe”） { 2 } 否则 { 1 };Ok（SdeMode { mixers， dsc， bpp }） } } }

#[cfg(test)]
mod tests {
    use super::*;
#[cfg（test）] mod tests { use super：:*;

    const ISHTAR: &str = "mode_name=1440x3200x60cmd\nbit_clk_rate=1360000000\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n\
                          mode_name=1440x3200x120cmd\nsubmode_idx=1\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n\
                          mode_name=1080x2400x60cmd\ndsc_mode=0\ntopology=sde_singlepipe\n";
const ISHTAR： &str = “mode_name=1440x3200x60cmd\nbit_clk_rate=1360000000\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n\ mode_name=1440x3200x120cmd\nsubmode_idx=1\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n\ mode_name=1080x2400x60cmd\ndsc_mode=0\ntopology=sde_singlepipe\n”;

    #[test]
    fn reads_the_topology_of_the_named_mode() {
        assert_eq!(SdeMode::find(ISHTAR, "1440x3200x120cmd").unwrap(), SdeMode { mixers: 2, dsc: true, bpp: Some(2) });
        assert_eq!(SdeMode::find(ISHTAR, "1080x2400x60cmd").unwrap(), SdeMode { mixers: 1, dsc: false, bpp: None });
        assert!(SdeMode::find(ISHTAR, "1440x3200x90cmd").is_err());
    }
#[测试] fn reads_the_topology_of_the_named_mode（） { assert_eq！（SdeMode：：find（ISHTAR， “1440x3200x120cmd”）.unwrap（）， SdeMode { mixers： 2， dsc： true， bpp： Some（2） }）;assert_eq！（SdeMode：：find（ISHTAR， “1080x2400x60cmd”）.unwrap（）， SdeMode { mixers： 1， dsc： false， bpp： None }）;断言！（SdeMode：：find（ISHTAR，“1440x3200x90cmd”）.is_err（））;}

    #[test]
    fn the_rate_a_mode_is_compressed_at_is_read_beside_the_rest() {
        // A haotian's own mode: two pipes, DSC on, compressed at 12 bits a
        // pixel. Driven at 8 instead, the panel shows the picture with its
        // colours mangled -- which is what this phone did until the rate was
        // set from here.
        let haotian = "mode_name=1440x3200x120cmd\nsubmode_idx=1\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n";
        let mode = SdeMode::find(haotian, "1440x3200x120cmd").unwrap();
        assert_eq!(mode.bpp, Some(2), "12 bits a pixel");
        assert!(mode.dsc && mode.mixers == 2);
        // A mode that names no rate leaves it alone rather than guessing.
        let bare = "mode_name=a\nbpp_mode=x\ndsc_mode=1\ntopology=sde_singlepipe\n";
        assert_eq!(SdeMode::find(bare, "a").unwrap().bpp, None);
    }
#[测试] fn the_rate_a_mode_is_compressed_at_is_read_beside_the_rest（） { // 昊天自有模式：两根管道，DSC 开启，以 12 位//像素压缩。改为 8 点行驶，面板显示照片颜色被扭曲——这部手机也是这样，直到//从这里开始设定速率。设昊天=“mode_name=1440x3200x120cmd\nsubmode_idx=1\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n”;let mode = SdeMode：：find（haotian， “1440x3200x120cmd”）.unwrap（）;assert_eq！（mode.bpp，Some（2），“12 位每像素”）;断言！（mode.dsc && mode.mixers == 2）;一个不标注利率的模式让它保持原样，而不是盲目猜测。令 bare = “mode_name=A\nbpp_mode=x\ndsc_mode=1\ntopology=sde_singlepipe\n”;assert_eq！（SdeMode：：find（bare， “a”）.unwrap（）.bpp， None）;}

    #[test]
    fn a_format_whose_bytes_name_red_first_takes_the_canvas_as_it_is() {
        // 'XB24' and 'AB24' lay their bytes out [R, G, B, x] on a machine of
        // this kind's byte order, which is the canvas's own order.
        assert_eq!(order_of(0x3432_4258), Order::Rgba, "Xbgr8888");
        assert_eq!(order_of(0x3432_4241), Order::Rgba, "Abgr8888");
    }
#[测试] fn a_format_whose_bytes_name_red_first_takes_the_canvas_as_it_is（） { // 'XB24' 和 'AB24' 在一台按 // 这种字节顺序排列的机器上排列它们的字节 [R， G， B， X]，这也是画布本身的顺序。assert_eq！（order_of（0x3432_4258），顺序：：rgba，“Xbgr8888”）;assert_eq！（order_of（0x3432_4241），顺序：：Rgba，“Abgr8888”;}

    #[test]
    fn a_format_whose_bytes_name_blue_first_has_its_ends_swapped() {
        // 'XR24' and 'AR24' lay theirs out [B, G, R, x]: red and blue reach
        // the panel the wrong way round unless the canvas's are swapped.
        assert_eq!(order_of(0x3432_5258), Order::Bgra, "Xrgb8888");
        assert_eq!(order_of(0x3432_5241), Order::Bgra, "Argb8888");
    }
#[测试] fn a_format_whose_bytes_name_blue_first_has_its_ends_swapped（） { // 'XR24' 和 'AR24' 分别铺设了 [B， G， R， X]： 红蓝会反向覆盖面板 // 除非画布互换。assert_eq！（order_of（0x3432_5258），目：：Bgra，“Xrgb8888”）;assert_eq！（order_of（0x3432_5241），顺序：：Bgra，“Argb8888”）;}

    #[test]
    fn every_format_offered_for_scanout_is_one_the_panel_can_be_handed() {
        // What the pick offers has to be exactly what the order is known for:
        // a format offered but not known would be presented with its red and
        // blue the wrong way round.
        for f in SCANOUT {
            assert!(matches!(order_of(f), Order::Rgba | Order::Bgra), "0x{f:08x}");
            assert!(f == 0x3432_4258 || f == 0x3432_4241 || f == 0x3432_5258 || f == 0x3432_5241, "0x{f:08x}");
        }
        assert_eq!(SCANOUT.len(), 4, "the four 32-bit RGB formats the canvas can reach");
    }
}
#[测试] fn every_format_offered_for_scanout_is_one_the_panel_can_be_handed（） { // 选号所提供的必须完全符合该顺序所知的特点：// 一个被提供但未知的格式会被反对地呈现其红色和蓝色。对于 SCANOUT 中的 f { assert！（火柴！（order_of（f）， 顺序：：Rgba |顺序：：Bgra），“0x{f：08x}”）;断言！（f == 0x3432_4258 || f == 0x3432_4241 || f == 0x3432_5258 || f == 0x3432_5241，“0x{f：08x}”）;} assert_eq！（SCANOUT.len（）， 4，“画布可达的四种 32 位 RGB 格式”）;} }

#[cfg(target_os = "android")]
pub mod kms {
    use super::*;
    use anyhow::{anyhow, Context};
    use drm::buffer::{self, DrmFourcc, DrmModifier, PlanarBuffer};
    use drm::control::dumbbuffer::DumbBuffer;
    use drm::control::{
        atomic, connector, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice, Event, FbCmd2Flags, ModeTypeFlags, PlaneType,
        ResourceHandle,
    };
    use drm::Device;
    use std::collections::HashMap;
    use std::fs::{File, OpenOptions};
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
    use std::path::Path;
    use std::time::{Duration, Instant};
#[cfg（target_os = “android”）] pub mod kms { use super：:*; use anyhow：：{anyhow， Context}; 使用 drm：：buffer：：{self， DrmFourcc， DrmModifier， PlanarBuffer}; 使用 drm：：control：:d umbbuffer：:D umbBuffer; 使用 drm：：control：：{ atomic， connector， framebuffer， plane， property， AtomicCommitFlags， Device as ControlDevice， Event， FbCmd2Flags， ModeTypeFlags， PlaneType， ResourceHandle， }; 使用 drm：:D evice;使用 std：：collections：：HashMap;使用 std：：fs：：{File， OpenOptions};使用 std：：os：：fd：：{AsFd， AsRawFd， BorrowedFd};使用标准：:p ath：:P ath;使用 std：：time：：{持续时间，瞬间};

    /// How long the display may take to complete a commit.
    const COMMIT_TIMEOUT: Duration = Duration::from_secs(2);
显示完成提交所需的时间。cont COMMIT_TIMEOUT：时长 = 时长：：from_secs（2）;

    struct Card(File);  struct Card（文件）;

    impl AsFd for Card {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.0.as_fd()
        }
    }
impl AsFd for Card { fn as_fd（&self） -> BorrowedFd<'_> { self.0.as_fd（） } }

    impl Device for Card {}
    impl ControlDevice for Card {}

    /// A single-plane dumb buffer presented to ADDFB2, so the framebuffer
    /// keeps the buffer's own pixel format.
    struct Planar<'a>(&'a DumbBuffer);
一个单平面的哑缓冲区呈现给 ADDFB2，因此帧缓冲区 /// 保留缓冲区自身的像素格式。结构平面（Planar<a>（和一个 DumbBuffer）;

    impl PlanarBuffer for Planar<'_> {
        fn size(&self) -> (u32, u32) {
            buffer::Buffer::size(self.0)
        }
        fn format(&self) -> DrmFourcc {
            buffer::Buffer::format(self.0)
        }
        fn modifier(&self) -> Option<DrmModifier> {
            None
        }
        fn pitches(&self) -> [u32; 4] {
            [buffer::Buffer::pitch(self.0), 0, 0, 0]
        }
        fn handles(&self) -> [Option<buffer::Handle>; 4] {
            [Some(buffer::Buffer::handle(self.0)), None, None, None]
        }
        fn offsets(&self) -> [u32; 4] {
            [0; 4]
        }
    }
impl PlanarBuffer for Planar<'_> { fn size（&self） -> （u32， u32） { buffer：： Buffer：：size（self.0） } fn format（&self） -> DrmFourcc { buffer：：Buffer：：format（self.0） } fn modifier（&self） -> Option<DrmModifier> { None } fn pitches（&self） -> [u32; 4] { [buffer：：Buffer：:p itch（self.0）， 0， 0， 0] } fn handles（&self） -> [Option<buffer：：Handle>; 4] { [Some（buffer：：Buffer：Buffer：handle（self.0））， None， None， None] } fn offsets（&self） -> [u32; 4] { [0; 4] }

    struct Buf {
        db: DumbBuffer,
        fb: framebuffer::Handle,
        /// The buffer mapped for writing for as long as the surface lives;
        /// mapping it per frame costs a page fault per page on every frame.
        map: *mut u8,
        len: usize,
    }
struct Buf { db： DumbBuffer， fb： framebuffer：：Handle， /// 用于写入的缓冲区映射时间为表面存在;/// 每帧映射每帧每页需发生一次页面错误。映射： *MUT U8， len： usize， }

    /// A KMS object's properties by name, with their current values.
    struct Props(HashMap<String, (property::Handle, u64)>);
KMS 对象的属性按名称和当前值显示。struct Props（HashMap<String， （property：：Handle， u64）>）;

    impl Props {
        fn read<T: ResourceHandle>(card: &Card, handle: T) -> Result<Self> {
            let values = card.get_properties(handle).context("read properties")?;
            let mut map = HashMap::new();
            for (&id, &value) in values.iter() {
                let info = card.get_property(id).context("read property")?;
                map.insert(info.name().to_string_lossy().into_owned(), (id, value));
            }
            Ok(Props(map))
        }
impl Props { fn read<T： ResourceHandle>（card： &Card， handle： T） -> Result<Self> { let values = card.get_properties（handle）.context（“read properties”）？; let mut map = HashMap：：new（）; for （&id， &value） in values.iter（） { let info = card.get_property（id）.context（“read property”）？; map.insert（info.name（）.to_string_lossy（）.into_owned）， （id， value））; }Ok（Props（map）） }

        fn get(&self, name: &str) -> Result<(property::Handle, u64)> {
            self.0.get(name).copied().ok_or_else(|| anyhow!("KMS property {name} missing"))
        }
fn get（&self， name： &str） -> Result<（property：：Handle， u64）> { self.0.get（name）.copied（）.ok_or_else（|| anyhow！（”KMS 属性 {name} 缺失“）） }

        fn handle(&self, name: &str) -> Result<property::Handle> {
            Ok(self.get(name)?.0)
        }
fn handle（&self， name： &str） -> Result<property：：Handle> { Ok（self.get（name）？.0) }

        fn value(&self, name: &str) -> Result<u64> {
            Ok(self.get(name)?.1)
        }
fn value（&self， name： &str） -> 结果 <u64> { Ok（self.get（name）？.1) }

        fn line(&self) -> String {
            let mut parts: Vec<String> = self.0.iter().map(|(name, (_, value))| format!("{name}={value}")).collect();
            parts.sort();
            parts.join(" ")
        }
    }
fn line（&self） -> 字符串 { 令 mut 部分： Vec<String> = self.0.iter（）.map（|（name， （_， value）） |格式！（“{name}={value}”））。collect（）;parts.sort（）;parts.join（“ ”） } }

    fn open_card(path: &str) -> Result<Card> {
        let file = OpenOptions::new().read(true).write(true).open(path).with_context(|| format!("open {path}"))?;
        let card = Card(file);
        card.set_client_capability(drm::ClientCapability::UniversalPlanes, true).context("enable universal planes")?;
        card.set_client_capability(drm::ClientCapability::Atomic, true).context("enable atomic modesetting")?;
        Ok(card)
    }
fn open_card（path： &str） -> 结果 <Card> { let file = OpenOptions：：new（）.read（true）.write（true）.open（path）.with_context（|| format！（”开启 {path}“）？;令 card = Card（文件）;card.set_client_capability（drm：：ClientCapability：：UniversalPlanes， true）.context（“enable universal planes”）？;card.set_client_capability（drm：：ClientCapability：：Atomic， true）.context（“enable atomic modesetting”）？;Ok（card） }

    /// The value of the choice named `choice` of an enum property.
    fn enum_value(card: &Card, id: property::Handle, choice: &str) -> Result<u64> {
        let info = card.get_property(id).context("read property")?;
        match info.value_type() {
            property::ValueType::Enum(e) => e.values().1.iter().find(|v| v.name().to_str() == Ok(choice)).map(|v| v.value()),
            _ => None,
        }
        .ok_or_else(|| anyhow!("{} offers no {choice}", info.name().to_string_lossy()))
    }
是枚举属性中名为“选择”的选项的值。fn enum_value（card： &Card， id： property：：Handle， choice： &str） -> 结果 <u64> { let info = card.get_property（id）.context（“read property”）？; match info.value_type（） { property：：ValueType：：Enum（e） => e.values（）.1.iter（）.find（|v| v.name（）.to_str（） == Ok（choice））.map（|v| v.value（））， _ => 无， } .ok_or_else（|| anyhow！（”{} 不提供 {choice}“， info.name（）.to_string_lossy（））） }

    /// The choice a plane's pixels are to be read as, and the property it
    /// belongs to.
    ///
    /// A panel driven over DSI is usually described to the display controller
    /// as YCbCr in the compressed sense -- the controller converts, the panel
    /// decodes -- and a plane left at the driver's own default hands the
    /// pixels over unconverted. What this phone's own recovery asks for is
    /// BT.601 YCbCr, which is also what the kernel puts on a plane nobody has
    /// spoken to, so that is what is asked for here.
    fn colour_encoding(card: &Card, props: &Props) -> Option<(property::Handle, u64)> {
        let (id, _) = props.get("COLOR_ENCODING").ok()?;
        let choice = enum_choice(card, id, &["ITU-R BT.601 YCbCr", "BT601_YCC", "yuv444_8bit"], 0).ok()?;
        Some((id, choice))
    }
平面像素的选择，以及它所属的属性 /// 。/// 通过 DSI 驱动的面板通常在压缩意义上被显示控制器描述为 YCbCr——控制器进行转换，面板///解码——而驱动程序默认操作的平面则会获得未转换的///像素。这部手机的恢复要求是 /// BT.601 YCbCr，内核也会把这个数据放到一个没人和 /// 通话的平面上，所以这里要求的就是这个。fn colour_encoding（card： &Card， props： &Props） -> Option<（property：：Handle， u64）> { let （id， _） = props.get（“COLOR_ENCODING”）.ok（）？; let choice = enum_choice（card， id， &[“ITU-R BT.601 YCbCr”， “BT601_YCC”， “yuv444_8bit”]， 0）.ok（）？;Some（（id， choice）） }

    /// The choice of range a plane's pixels are to be read in, and the
    /// property it belongs to: the limited one, as Android's own recovery
    /// asks for, which is what a video range means to a panel.
    fn colour_range(card: &Card, props: &Props) -> Option<(property::Handle, u64)> {
        let (id, _) = props.get("COLOR_RANGE").ok()?;
        let choice = enum_choice(card, id, &["YCbCr limited range", "Limited", "limited"], 1).ok()?;
        Some((id, choice))
    }
飞机像素的选择范围，以及它所属的///属性：有限属性，正如 Android 自身的恢复///所要求的，这对面板来说就是视频范围的含义。fn colour_range（card： &Card， props： &Props） -> Option<（property：：Handle， u64）> { let （id， _） = props.get（“COLOR_RANGE”）.ok（）？; let choice = enum_choice（card， id， &[“YCbCr limited range”， “Limited”， “limited”]， 1）.ok（）？;Some（（id， choice）） }

    /// The choice an enum property names `choice`, or the one sitting at
    /// `index` among its choices where no choice is named that.
    ///
    /// Qualcomm's `bpp_mode` is an enumeration whose choices are numbered the
    /// same way the mode description numbers its rate -- 0 for 8 bits a
    /// pixel, 1 for 10, 2 for 12 -- but the names it gives them are the
    /// driver's business. Looking the number up among the values covers both
    /// a driver that names them and one that only numbers them.
    fn enum_choice(card: &Card, id: property::Handle, choices: &[&str], index: u64) -> Result<u64> {
        for choice in choices {
            if let Ok(v) = enum_value(card, id, choice) {
                return Ok(v);
            }
        }
        let info = card.get_property(id).context("read property")?;
        if let property::ValueType::Enum(e) = info.value_type() {
            let (_, values) = e.values();
            if let Some(v) = values.iter().find(|v| v.value() == index) {
                return Ok(v.value());
            }
            // The enumeration is numbered from zero in the order it lists
            // them where the values themselves are not the index.
            if let Some(v) = values.get(index as usize) {
                return Ok(v.value());
            }
        }
        anyhow::bail!("{} offers nothing at {index}", info.name().to_string_lossy())
    }
枚举属性中的选择称为“选择”，或者在其选项中位于 /// “索引”的选项中没有被命名的选项。/// 高通的“bpp_mode”是一个枚举，其选择编号与模式描述中速率相同——0 代表 8 位///像素，1 代表 10 位，2 代表 12 位——但它给它们命名的由///驱动自行决定。查找数值中的数字既包括一个给它们命名的驱动，也包括一个只编号它们的驱动。fn enum_choice（card： &Card， id： property：：Handle， choices： &[&str]， index： u64） -> Result<u64> { 用于选择中的选择 { if let Ok（v） = enum_value（card， id， choice） { return Ok（v）; } } let info = card.get_property（id）.context（“read property”）？; if let property：：ValueType：：Enum（e） = info.value_type（） { let （_， values） = e.values（）; if let Some（v） = values.iter（）.find（|v| v.value（） == index） { return Ok（v.value（））; } // 枚举从零开始编号，顺序如下 // 其中数值本身不是指数。如果 Some（v） = values.get（索引为 usize） { 返回 Ok（v.value（））; } }总之：：跳票！（“{} 在 {index} 处没有提供任何信息”，info.name（）.to_string_lossy（）） }

    /// Built-in panel interfaces. Writeback and external outputs are never
    /// the recovery's screen.
    fn is_panel(interface: connector::Interface) -> bool {
        matches!(interface, connector::Interface::DSI | connector::Interface::EmbeddedDisplayPort | connector::Interface::LVDS)
    }
内置面板接口。写回和外部输出从来不会出现在恢复界面。fn is_panel（interface： connector：：Interface） -> bool { matches！（接口，连接器：：接口：:D SI |connector：：Interface：：EmbeddedDisplayPort |connector：：Interface：：LVDS） }

    /// Commit without blocking and wait for the flip event, so a commit the
    /// display never completes surfaces as an error instead of an endless
    /// wait.
    fn commit(card: &Card, req: atomic::AtomicModeReq, flags: AtomicCommitFlags) -> Result<()> {
        card.atomic_commit(flags | AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT, req).context("atomic commit")?;
        flipped(card)
    }
提交时不阻塞，等待翻转事件，这样提交///显示时不会以错误形式完成表面，而不是无休止地等待///。fn commit（card： &Card， req： atomic：：AtomicModeReq， flags： AtomicCommitFlags） -> Result<（）> { card.atomic_commit（flags |AtomicCommitFlags：：NONBLOCK |AtomicCommitFlags：:P AGE_FLIP_EVENT， req）.context（“atomic commit”）？;flipped（card） }

    /// Wait for the flip event of the commit made last, or fail once the
    /// display has kept it waiting too long.
    fn flipped(card: &Card) -> Result<()> {
        let deadline = Instant::now() + COMMIT_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let mut pfd = libc::pollfd { fd: card.as_fd().as_raw_fd(), events: libc::POLLIN, revents: 0 };
            let n = unsafe { libc::poll(&mut pfd, 1, left.as_millis() as i32) };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err).context("wait for the display");
            }
            if n == 0 {
                return Err(anyhow!("the display did not complete the commit within {} ms", COMMIT_TIMEOUT.as_millis()));
            }
            for event in card.receive_events().context("read display events")? {
                if let Event::PageFlip(_) = event {
                    return Ok(());
                }
            }
        }
    }
等待最后提交的翻转事件，或者当 /// 显示让它等待过久时失败。fn flipped（card： &Card） -> Result<（）> {let deadline = Instant：：now（） + COMMIT_TIMEOUT; loop { let left = deadline.saturating_duration_since（Instant：：now（））; let mut pfd = libc：:p ollfd { fd： card.as_fd（）.as_raw_fd（）， events： libc：:P OLLIN， revents： 0 }; let n = unsafe { libc：:p oll（&mut pfd， 1， left.as_millis（） as i32） }; if n < 0 { let err = std：：io：：Error：：last_os_error（）; if err.kind（） == std：：io：：ErrorKind：：Interrupted { continue; }返回 Err（err）.context（“等待显示”）;} 如果 n == 0 { return Err（无论如何！（”显示未在 {} ms“， COMMIT_TIMEOUT.as_millis（））内完成提交）;} 用于 card.receive_events（）.context（“read display events”）中的事件？{ 如果让 Event：:P ageFlip（_） = 事件 { 返回 Ok（（））; } }} }

    /// Connectors with their modes and properties, encoders with the CRTC
    /// they feed, CRTCs with their mode and properties, and the planes in
    /// use: what a device has for scanout and how it is wired right now.
    pub fn report(path: &str) -> Result<String> {
        use std::fmt::Write;
        let card = open_card(path)?;
        let res = card.resource_handles().context("read KMS resources")?;
        let mut out = String::new();
        for &h in res.connectors() {
            let c = card.get_connector(h, true).context("read connector")?;
            let encoder = c.current_encoder().map(u32::from);
            writeln!(out, "connector {} {:?} {:?} encoder {encoder:?} {} modes", u32::from(h), c.interface(), c.state(), c.modes().len())?;
            let props = Props::read(&card, h)?;
            writeln!(out, "  {}", props.line())?;
            if is_panel(c.interface()) {
                if let Ok((_, blob)) = props.get("mode_properties") {
                    let text = card.get_property_blob(blob).context("read mode_properties")?;
                    writeln!(out, "  mode_properties: {}", String::from_utf8_lossy(&text).replace('\n', " | "))?;
                }
                for m in c.modes() {
                    let (w, hgt) = m.size();
                    writeln!(out, "  mode {:?} {w}x{hgt} {} Hz clock {} type {:?} flags {:?}", m.name(), m.vrefresh(), m.clock(), m.mode_type(), m.flags())?;
                }
            }
        }
        for &h in res.encoders() {
            let e = card.get_encoder(h).context("read encoder")?;
            let possible: Vec<u32> = res.filter_crtcs(e.possible_crtcs()).into_iter().map(u32::from).collect();
            writeln!(out, "encoder {} {:?} crtc {:?} possible {possible:?}", u32::from(h), e.kind(), e.crtc().map(u32::from))?;
        }
        for &h in res.crtcs() {
            let i = card.get_crtc(h).context("read CRTC")?;
            let mode = i.mode().map(|m| format!("{:?} {} Hz", m.name(), m.vrefresh()));
            writeln!(out, "crtc {} mode {mode:?} fb {:?}", u32::from(h), i.framebuffer().map(u32::from))?;
            writeln!(out, "  {}", Props::read(&card, h)?.line())?;
        }
        for p in card.plane_handles().context("list planes")? {
            let i = card.get_plane(p).context("read plane")?;
            let props = Props::read(&card, p)?;
            writeln!(
                out,
                "plane {} type {} crtc {:?} fb {:?} possible {} formats {}",
                u32::from(p),
                props.value("type")?,
                i.crtc().map(u32::from),
                i.framebuffer().map(u32::from),
                res.filter_crtcs(i.possible_crtcs()).len(),
                i.formats().len()
            )?;
            if i.crtc().is_some() {
                writeln!(out, "  {}", props.line())?;
            }
        }
        Ok(out)
    }
 连接器及其模式和特性，编码器与它们馈入的 CRTC ///，CRTC 及其模式和属性，以及///中的平面用途：设备扫描的设备以及当前的布线方式。pub fn report（path： &str） -> Result<String> { use std：：fmt：：Write; let card = open_card（path）？; let res = card.resource_handles（）.context（“read KMS resources”）？; let mut out = String：：new（）; for &h in res.connectors（） { let c = card.get_connector（h， true）.context（“read connector”）？; let encoder = c.current_encoder（）.map（u32：：from）; writeln！（out， “connector {} {：？}{:?}Encoder {encoder：？}{} modes“， u32：：from（h）， c.interface（）， c.state（）， c.modes（）.len（）？;令 props = Props：：read（&card， h）？;写吧！（出，“{}”，props.line（）？;如果 is_panel（c.interface（）） { 如果 let Ok（（_， blob）） = props.get（“mode_properties”） { let text = card.get_property_blob（blob）.context（“read mode_properties”）？; writeln！（out， “mode_properties： {}”， String：：from_utf8_lossy（&text）.replace（'\n'， “ | ”）？;} 对于 m 在 c.modes（） { 设 （w， hgt） = m.size（）; writeln！（“模式 {：？}{w}x{hgt} {} Hz clock {} type {：？} flags {：？}“， m.name（）， m.vrefresh（）， m.clock（）， m.mode_type（）， m.flags（）？;} } } 对于 &h 在 res.encoders（） { let e = card.get_encoder（h）.context（“read encoder”）？; 允许：Vec<u32> = res.filter_crtcs（e.possible_crtcs（）.into_iter）.map（u32：：from）.collect（）; writeln！（out， “Encoder {} {：？}CRTC {：？} 可能 {可能：？}“， u32：：from（h）， e.kind（）， e.crtc（）.map（u32：：from）？;} 对于 & h，在 res.crtcs（） { let i = card.get_crtc（h）.context（“read CRTC”）？; 令 mode = i.mode（）.map（|m| format！（”{:?}{} Hz“， m.name（）， m.vrefresh（））;写吧！（out， “CRTC {} mode {mode：？}fb {：？}“， u32：：from（h）， i.framebuffer（）.map（u32：：from））？;写吧！（出，“{}”，Props：：read（&card， h）？.line（）？;} 对于 p 在 card.plane_handles（）.context（“列表平面”）？{ 令 i =  card.get_plane（p）.context（“read plane”）？; let props = Props：：read（&card， p）？; writeln！（ out，“平面 {} 类型 {} CRTC {：？}FB {：？} 可能的{} formats {}“， u32：：from（p）， props.value（”type“）？， i.crtc（）.map（u32：：from）， i.framebuffer（）.map（u32：：from）， res.filter_crtcs（i.possible_crtcs（））.len（）， i.formats（）.len（） ）？;如果 i.crtc（）.is_some（） { writeln！（结束，“{}”，props.line（）？;} } 好的（出） }

    /// The panel connector's CRTC, from the encoder driving it.
    fn panel_crtc(card: &Card) -> Result<drm::control::crtc::Handle> {
        let res = card.resource_handles().context("read KMS resources")?;
        for &h in res.connectors() {
            let info = card.get_connector(h, false).context("read connector")?;
            if is_panel(info.interface()) && info.state() == connector::State::Connected {
                let encoder = info.current_encoder().ok_or_else(|| anyhow!("the panel has no encoder; is anything showing?"))?;
                return card.get_encoder(encoder).context("read encoder")?.crtc().ok_or_else(|| anyhow!("the panel's encoder drives no CRTC"));
            }
        }
        Err(anyhow!("no connected built-in panel"))
    }
面板连接器是 CRTC，来自驱动它的编码器。fn panel_crtc（card： &Card） -> 结果 <drm：：control：：crtc：：Handle> { let res = card.resource_handles（）.context（“read KMS resources”）？; for &h in res.connectors（） { let info = card.get_connector（h， false）.context（“read connector”）？; if is_panel（info.interface（）） && info.state（） == connector：：State：：Connected { let encoder = info.current_encoder（）.ok_or_else（|| anyway！（”该面板没有编码器;有什么显示出来吗？“））？;return card.get_encoder（encoder）.context（“read encoder”）？.CRTC（）.ok_or_else（|| anyway！（”面板的编码器不驱动 CRTC）））。} } 呃（总之！（”没有连接的内置面板）}

    /// The framebuffer the panel scans out right now, as tightly packed RGBA
    /// rows: what the display controller sends to the screen. Reading another
    /// client's framebuffer needs root.
    pub fn capture(path: &str) -> Result<Canvas> {
        let card = open_card(path)?;
        let crtc = panel_crtc(&card)?;
        let fb = card.get_crtc(crtc).context("read CRTC")?.framebuffer().ok_or_else(|| anyhow!("nothing is scanned out"))?;
        let info = card.get_framebuffer(fb).context("read framebuffer")?;
        if info.bpp() != 32 {
            return Err(anyhow!("framebuffer has {} bits per pixel, not 32", info.bpp()));
        }
        let handle = info.buffer().ok_or_else(|| anyhow!("no buffer handle for the framebuffer; capturing needs root"))?;
        let (w, h) = info.size();
        let pitch = info.pitch() as usize;
        let map = drm_ffi::mode::dumbbuffer::map(card.as_fd(), handle.into(), 0, 0).context("map the framebuffer")?;
        let len = pitch * h as usize;
        let ptr = unsafe { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_SHARED, card.as_fd().as_raw_fd(), map.offset as libc::off_t) };
        if ptr == libc::MAP_FAILED {
            return Err(std::io::Error::last_os_error()).context("mmap the framebuffer");
        }
        let src = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
        let mut c = Canvas::new(w, h);
        let row = w as usize * 4;
        for y in 0..h as usize {
            c.data[y * row..(y + 1) * row].copy_from_slice(&src[y * pitch..y * pitch + row]);
        }
        unsafe { libc::munmap(ptr, len) };
        Ok(c)
    }
面板现在扫描的帧缓冲区，作为紧凑的 RGBA ///行：显示控制器发送到屏幕的信号。读取另一个客户端的 /// 帧缓冲区需要 root。pub fn capture（path： &str） -> 结果 <Canvas> { let card = open_card（path）？; let crtc = panel_crtc（&card）？; let fb = card.get_crtc（crtc）.context（“read CRTC”）？。framebuffer（）.ok_or_else（|| anyway！（”没有扫描任何东西））？;令 info = card.get_framebuffer（FB）.context（“read framebuffer”）？;if info.bpp（） ！= 32 { return Err（anyhow！（”帧缓冲区的 bit 是 {} 位每像素，不是 32“，info.bpp（））;} 让 handle = info.buffer（）.ok_or_else（|| anyway！（”帧缓冲区没有缓冲处理;捕获需要根））？;设 （w， h） = info.size（）;令 pitch = info.pitch（） 作为 usize;设 map =  drm_ffi：：mode：:d umbbuffer：：map（card.as_fd），handle.into（）， 0， 0）.context（“映射帧缓冲区”）？;设 len = 音高 * h，作为 usize;设 ptr = unsafe { libc：：mmap（std：:p tr：：null_mut（）， len， libc：:P ROT_READ， libc：：MAP_SHARED， card.as_fd（）.as_raw_fd（）， map.offset as libc：：off_t） };if ptr == libc：：MAP_FAILED { return Err（std：：io：：Error：：last_os_error（））.context（“mmap the framebuffer”）; } 让 src = unsafe { std：：slice：：from_raw_parts（ptr as *const u8， len） };设 mut c = 画布：：new（w， h）;设行 = w 为 usize * 4;对于 0..h 的 Y，作为 usize { c.data[y * row..（y + 1） * 行].copy_from_slice（&src[y * 音高..y * 音高 + 行]）;} unsafe { libc：：munmap（ptr， len） };Ok（c） }

    /// Direct scanout on the built-in panel: its fastest mode on the CRTC
    /// driving it, three dumb buffers taken in turn on its primary planes,
    /// and every other connector and plane taken off that CRTC.
    pub struct KmsSurface {
        card: Card,
        bufs: [Buf; 3],
        /// The buffer the next frame goes into.
        back: usize,
        /// A frame was committed whose flip has not come yet.
        flipping: bool,
        /// The planes showing the buffer, one per layer mixer, with their
        /// FB_ID properties.
        planes: Vec<(plane::Handle, property::Handle)>,
        w: u32,
        h: u32,
        order: Order,
        summary: String,
    }
内置面板上的直接扫描：CRTC 上用最快模式 /// 驱动它，三块哑缓冲区轮流放在主平面上，/// 以及所有其他连接器和平面从该 CRTC 上移除。pub struct KmsSurface { card： card， bufs： [Buf; 3]， /// 下一帧进入缓冲区。back： usize， /// 已提交一个帧，其翻转尚未完成。翻转： bool， /// 显示缓冲区的平面，每个层混合器一个，及其 /// FB_ID 属性。平面： Vec<（plane：：Handle， property：：Handle）>， w： u32， h： u32， order： Order， summary： String， }

    impl KmsSurface {
        pub fn open(path: &str) -> Result<Self> {
            let card = open_card(path)?;
            let res = card.resource_handles().context("read KMS resources")?;
impl KmsSurface { pub fn open（path： &str） -> Result<Self> { let card = open_card（path）？; let res = card.resource_handles（）.context（“read KMS resources”）？;

            let mut panel = None;
            for &h in res.connectors() {
                let info = card.get_connector(h, true).context("read connector")?;
                if is_panel(info.interface()) && info.state() == connector::State::Connected {
                    panel = Some(info);
                    break;
                }
            }
            let conn = panel.ok_or_else(|| anyhow!("no connected built-in panel"))?;
            let preferred = conn
                .modes()
                .iter()
                .find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED))
                .ok_or_else(|| anyhow!("the panel reports no preferred mode"))?;
            // The fastest refresh at the preferred size, the first listed among
            // equals: Android scrolls at it, and the 13 Ultra's touch reports
            // come at the same 120 Hz.
            let mode = *conn
                .modes()
                .iter()
                .filter(|m| m.size() == preferred.size())
                .rev()
                .max_by_key(|m| m.vrefresh())
                .expect("the preferred mode itself has the preferred size");
            let (w, h) = (mode.size().0 as u32, mode.size().1 as u32);
令 mut 面板 = 无;对于 & 在 res.connectors（） { let info = card.get_connector（h， true）.context（“read connector”）？; if is_panel（info.interface（）） && info.state（） == connector：：State：：Connected { panel = Some（info）; break; } } let conn = panel.ok_or_else（|| anyway！（”没有连接的内置面板）？;设 preferred = conn .modes（） .iter（） .find（|m| m.mode_type（）.contains（ModeTypeFlags：:P REFERED）） .ok_or_else（|| anyhow！（”面板报告无首选模式））？在首选大小下最快刷新率为：Android 滚动时，13 Ultra 的触控报告 // 同样为 120 Hz。让 mode = *conn .modes（） .iter（） .filter（|m| m.size（） == preferred.size（）） .rev（） .max_by_key（|m| m.vrefresh（）） .expect（“首选模式本身具有首选大小”）;设 （w， h） = （mode.size（）.0 作为 u32，mod.size（）.1 作为 u32）;

            // The CRTC already driving the panel, or the first one an
            // encoder of this connector can drive.
            let current = match conn.current_encoder() {
                Some(e) => card.get_encoder(e).context("read encoder")?.crtc(),
                None => None,
            };
            let crtc = match current {
                Some(c) => c,
                None => {
                    let mut found = None;
                    for &e in conn.encoders() {
                        let enc = card.get_encoder(e).context("read encoder")?;
                        if let Some(&c) = res.filter_crtcs(enc.possible_crtcs()).first() {
                            found = Some(c);
                            break;
                        }
                    }
                    found.ok_or_else(|| anyhow!("no CRTC can drive the panel"))?
                }
            };
CRTC 已经驱动配电箱，或者是该连接器的//编码器能驱动的第一个。让 current = match conn.current_encoder（） { Some（e） => card.get_encoder（e）.context（“read encoder”）？。crtc（）， 无 => 无， };令 crtc = 匹配电流 { Some（c） = > c， None => { let mut found = None;对于 conn.encoders（） { let enc = card.get_encoder（e）.context（“read encoder”）？;如果 let Some（&c） = res.filter_crtcs（enc.possible_crtcs（））.first（） { found = Some（c）; break; } } found.ok_or_else（|| anyhow！（”没有 CRTC 能驱动这个面板）？} };

            // Qualcomm's display driver blends a mode on one or more layer
            // mixers, each owning a vertical strip of the screen; the 13
            // Ultra's dual-pipe topology has two. The pipeline is set up the
            // way Android's compositor sets it up: one plane per mixer
            // covering its strip, the mode's DSC variant selected, and frame
            // starts posted to the hardware. With one plane across both
            // mixers, no DSC selection and the default trigger, the 13
            // Ultra's panel tore while scrolling.
            let conn_props = Props::read(&card, conn.handle())?;
            let layout = match conn_props.get("mode_properties") {
                Ok((_, blob)) => {
                    let text = card.get_property_blob(blob).context("read mode_properties")?;
                    SdeMode::find(&String::from_utf8_lossy(&text), &mode.name().to_string_lossy())?
                }
                Err(_) => SdeMode { mixers: 1, dsc: false, bpp: None },
            };
            // One plane per layer mixer, each covering its own strip, which is
            // how this phone's own recovery drives the same mode: two planes,
            // 720 wide each, one at x=0 and one at x=720.
            let strips = layout.mixers as usize;
            if w % layout.mixers != 0 {
                return Err(anyhow!("{w} pixels do not split into {strips} strips"));
            }
高通的显示驱动在一个或多个层//混合器上混合模式，每个混合器拥有屏幕的垂直条带;13 // Ultra 的双管拓扑结构有两个。流水线的设置方式是 Android 合成器的 // 方式：每个混音器用一个平面 // 覆盖其条带，选择模式的 DSC 变体，帧 // 开始发布到硬件上。两个混音台都用一块平面，没有 DSC 选择和默认扳机，13 // Ultra 的面板在滚动时撕裂了。设 conn_props = Props：：read（&card， conn.handle（）？;let layout = match conn_props.get（“mode_properties”） { Ok（（_， blob）） => { let text = card.get_property_blob（blob）.context（“read mode_properties”）？;SdeMode：：find（&String：：from_utf8_lossy（&text）， &mode.name（）.to_string_lossy（）？} Err（_） => SdeMode { 混频器： 1， dsc： false， bpp： None }， };每个层混合器一个平面，每个平面覆盖自己的条带，这也是这部手机恢复模式驱动相同模式的方式：两个平面，每个宽 720，一个 x=0，一个 x=720。令条 = layout.mixers 作为 usize;如果 w % layout.mixers ！= 0 { return Err（anyhow！（”{w} 像素不会分裂成 {strips} 条“）;}

            // Primary planes that can drive the CRTC, those already on it
            // first so the boot splash's planes are reused; planes on the
            // CRTC that are not used are switched off.
            let mut candidates: Vec<(plane::Handle, Vec<u32>, Props, bool)> = Vec::new();
            let mut attached = Vec::new();
            for p in card.plane_handles().context("list planes")? {
                let info = card.get_plane(p).context("read plane")?;
                let props = Props::read(&card, p)?;
                let on_crtc = info.crtc() == Some(crtc);
                if on_crtc {
                    attached.push(p);
                }
                if props.value("type")? == PlaneType::Primary as u64 && res.filter_crtcs(info.possible_crtcs()).contains(&crtc) {
                    candidates.push((p, info.formats().to_vec(), props, on_crtc));
                }
            }
            candidates.sort_by_key(|c| !c.3);
            if candidates.len() < strips {
                return Err(anyhow!("{strips} primary planes needed, {} can drive the CRTC", candidates.len()));
            }
            candidates.truncate(strips);
可以驱动 CRTC 的主平面，已经在 CRTC 上的飞机优先//，这样启动溅起的平面才能被重用;未使用的 // CRTC 平面被关闭。设 mut 候选对象：Vec<（plane：：Handle， Vec<u32>， Props， bool）> = Vec：：new（）;令 mut attached = Vec：：new（）;对于 p 在 card.plane_handles（）.context（“列表平面”）中？{ let info = card.get_plane（p）.context（“read plane”）？; let props = Props：：read（&card， p）？; let on_crtc = info.crtc（） == Some（crtc）; if on_crtc { attached.push（p）; } if props.value（“type”）？ == PlaneType：:P rimary as u64 && res.filter_crtcs（info.possible_crtcs（））.contains（&crtc） { candidates.push（（p， info.formats（）.to_vec）， props， on_crtc））; } } }candidates.sort_by_key（|c| ！c.3）;如果 candidates.len（） < strips { return Err（anyhow！（”{strips} 需要主飞机，{} 可以驱动 CRTC“，candidates.len（））;} candidates.截断（strips）;

            // `XB24` first, which is the order the canvas already holds its
            // bytes in -- and what this phone's own recovery hands its panel.
            let fourcc = SCANOUT
                .into_iter()
                .find(|f| candidates.iter().all(|c| c.1.contains(f)))
                .ok_or_else(|| anyhow!("the primary planes share none of the 32-bit RGB formats"))?;
            let order = order_of(fourcc);
            let fourcc = DrmFourcc::try_from(fourcc).expect("the formats chosen are ones it names");
先是“XB24”，这是画布已经保持//字节的顺序——也是这款手机恢复中面板显示的顺序。设 fourcc = SCANOUT .into_iter（） .find（|f| candidates.iter（）.all（|c| c.1.contains（f）））.ok_or_else（|| anyhow！（”主平面不共享任何 32 位 RGB 格式）？;令序 = order_of（fourcc）;设 fourcc = DrmFourcc：：try_from（fourcc）.expect（“所选格式即其命名格式”）;

            let make = || -> Result<Buf> {
                let db = card.create_dumb_buffer((w, h), fourcc, 32).context("create dumb buffer")?;
                let fb = card.add_planar_framebuffer(&Planar(&db), FbCmd2Flags::empty()).context("add framebuffer")?;
                let len = buffer::Buffer::pitch(&db) as usize * h as usize;
                let offset = drm_ffi::mode::dumbbuffer::map(card.as_fd(), buffer::Buffer::handle(&db).into(), 0, 0).context("map dumb buffer")?.offset;
                let fd = card.as_fd().as_raw_fd();
                let map = unsafe { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, fd, offset as libc::off_t) };
                if map == libc::MAP_FAILED {
                    return Err(std::io::Error::last_os_error()).context("mmap dumb buffer");
                }
                Ok(Buf { db, fb, map: map.cast(), len })
            };
            let bufs = [make()?, make()?, make()?];
设 = ||-> 结果 <Buf> { let db = card.create_dumb_buffer（（w， h）， fourcc， 32）.context（“create dumb buffer”）？; let fb = card.add_planar_framebuffer（&Planar（&db）， FbCmd2Flags：：empty（））.context（“add framebuffer”）？; let len = buffer：：Buffer：:p itch（&db） 作为 usize * h 作为 usize; let offset = drm_ffi：：mode：:d umbbuffer：：map（card.as_fd），buffer：：Buffer：：handle（&db）.into（）， 0， 0）.context（“map dumb buffer”）？.偏移;设 fd = card.as_fd（）.as_raw_fd（）;设 map = unsafe { libc：：mmap（std：:p tr：：null_mut（）， len， libc：:P ROT_READ | libc：:P ROT_WRITE， libc：：MAP_SHARED， fd， offset as libc：：off_t） };if map == libc：：MAP_FAILED { return Err（std：：io：：Error：：last_os_error（））.context（“mmap dumb buffer”）; }Ok（Buf { db， fb， map： map.cast（）， len }） };令 bufs = [make（）？， make（）？， make（）？];

            let crtc_id = u64::from(u32::from(crtc));
            let mut req = atomic::AtomicModeReq::new();
            for &other in res.connectors().iter().filter(|&&c| c != conn.handle()) {
                let props = Props::read(&card, other)?;
                if props.value("CRTC_ID")? == crtc_id {
                    req.add_property(other, props.handle("CRTC_ID")?, property::Value::CRTC(None));
                }
            }
            let crtc_props = Props::read(&card, crtc)?;
            let blob = card.create_property_blob(&mode).context("create mode blob")?;
            let first = card.create_property_blob(preferred).context("create mode blob")?;
            req.add_property(conn.handle(), conn_props.handle("CRTC_ID")?, property::Value::CRTC(Some(crtc)));
            // The mode is DSC-compressed: select its variant, as Android's
            // compositor does, and give the compressor the rate the mode was
            // made for. The rate is the panel's own: compressed at another
            // one it is decompressed with the wrong arithmetic, and what
            // reaches the glass is the picture with its colours mangled.
            //
            // The connector numbers the rate as an enumeration, so the number
            // the mode's description carries is looked up among the choices
            // rather than written straight in, and a connector that does not
            // offer it is left as it is.
            if layout.dsc {
                let (id, _) = conn_props.get("dsc_mode")?;
                req.add_property(conn.handle(), id, property::Value::UnsignedRange(enum_value(&card, id, "dsc_enabled")?));
                if let Some(bpp) = layout.bpp {
                    if let Ok((id, _)) = conn_props.get("bpp_mode") {
                        let choice = enum_choice(&card, id, &["bpp_8", "bpp_10", "bpp_12"], bpp);
                        if let Ok(choice) = choice {
                            req.add_property(conn.handle(), id, property::Value::UnsignedRange(choice));
                        }
                    }
                }
            }
            // Frame starts are posted to the hardware: the driver's own
            // default on display hardware of this generation, and Android's
            // compositor sets the same.
            if let Ok((id, _)) = conn_props.get("frame_trigger_mode") {
                req.add_property(conn.handle(), id, property::Value::UnsignedRange(enum_value(&card, id, "posted_start")?));
            }
            req.add_property(crtc, crtc_props.handle("MODE_ID")?, first);
            req.add_property(crtc, crtc_props.handle("ACTIVE")?, property::Value::Boolean(true));
            for &other in attached.iter().filter(|&&p| !candidates.iter().any(|c| c.0 == p)) {
                let props = Props::read(&card, other)?;
                req.add_property(other, props.handle("FB_ID")?, property::Value::Framebuffer(None));
                req.add_property(other, props.handle("CRTC_ID")?, property::Value::CRTC(None));
            }
            let strip = w / layout.mixers;
            let mut planes = Vec::new();
            for (i, (plane, _, props, _)) in candidates.iter().enumerate() {
                let x = strip * i as u32;
                let fb_prop = props.handle("FB_ID")?;
                req.add_property(*plane, fb_prop, property::Value::Framebuffer(Some(bufs[0].fb)));
                req.add_property(*plane, props.handle("CRTC_ID")?, property::Value::CRTC(Some(crtc)));
                req.add_property(*plane, props.handle("SRC_X")?, property::Value::UnsignedRange((x as u64) << 16));
                req.add_property(*plane, props.handle("SRC_Y")?, property::Value::UnsignedRange(0));
                req.add_property(*plane, props.handle("SRC_W")?, property::Value::UnsignedRange((strip as u64) << 16));
                req.add_property(*plane, props.handle("SRC_H")?, property::Value::UnsignedRange((h as u64) << 16));
                req.add_property(*plane, props.handle("CRTC_X")?, property::Value::SignedRange(x as i64));
                req.add_property(*plane, props.handle("CRTC_Y")?, property::Value::SignedRange(0));
                req.add_property(*plane, props.handle("CRTC_W")?, property::Value::UnsignedRange(strip as u64));
                req.add_property(*plane, props.handle("CRTC_H")?, property::Value::UnsignedRange(h as u64));
                // How the panel is told to read the pixels: this phone's own
                // recovery drives it as ITU-R BT.601 YCbCr in its limited
                // range, and a plane left at the driver's default is what
                // showed blue as magenta. Said as the connector's own
                // enumerations, and only where the connector offers them.
                if let Some((what, choice)) = colour_encoding(&card, props) {
                    req.add_property(*plane, what, property::Value::UnsignedRange(choice));
                }
                if let Some((what, choice)) = colour_range(&card, props) {
                    req.add_property(*plane, what, property::Value::UnsignedRange(choice));
                }
                planes.push((*plane, fb_prop));
            }
            // The panel comes up in its preferred mode, then switches to the
            // fastest. Powered up in the fast mode directly, the display
            // controller runs at the fast rate but the panel keeps its own:
            // the 13 Ultra's sends its TE at 60 Hz, and every frame waits two
            // TEs. Switching a running panel sends it the commands for the
            // new rate, as Android's switches do.
            commit(&card, req, AtomicCommitFlags::ALLOW_MODESET).context("set the panel mode")?;
            if mode.vrefresh() != preferred.vrefresh() {
                let mut req = atomic::AtomicModeReq::new();
                req.add_property(crtc, crtc_props.handle("MODE_ID")?, blob);
                commit(&card, req, AtomicCommitFlags::ALLOW_MODESET).context("switch the panel to its fastest refresh")?;
            }
 设 crtc_id = u64：：from（u32：：from（CRTC））;设 mut req = atomic：：AtomicModeReq：：new（）;对于 &other 在 res.connectors（）.iter（）.filter（|&&c| c ！= conn.handle（）） { let props = Props：：read（&card， other）？; if props.value（“CRTC_ID”）？ == crtc_id { req.add_property（other， props.handle（“CRTC_ID”）？， property：：Value：：CRTC（None））; } } } let crtc_props = Props：：read（&card， crtc）？;设 blob = card.create_property_blob（&mode）.context（“Create mode blob”）？;令 first = card.create_property_blob（preferred）.context（“创建模式 blob”）？req.add_property（conn.handle（）， conn_props.handle（“CRTC_ID”）？， property：：Value：：CRTC（Some（crtc）））;该模式采用 DSC 压缩：选择其变体，就像 Android 的//合成器一样，然后给压缩器显示该模式原本的速率。这个速率是面板本身的：在另一个面板上压缩时，它用错误的计算方法去压缩，结果到达玻璃的//就是画面颜色被扭曲的画面。// 连接器以枚举方式编号速率，因此模式描述中的数字 // 会在选项中查找，而不是直接写入，而没有 // 的连接器则保持原样。如果 layout.dsc { let （id， _） = conn_props.get（“dsc_mode”）？; req.add_property（conn.handle（）， id， property：：Value：：UnsignedRange（enum_value（&card， id， id， “dsc_enabled”）？）;如果 let Some（bpp） = layout.bpp { if let Ok（（id， _）） = conn_props.get（“bpp_mode”） { let choice = enum_choice（&card， id， &[“bpp_8”， “bpp_10”， “bpp_12”]， bpp）; 如果 let Ok（choice） = 选择 { req.add_property（conn.handle（）， id， property：：Value：：UnsignedRange（choice））;} } } } // 帧起始时间被发布到硬件上：驱动程序自身的 // 在这一代显示硬件上的默认设置，Android 的 // 合成器也设置相同。如果设 Ok（（id， _）） = conn_props。get（“frame_trigger_mode”） { req.add_property（conn.handle（）， id， property：：Value：：UnsignedRange（enum_value（&card， id， “posted_start”）？））; } req.add_property（crtc， crtc_props.handle（“MODE_ID”）？）;req.add_property（crtc， crtc_props.handle（“ACTIVE”）？， property：：Value：：Boolean（true））;对于 attached.iter（）.filter（|&&p| ！candidates.iter（）.any（|c| c.0 == p）） { let props = Props：：read（&card， other）？; req.add_property（other， props.handle（“FB_ID”）？， property：：Value：：Framebuffer（None））; req.add_property（other， props.handle（“CRTC_ID”）？， property：：Value：：CRTC（None））; } let strip = w / layout.mixers;令 mut 平面 = Vec：：new（）;对于 （i， （plane， _， props， _）） 在 candidates.iter（）.enumerate（） { 令 x = 剥离 * i 作为 u32; 令 fb_prop = props.handle（“FB_ID”）？; req.add_property（*plane， fb_prop， property：：Value：：Framebuffer（Some（bufs[0].fb））））; req.add_property（*plane， props.handle（“CRTC_ID”）？， property：：Value：：CRTC（Some（crtc）））; req.add_property（*plane， props.handle（“SRC_X”）？， property：：Value：：UnsignedRange（（x 作为 u64） << 16））; req.add_property（*plane， props.handle（“SRC_Y”）？， property：：Value：：UnsignedRange（0））; req.add_property（*plane， props.handle（“SRC_W”）？， property：：Value：：UnsignedRange（（条带为u64） << 16））;req.add_property（*plane， props.handle（“SRC_H”）？，property：：Value：：UnsignedRange（（h作为u64） << 16））;req.add_property（*plane， props.handle（“CRTC_X”）？，property：：Value：：SignedRange（x as i64））;req.add_property（*plane， props.handle（“CRTC_Y”）？， property：：Value：：SignedRange（0））;req.add_property（*plane， props.handle（“CRTC_W”）？，property：：Value：：UnsignedRange（条带为u64））;req.add_property（*plane， props.handle（“CRTC_H”）？， property：：Value：：UnsignedRange（h as u64））;面板如何读取像素：这部手机的 // 恢复程序将它作为 ITU-R BT 驱动。601 YCbCr 在其有限范围内//，而保持在驾驶员默认状态的飞机则显示蓝色为品红色。说成连接器自己的//枚举，且仅在连接器提供时使用。如果让 Some（（what， choice）） = colour_encoding（&card， props） { req.add_property（*plane， what， property：：Value：：UnsignedRange（choice））; } 如果 let Some（（what， choice）） = colour_range（&card， props） { req.add_property（*plane， what， property：：Value：：UnsignedRange（choice））; } planes.push（（*plane， fb_prop））;} // 面板会以其偏好模式启动，然后切换到最快的//模式。直接在快速模式下开机，显示 // 控制器运行速度很快，但面板保持自己的速度：// 13 Ultra 的 TE 频率为 60 Hz，每帧等待两个 // TE。切换正在运行的面板会发送 // 新速率的命令，就像 Android 的交换机一样。commit（&card， req， AtomicCommitFlags：：ALLOW_MODESET）.context（“set the panel mode”）？;if mode.vrefresh（） ！= preferred.vrefresh（） { let mut req = atomic：：AtomicModeReq：：new（）; req.add_property（crtc， crtc_props.handle（“MODE_ID”）？blob）; commit（&card， req， AtomicCommitFlags：：ALLOW_MODESET）.context（“将面板切换到最快刷新”）？;}

            let ids: Vec<u32> = planes.iter().map(|p| u32::from(p.0)).collect();
            // What the panel was told, not merely how it was found: a colour
            // that reaches it wrong is read from the format each plane is
            // handed, the colour space it is asked to convert from, and the
            // strips the layer mixers blend. All of it goes in the summary
            // the recovery writes to the kernel log once the surface is up.
            let mut told = String::new();
            for &(p, _) in &planes {
                let props = Props::read(&card, p).context("read plane properties")?;
                let get = |name: &str| props.value(name).map(|v| v.to_string()).unwrap_or_else(|_| "-".into());
                let colour = ["COLOR_ENCODING", "COLOR_RANGE", "COLOR_SPACE", "CSC_MATRIX", "ALPHA_ENABLE", "PIXEL_FORMAT"]
                    .iter()
                    .map(|n| format!("{n}={}", get(n)))
                    .collect::<Vec<_>>()
                    .join(",");
                told.push_str(&format!(
                    " plane {} src {}+{} {}x{} crtc {}+{} {}x{} {colour};",
                    u32::from(p),
                    get("SRC_X"),
                    get("SRC_Y"),
                    get("SRC_W"),
                    get("SRC_H"),
                    get("CRTC_X"),
                    get("CRTC_Y"),
                    get("CRTC_W"),
                    get("CRTC_H"),
                ));
            }
            let crtc_props_now = Props::read(&card, crtc).context("read CRTC properties")?;
            let crtc_colour = ["GAMMA_LUT", "DEGAMMA_LUT", "CTM", "COLOR_ENCODING", "COLOR_RANGE"]
                .iter()
                .map(|n| format!("{n}={}", crtc_props_now.value(n).map(|v| v.to_string()).unwrap_or_else(|_| "-".into())))
                .collect::<Vec<_>>()
                .join(",");
            let summary = format!(
                "{:?} connector {} mode {:?} {w}x{h} {} Hz, crtc {}, planes {ids:?} {:?}, strips {strips}, dsc {}, crtc {crtc_colour};{told}",
                conn.interface(),
                u32::from(conn.handle()),
                mode.name(),
                mode.vrefresh(),
                u32::from(crtc),
                fourcc,
                layout.dsc,
            );
            Ok(KmsSurface { card, bufs, back: 1, flipping: false, planes, w, h, order, summary })
        }
设 ids： Vec<u32> = planes.iter（）.map（|p| u32：：from（p.0））.collect（）; // 面板被告知的内容，而不仅仅是发现方式：错误到达的颜色//会读取每个平面所接收的格式、需要转换的色彩空间，以及层混合器混合的//去除。所有内容都写在摘要里 // 恢复会在表面上线后写入内核日志。让 mut told = String：：new（）;对于 &（p， _） 在 & 平面中 { 让 props = Props：：read（&card， p）.context（“read plane properties”）？; 令 get = |name： &str| props.value（name）.map（|v| v.to_string（））.unwrap_or_else（|_|“-”.into（））;设 color = [“COLOR_ENCODING”， “COLOR_RANGE”、“COLOR_SPACE”、“CSC_MATRIX”、“ALPHA_ENABLE”、“PIXEL_FORMAT”] .iter（） .map（|n| format！（”{n}={}“， get（n））） .collect：：<Vec<_>>（） .join（”，“）; told.push_str（&format！（“平面 {} src {}+{} {}x{} crtc {}+{} {}x{} {colour};”， u32：：from（p）， get（“SRC_X”）、get（“SRC_Y”）、get（“SRC_W”）、get（“SRC_H”）、get（“CRTC_X”）、get（“CRTC_Y”）、get（“CRTC_W”）、get（“CRTC_H”））;} 令 crtc_props_now = Props：：read（&card， crtc）.context（“read CRTC properties”）？;设 crtc_colour = [“GAMMA_LUT”， “DEGAMMA_LUT”， “CTM”、“COLOR_ENCODING”， “COLOR_RANGE”] .iter（） .map（|n| format！（”{n}={}“， crtc_props_now.value（n）.map（|v| v.to_string（））.unwrap_or_else（|_|“-”.into（）））））.collect：：<Vec<_>>（） .join（“，”）; let summary = format！（"{:?}connector {} mode {：？}{w}x{h} {} Hz， crtc {}， planes {ids：？}{：？}， strips {strips}， dsc {}， crtc {crtc_colour};{told}“， conn.interface（）， u32：：from（conn.handle（））， mode.name（）， mode.vrefresh（）， u32：：from（crtc）， fourcc， layout.dsc， ）;Ok（KmsSurface { card， bufs， back： 1， flipping： false， planes， w， h， order， summary }） }

        pub fn describe(&self) -> &str {
            &self.summary
        }
pub fn descript（&self） -> &str { &self.summary }

    }

    impl Drop for KmsSurface {
        fn drop(&mut self) {
            for buf in &self.bufs {
                unsafe { libc::munmap(buf.map.cast(), buf.len) };
            }
        }
    }
impl Drop for KmsSurface { fn drop（&mut self） { for buf in &self.bufs { unsafe { libc：：munmap（buf.map.cast（）， buf.len） }; }

    impl Surface for KmsSurface {
        fn size(&self) -> (u32, u32) {
            (self.w, self.h)
        }
impl Surface for KmsSurface { fn size（&self） -> （u32， u32） { （self.w， self.h） }

        /// A command-mode panel is sent a frame from one of its TE signals
        /// through most of the time to the next (6.8 of 8.3 ms on the 13
        /// Ultra at 120 Hz), and the frame's flip comes as the sending
        /// starts. A frame is committed the moment the flip of the one
        /// before comes, to be sent from the next TE, and drawn while that
        /// one is sent: into the buffer of the frame before both, which the
        /// panel finished reading before the frame now being sent started.
        fn present(&mut self, canvas: &Canvas) -> Result<()> {
            if (canvas.w, canvas.h) != (self.w, self.h) {
                return Err(anyhow!("canvas {}x{} does not match the display {}x{}", canvas.w, canvas.h, self.w, self.h));
            }
            let order = self.order;
            let buf = &self.bufs[self.back];
            let pitch = buffer::Buffer::pitch(&buf.db) as usize;
            let row = self.w as usize * 4;
            let dst = unsafe { std::slice::from_raw_parts_mut(buf.map, buf.len) };
            for y in 0..self.h as usize {
                let src = &canvas.data[y * row..(y + 1) * row];
                let out = &mut dst[y * pitch..y * pitch + row];
                match order {
                    Order::Rgba => out.copy_from_slice(src),
                    Order::Bgra => {
                        for (o, s) in out.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                            *o = [s[2], s[1], s[0], s[3]];
                        }
                    }
                }
            }
            let mut req = atomic::AtomicModeReq::new();
            for &(plane, fb_prop) in &self.planes {
                req.add_property(plane, fb_prop, property::Value::Framebuffer(Some(buf.fb)));
            }
            self.settle()?;
            let flags = AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT;
            self.card.atomic_commit(flags, req).context("flip")?;
            self.flipping = true;
            self.back = (self.back + 1) % self.bufs.len();
            Ok(())
        }
命令模式面板大部分时间从其 TE 信号///发送帧到下一个信号（13 /// Ultra 120 Hz 时为 6.8 毫秒，共 8.3 毫秒），帧翻转发生在发送///开始时。当前一个////的翻转发生时，帧就被提交，从下一个 TE 发送，并在发送该///帧时被绘制到两个/之前的帧缓冲区，该缓冲区在///面板读取完毕后，当前发送的帧开始。fn present（&mut self， canvas： &Canvas） -> Result<（）> { if （canvas.w， canvas.h） ！= （self.w， self.h） { return Err（anyhow！（”canvas {}x{} 与显示 {}x{}“、canvas.w、canvas.h、self.w、self.h）不匹配）;} 令 Order = 自序;令 buf = &self.bufs[self.back];令 pitch = buffer：：Buffer：:p itch（&buf.db） 作为 usize;设行 = self.w，usize * 4;设 dst = unsafe { std：：slice：：from_raw_parts_mut（buf.map， buf.len） };对于 y 在 0..self.h 作为 usize { 让 src = &canvas.data[y * row..（y + 1） * 行];放出 = & mut DST[y * 音高......y * 音高 + 行];匹配顺序 { Order：：Rgba => out.copy_from_slice（src）， Order：：Bgra => { for （o， s） in out.as_chunks_mut：：<4>（）.0.iter_mut（）（）.zip（src.as_chunks：：<4>（）.0） { *o = [s[2]， s[1]， s[0]， s[3]]; } } } } let mut req = atomic：：AtomicModeReq：：new（）; for &（plane， fb_prop） in &self.planes { req.add_property（plane， fb_prop， property：：Value：：Framebuffer（Some（buf.fb））））; } self.settle（）？; let flags = AtomicCommitFlags：：非阻塞 |AtomicCommitFlags：:P AGE_FLIP_EVENT;self.card.atomic_commit（flags， req）.context（“flip”）？;self.flipping = true;self.back = （self.back + 1） % self.bufs.len（）;Ok（（）） }

        fn settle(&mut self) -> Result<()> {
            if self.flipping {
                flipped(&self.card).context("flip")?;
                self.flipping = false;
            }
            Ok(())
        }
    }
}

fn settle（&mut self） -> 结果 <（）> { 如果 self.flipping { flipped（&self.card）.context（“flip”）？; self.flipping = false; }Ok（（）） } } }

loading ...  加载中......
