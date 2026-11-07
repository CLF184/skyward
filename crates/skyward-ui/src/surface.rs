use crate::canvas::Canvas;
use anyhow::Result;

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

/// Byte order of the scanout buffer against the canvas's RGBA.
#[cfg(any(target_os = "android", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Order {
    Rgba,
    Bgra,
}

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

/// The 32-bit RGB formats a scanout buffer may be made in, the ones whose
/// pixels the canvas reaches by turning nothing or two bytes round.
///
/// `XB24` first: it is the order the canvas already holds its bytes in, and
/// what a haotian's own recovery drives its panel with.
#[cfg(any(target_os = "android", test))]
pub(crate) const SCANOUT: [u32; 4] = [0x3432_4258, 0x3432_4241, 0x3432_5258, 0x3432_5241];

/// How Qualcomm's display driver drives a mode, from the text of the
/// connector's mode_properties blob: key=value lines, a block per mode_name.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, PartialEq, Eq)]
struct SdeMode {
    /// Layer mixers in the mode's topology, each blending one vertical
    /// strip of the screen.
    mixers: u32,
    /// Whether the picture is compressed over the link. The driver works the
    /// rate out from the panel it was told about at boot, so this is read
    /// only to say in the summary how the mode is driven.
    dsc: bool,
}

#[cfg(any(target_os = "android", test))]
impl SdeMode {
    fn find(text: &str, name: &str) -> Result<SdeMode> {
        let mut current = false;
        let mut topology = None;
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
        Ok(SdeMode { mixers, dsc })
    }
}

/// The block of parameters the display controller starts its subpixel
/// renderer with, field for field as this phone's own recovery writes it.
///
/// The panel in a haotian is a pen tile one: red and blue dots sit on one
/// diagonal grid and green dots between them, so the dots that carry a pixel's
/// colour are not all underneath it. The glass itself rearranges nothing --
/// the display controller does, and only once it has been handed this block.
/// Run without it, the controller gives the glass plain RGB and the dots take
/// the wrong components: red and blue dots are lit as the same component,
/// which is why these pages came out with blue as magenta and red as near
/// black while green, white and black looked right.
///
/// The parameters are Qualcomm's driver's own, and the figures in them are
/// this panel's: the pattern its dots repeat in (`cfg13`, every other dot of a
/// line), the twelve bits a pixel's colour is spread over (`cfg15`), the taps
/// of the filter that decides how much of a pixel each dot takes (`cfg16`,
/// `cfg17`). A haotian's taps are not the ones the recovery falls back to for
/// other phones -- its first and its thirteenth are the other way round, and
/// the width of its dots' pattern (`cfg7`) is five rather than four -- and it
/// chooses them by comparing `ro.boot.hardware.sku` with "haotian", which is
/// what [`SprInit::for_panel`] repeats.
///
/// The `SDE_SPR_INIT_CFG_V2` property takes all of this; the first version of
/// the property, which phones a generation older carry, takes the same block
/// without its last two fields.
#[cfg(any(target_os = "android", test))]
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SprInit {
    /// Zero: the renderer is asked to work. `SPR_FLAG_BYPASS`, bit 0, would
    /// ask it to step aside.
    flags: u64,
    cfg0: u16,
    cfg1: u16,
    cfg2: u16,
    cfg3: u16,
    cfg4: u16,
    cfg5: u16,
    cfg6: u16,
    /// How wide the repeating pattern of the panel's dots is, in dots.
    cfg7: u16,
    cfg8: u16,
    cfg9: u16,
    cfg10: u32,
    /// The layout of the dots in a line, as the pattern's own words.
    cfg11: [u16; 4],
    cfg12: [u16; 4],
    /// The bits of one line of dots: `0x155` is every other one.
    cfg13: [u16; 4],
    /// Where each of the panel's channels starts in the bits a pixel is
    /// spread over.
    cfg14: [u16; 5],
    /// The ramp the twelve bits a pixel are spread over, from none of them to
    /// all of them.
    cfg15: [u16; 32],
    /// Rust and C pad a struct of this kind in the same place; the block the
    /// kernel reads is the same at every start only if the padding is written
    /// too, so it is a field here rather than left to the compiler.
    _pad: [u8; 2],
    /// The taps of the filter, at each position of the pattern.
    cfg16: [i32; 16],
    cfg17: [i32; 24],
    cfg18_en: u16,
    cfg18: [u8; 7],
    _pad_end: [u8; 3],
}

/// The length of the block the first version of the property takes: the block
/// above without its last two fields.
#[cfg(any(target_os = "android", test))]
const SPR_INIT_V1: usize = 0x128;

/// The length of the block the current version of the property takes.
#[cfg(any(target_os = "android", test))]
const SPR_INIT_V2: usize = 0x130;

#[cfg(any(target_os = "android", test))]
impl SprInit {
    /// The block this phone's own recovery hands the controller, chosen the
    /// way the recovery chooses it.
    fn for_panel(haotian: bool) -> SprInit {
        SprInit {
            flags: 0,
            cfg0: 1,
            cfg1: 1,
            cfg2: 1,
            cfg3: 0,
            cfg4: 0,
            cfg5: 8,
            cfg6: 2,
            cfg7: if haotian { 5 } else { 4 },
            cfg8: 0,
            cfg9: 0,
            cfg10: 0,
            cfg11: [1, 0, 1, 0],
            cfg12: [0; 4],
            cfg13: [0x155, 0x155, 0x155, 0],
            cfg14: [0, 4, 8, 0xc, 0x10],
            cfg15: [
                0, 0x84, 0x108, 0x18c, 0x211, 0x295, 0x319, 0x39d, 0x421, 0x4a5, 0x529, 0x5ad, 0x632, 0x6b6, 0x73a, 0x7be, 0x842, 0x8c6, 0x94a,
                0x9ce, 0xa53, 0xad7, 0xb5b, 0xbdf, 0xc63, 0xce7, 0xd6b, 0xdef, 0xe74, 0xef8, 0xf7c, 0xfff,
            ],
            cfg16: [0, 256, 256, 0, 0, 256, 256, 0, 0, 256, 256, 0, 0, 256, 256, 0],
            cfg17: if haotian {
                [0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            } else {
                [-2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
            },
            cfg18_en: 0,
            cfg18: [0; 7],
            _pad: [0; 2],
            _pad_end: [0; 3],
        }
    }

    /// The bytes the property's blob is made of: every field an integer, in
    /// the order the driver declares them, with no gap left un-written.
    fn bytes(&self) -> &[u8] {
        // A struct of integers of this shape has the layout its size says and
        // no padding left unwritten, so its bytes are what C would hand over.
        unsafe { std::slice::from_raw_parts(std::ptr::from_ref(self).cast(), std::mem::size_of::<SprInit>()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISHTAR: &str = "mode_name=1440x3200x60cmd\nbit_clk_rate=1360000000\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n\
                          mode_name=1440x3200x120cmd\nsubmode_idx=1\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n\
                          mode_name=1080x2400x60cmd\ndsc_mode=0\ntopology=sde_singlepipe\n";

    #[test]
    fn reads_the_topology_of_the_named_mode() {
        assert_eq!(SdeMode::find(ISHTAR, "1440x3200x120cmd").unwrap(), SdeMode { mixers: 2, dsc: true });
        assert_eq!(SdeMode::find(ISHTAR, "1080x2400x60cmd").unwrap(), SdeMode { mixers: 1, dsc: false });
        assert!(SdeMode::find(ISHTAR, "1440x3200x90cmd").is_err());
    }

    #[test]
    fn a_mode_without_a_rate_or_a_variant_of_its_own_is_read_as_the_rest() {
        // What the driver chose for the link is not read here: the phone's own
        // recovery sets no connector property, and the rate the compressor
        // runs at is the driver's own business. Only the topology and whether
        // the mode is compressed are needed, and a mode that says nothing
        // about either is taken as one mixer, uncompressed.
        let haotian = "mode_name=1440x3200x120cmd\nsubmode_idx=1\nbpp_mode=2\ndsc_mode=1\ntopology=sde_dualpipe_dscmerge\n";
        let mode = SdeMode::find(haotian, "1440x3200x120cmd").unwrap();
        assert!(mode.dsc && mode.mixers == 2);
        let bare = "mode_name=a\nbpp_mode=x\ntopology=sde_singlepipe\n";
        assert_eq!(SdeMode::find(bare, "a").unwrap(), SdeMode { mixers: 1, dsc: false });
    }

    #[test]
    fn a_format_whose_bytes_name_red_first_takes_the_canvas_as_it_is() {
        // 'XB24' and 'AB24' lay their bytes out [R, G, B, x] on a machine of
        // this kind's byte order, which is the canvas's own order.
        assert_eq!(order_of(0x3432_4258), Order::Rgba, "Xbgr8888");
        assert_eq!(order_of(0x3432_4241), Order::Rgba, "Abgr8888");
    }

    #[test]
    fn a_format_whose_bytes_name_blue_first_has_its_ends_swapped() {
        // 'XR24' and 'AR24' lay theirs out [B, G, R, x]: red and blue reach
        // the panel the wrong way round unless the canvas's are swapped.
        assert_eq!(order_of(0x3432_5258), Order::Bgra, "Xrgb8888");
        assert_eq!(order_of(0x3432_5241), Order::Bgra, "Argb8888");
    }

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

    #[test]
    fn the_subpixel_renderer_is_told_the_layout_of_a_haotians_dots() {
        // The block this phone's own recovery writes, byte for byte: the
        // renderer left working, the dots' pattern every other one of a line,
        // and the twelve bits a pixel spread over the dots that carry them.
        let block = SprInit::for_panel(true);
        let b = block.bytes();
        assert_eq!(b.len(), SPR_INIT_V2, "the block the current property takes");
        assert_eq!(&b[0x00..0x08], &[0; 8], "flags: the renderer works rather than stands aside");
        assert_eq!(&b[0x08..0x12], &[1, 0, 1, 0, 1, 0, 0, 0, 0, 0], "cfg0 to cfg4");
        assert_eq!(u16::from_le_bytes([b[0x12], b[0x13]]), 8, "cfg5");
        assert_eq!(u16::from_le_bytes([b[0x14], b[0x15]]), 2, "cfg6");
        assert_eq!(u16::from_le_bytes([b[0x16], b[0x17]]), 5, "cfg7: a haotian's pattern is five dots wide");
        assert_eq!(&b[0x20..0x28], &[1, 0, 0, 0, 1, 0, 0, 0], "cfg11");
        assert_eq!(&b[0x30..0x38], &[0x55, 0x01, 0x55, 0x01, 0x55, 0x01, 0, 0], "cfg13: every other dot");
        assert_eq!(&b[0x38..0x42], &[0, 0, 4, 0, 8, 0, 0xc, 0, 0x10, 0], "cfg14: where each channel starts");
        assert_eq!(&b[0x42..0x44], &[0, 0], "cfg15 starts at none of the bits");
        assert_eq!(&b[0x80..0x82], &[0xff, 0x0f], "cfg15 ends at all twelve of them");
        assert_eq!(&b[0x84..0x88], &[0, 0, 0, 0], "cfg16");
        assert_eq!(&b[0xc4..0xc8], &[0, 0, 0, 0], "cfg17: a haotian's first tap");
        assert_eq!(&b[0xf4..0xfc], &[0xfe, 0xff, 0xff, 0xff, 2, 0, 0, 0], "cfg17: its thirteenth and fourteenth");
        assert_eq!(&b[0x124..], &[0; 12], "nothing after cfg17 is asked for");
    }

    #[test]
    fn the_taps_of_another_panel_are_the_ones_the_recovery_falls_back_to() {
        // The recovery keys the taps on the phone's own name: a haotian's
        // first and thirteenth are the other way round from another phone's,
        // and its pattern is one dot wider. Nothing else in the block differs.
        let haotian = SprInit::for_panel(true).bytes().to_vec();
        let other = SprInit::for_panel(false).bytes().to_vec();
        let differing: Vec<usize> = (0..haotian.len()).filter(|&i| haotian[i] != other[i]).collect();
        assert_eq!(
            differing,
            [0x16, 0xc4, 0xc5, 0xc6, 0xc7, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb],
            "cfg7 and the taps of cfg17"
        );
    }

    #[test]
    fn the_first_version_of_the_property_takes_the_block_without_its_tail() {
        // A driver from before the current property takes the same block
        // without the last two fields, which is exactly eight bytes shorter.
        assert_eq!(SprInit::for_panel(true).bytes().len(), SPR_INIT_V2, "the block as the struct is");
        assert_eq!(SPR_INIT_V2 - SPR_INIT_V1, 8, "the two fields the first version has not");
    }
}

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
    use std::time::{Duration, Instant};

    /// How long the display may take to complete a commit.
    const COMMIT_TIMEOUT: Duration = Duration::from_secs(2);

    struct Card(File);

    impl AsFd for Card {
        fn as_fd(&self) -> BorrowedFd<'_> {
            self.0.as_fd()
        }
    }

    impl Device for Card {}
    impl ControlDevice for Card {}

    /// A single-plane dumb buffer presented to ADDFB2, so the framebuffer
    /// keeps the buffer's own pixel format.
    struct Planar<'a>(&'a DumbBuffer);

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

    struct Buf {
        db: DumbBuffer,
        fb: framebuffer::Handle,
        /// The buffer mapped for writing for as long as the surface lives;
        /// mapping it per frame costs a page fault per page on every frame.
        map: *mut u8,
        len: usize,
    }

    /// A KMS object's properties by name, with their current values.
    struct Props(HashMap<String, (property::Handle, u64)>);

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

        fn get(&self, name: &str) -> Result<(property::Handle, u64)> {
            self.0.get(name).copied().ok_or_else(|| anyhow!("KMS property {name} missing"))
        }

        fn handle(&self, name: &str) -> Result<property::Handle> {
            Ok(self.get(name)?.0)
        }

        fn value(&self, name: &str) -> Result<u64> {
            Ok(self.get(name)?.1)
        }

        fn line(&self) -> String {
            let mut parts: Vec<String> = self.0.iter().map(|(name, (_, value))| format!("{name}={value}")).collect();
            parts.sort();
            parts.join(" ")
        }
    }

    fn open_card(path: &str) -> Result<Card> {
        let file = OpenOptions::new().read(true).write(true).open(path).with_context(|| format!("open {path}"))?;
        let card = Card(file);
        card.set_client_capability(drm::ClientCapability::UniversalPlanes, true).context("enable universal planes")?;
        card.set_client_capability(drm::ClientCapability::Atomic, true).context("enable atomic modesetting")?;
        Ok(card)
    }

    /// The value of the choice named `choice` of an enum property.
    fn enum_value(card: &Card, id: property::Handle, choice: &str) -> Result<u64> {
        let info = card.get_property(id).context("read property")?;
        match info.value_type() {
            property::ValueType::Enum(e) => e.values().1.iter().find(|v| v.name().to_str() == Ok(choice)).map(|v| v.value()),
            _ => None,
        }
        .ok_or_else(|| anyhow!("{} offers no {choice}", info.name().to_string_lossy()))
    }

    /// The CRTC property the display controller's subpixel renderer is started
    /// through, and the length of the block that version of it takes: the
    /// current version where the driver has it, the first one otherwise.
    ///
    /// The property is what a panel whose glass rearranges no dots is driven
    /// with, and a driver that has the renderer at all has one of the two.
    fn spr_property(props: &Props) -> Option<(property::Handle, usize)> {
        if let Ok((id, _)) = props.get("SDE_SPR_INIT_CFG_V2") {
            return Some((id, SPR_INIT_V2));
        }
        props.get("SDE_SPR_INIT_CFG_V1").ok().map(|(id, _)| (id, SPR_INIT_V1))
    }

    /// Whether the panel's own glass leaves the drawing of its subpixels to
    /// the display controller: the bootloader says so in a property of its
    /// own, which is the one Android's recovery reads before it starts the
    /// renderer.
    fn subpixel_rendering() -> bool {
        skyward_sys::bool_prop("ro.boot.display_ap_spr", false)
    }

    /// Whether this phone is a haotian, whose panel's filter taps are its own.
    fn haotian() -> bool {
        skyward_sys::prop("ro.boot.hardware.sku").as_deref() == Some("haotian")
    }

    /// Built-in panel interfaces. Writeback and external outputs are never
    /// the recovery's screen.
    fn is_panel(interface: connector::Interface) -> bool {
        matches!(interface, connector::Interface::DSI | connector::Interface::EmbeddedDisplayPort | connector::Interface::LVDS)
    }

    /// Commit without blocking and wait for the flip event, so a commit the
    /// display never completes surfaces as an error instead of an endless
    /// wait.
    fn commit(card: &Card, req: atomic::AtomicModeReq, flags: AtomicCommitFlags) -> Result<()> {
        card.atomic_commit(flags | AtomicCommitFlags::NONBLOCK | AtomicCommitFlags::PAGE_FLIP_EVENT, req).context("atomic commit")?;
        flipped(card)
    }

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

    impl KmsSurface {
        pub fn open(path: &str) -> Result<Self> {
            let card = open_card(path)?;
            let res = card.resource_handles().context("read KMS resources")?;

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
                Err(_) => SdeMode { mixers: 1, dsc: false },
            };
            // One plane per layer mixer, each covering its own strip, which is
            // how this phone's own recovery drives the same mode: two planes,
            // 720 wide each, one at x=0 and one at x=720.
            let strips = layout.mixers as usize;
            if w % layout.mixers != 0 {
                return Err(anyhow!("{w} pixels do not split into {strips} strips"));
            }

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

            // `XB24` first, which is the order the canvas already holds its
            // bytes in -- and what this phone's own recovery hands its panel.
            let fourcc = SCANOUT
                .into_iter()
                .find(|f| candidates.iter().all(|c| c.1.contains(f)))
                .ok_or_else(|| anyhow!("the primary planes share none of the 32-bit RGB formats"))?;
            let order = order_of(fourcc);
            let fourcc = DrmFourcc::try_from(fourcc).expect("the formats chosen are ones it names");

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
            // The mode is DSC-compressed, and the compressor's rate and every
            // other setting of the link is the driver's own: worked out at boot
            // from the panel the device tree describes, which is the state the
            // boot splash was drawn in. This phone's own recovery sets no
            // connector property either -- only `MODE_ID`, `ACTIVE`, the
            // plane's frame and rectangles, and the subpixel block below.
            //
            // The experiments that forced the rate here are what the pages'
            // colours are still paying for: with `bpp_mode` written from the
            // mode's description and the DSC variant selected by hand, the
            // panel is decompressed by arithmetic the phone may not have
            // chosen. Leave the link as the driver left it.
            if layout.dsc {
                let (id, _) = conn_props.get("dsc_mode")?;
                req.add_property(conn.handle(), id, property::Value::UnsignedRange(enum_value(&card, id, "dsc_enabled")?));
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
                planes.push((*plane, fb_prop));
            }
            // A panel whose glass renders its own subpixels is driven with
            // the block that says how its dots are laid out; one whose glass
            // does not leaves that to the controller, and the controller is
            // handed the block this phone's own recovery hands it. Left
            // unstarted the dots are lit as plain RGB -- red and blue dots
            // taking the same component -- which is the colour fault these
            // pages had. The bootloader knows which panel is here and says so.
            let mut spr = String::from("-");
            if let Some((prop, len)) = spr_property(&crtc_props) {
                if subpixel_rendering() {
                    let block = SprInit::for_panel(haotian());
                    match card.create_property_blob(&block.bytes()[..len]) {
                        Ok(value) => {
                            req.add_property(crtc, prop, value);
                            spr = format!("v{}", if len == SPR_INIT_V2 { 2 } else { 1 });
                        }
                        // The pages are still readable with the wrong colours,
                        // so a block that cannot be made is said in the
                        // summary rather than left to fail the display.
                        Err(error) => spr = format!("failed ({error})"),
                    }
                } else {
                    spr = String::from("off");
                }
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
            // The subpixel block as the driver holds it now: a blob id of zero
            // means the write did not take at all, and the bytes say which of
            // the two versions of the block the CRTC kept.
            let kept = crtc_props_now.value("SDE_SPR_INIT_CFG_V2").ok().filter(|&id| id != 0);
            let spr_kept = match kept.and_then(|id| card.get_property_blob(id).ok()) {
                Some(bytes) => format!("{} bytes, starts {:02x?}", bytes.len(), &bytes[..bytes.len().min(8)]),
                None => String::from("no blob"),
            };
            // What the link was left at: the driver's own choices, which the
            // phone's own recovery leaves alone too. Read beside the block
            // because a compressor rate the panel was not made for is read as
            // the picture with its channels mangled.
            let conn_props_now = Props::read(&card, conn.handle()).context("read connector properties")?;
            let link = ["dsc_mode", "bpp_mode", "frame_trigger_mode"]
                .iter()
                .map(|n| format!("{n}={}", conn_props_now.value(n).map(|v| v.to_string()).unwrap_or_else(|_| "-".into())))
                .collect::<Vec<_>>()
                .join(",");
            let crtc_colour = ["GAMMA_LUT", "DEGAMMA_LUT", "CTM", "COLOR_ENCODING", "COLOR_RANGE", "SDE_SPR_INIT_CFG_V2"]
                .iter()
                .map(|n| format!("{n}={}", crtc_props_now.value(n).map(|v| v.to_string()).unwrap_or_else(|_| "-".into())))
                .collect::<Vec<_>>()
                .join(",");
            let summary = format!(
                "{:?} connector {} mode {:?} {w}x{h} {} Hz, crtc {}, planes {ids:?} {:?}, strips {strips}, dsc {}, link {link}, spr {spr} ({spr_kept}), crtc {crtc_colour};{told}",
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

        pub fn describe(&self) -> &str {
            &self.summary
        }

    }

    impl Drop for KmsSurface {
        fn drop(&mut self) {
            for buf in &self.bufs {
                unsafe { libc::munmap(buf.map.cast(), buf.len) };
            }
        }
    }

    impl Surface for KmsSurface {
        fn size(&self) -> (u32, u32) {
            (self.w, self.h)
        }

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

        fn settle(&mut self) -> Result<()> {
            if self.flipping {
                flipped(&self.card).context("flip")?;
                self.flipping = false;
            }
            Ok(())
        }
    }
}
