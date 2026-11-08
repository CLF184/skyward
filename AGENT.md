# Working on Skyward

Notes for whoever picks this codebase up next — human or agent. They are the
things this project's own history cost the most to learn.

## What this is

A recovery written in Rust for Xiaomi phones, built on top of each phone's
**stock recovery ramdisk**: the stock init, adbd, SELinux policy and libraries
stay, and `system/bin/recovery` becomes Skyward's own binary. It is not a
from-scratch ramdisk.

```
cargo ndk -t arm64-v8a --platform 30 build --release -p skyward   # the phone
cargo build --release -p skyward-pack                            # the packer
```

The packer reads the stock recovery out of the phone maker's full update
package and builds an image on top of it. `devices/<codename>.toml` describes
each phone: where its stock build is, which hardware services to start, how the
display and the motor are wired.

## The rule that matters most

> **A diagnostic may never be able to fail the feature it is describing.**

This codebase learned it the hard way. Writing a long line to `/dev/kmsg`,
writing an 18 MB frame to a small partition, and a `?` on either of them put
this phone into a loop of lighting its panel and stopping — from code that was
only there to explain a colour. Twice.

So: diagnostics write with `let _ =`, and never in a path whose failure ends
`run()`. `skyward_sys::kmsg` splits a line that is too long for the kernel log
into several and never refuses one, so the length of a message cannot fail a
caller either.

## Verifying anything about the display

`/dev/dri/card0` is held by the running recovery. That means:

- **A second program cannot open it.** `recovery kms`, `recovery capture` and
  any `skyward …` that touches DRM will fail with `EPERM` while the pages are
  up, or stop the recovery to get the card — and the recovery's USB
  configuration carries adbd, so stopping it takes adb with it.
- **Read the state instead.** With debugfs mounted
  (`mount -t debugfs none /sys/kernel/debug`), everything the panel was told
  is in `/sys/kernel/debug/dri/0/`: `state` for what each plane is doing,
  `framebuffer` for the buffer's format and layout, and one directory per
  connector and plane.
- **Compare against a known-good recovery.** YARP (a TWRP build for the same
  phones) drives this hardware correctly. Its ramdisk can be unpacked and its
  `platform/<soc>/init/*.rc` read, and its live DRM state read the same way.
  Most of the display fixes here came from diffing the two.

### Where things may be written

`/tmp` and `/sdcard` are **memory** in a recovery: anything left there is gone
by the time the pages come back after a start. `/data` may not be mounted at
all (it is encrypted). `/mnt/vendor/persist` is a real partition but **small —
a few megabytes**. An 18 MB frame does not fit, and filling it makes the
recovery fail on every start.

If a file has to outlive a start, it goes on persist as a small marker only.

## What the display needs on `haotian` (Xiaomi 15 Pro, sm8750)

Found by comparing against YARP's live DRM state and against what the phone's
own recovery writes. The format, the planes and the subpixel block below are
what all three have in common. Beyond `dsc_mode` and the tear signal below the
connector's own settings are left to the driver: the phone's own recovery writes
no connector property at all, and the driver works them out at boot from the
panel the device tree describes, which is the state the boot splash is drawn in.

| What | Value | Why |
| --- | --- | --- |
| Format | `XB24` (`Xbgr8888`), first in `SCANOUT` | what the phone's own recovery hands over |
| Planes | one per layer mixer, 720 wide each | `sde_dualpipe_dscmerge` topology |
| `dsc_mode` | `dsc_enabled` | the mode is compressed; set through `enum_value`, skipped where the connector offers no such property |
| `frame_trigger_mode` | `posted_start` | frame starts posted to the hardware, the driver's default on this generation and what Android's compositor sets |
| `bpp_mode` | not set, the driver's own | the recovery whose colours are right never sets it |
| `COLOR_ENCODING`, `COLOR_RANGE` | not set, the driver's own | same; the two writes were an experiment, and the plane state says the driver puts them there itself |
| `SDE_SPR_INIT_CFG_V2` | the block `spr::setup` makes | see below |

Everything is set in `surface.rs::KmsSurface::open`. `SdeMode::find` reads the
topology and the DSC flag out of the connector's `mode_properties` blob.

### The pages' colours: the panel's subpixels

The fault is the panel's subpixel layout, and it is answered by telling the
display controller about it.

The panel in `haotian` is described in its device tree as
`xiaomi o2 42 02 0a cmd mode dsc dsi panel`, and it declares itself a pen tile
one: `qcom,spr-pack-type = "pentile"` and
`qcom,spr-pentile-pack-type = "BG-RG Type B"`. Its glass renders no subpixels
of its own — red and blue dots sit on one diagonal grid with a green dot
between each pair — so it is the display controller that spreads each pixel's
colour over the dots that are really there. The bootloader says a panel like
that is here in `ro.boot.display_ap_spr=1`, and the stock recovery's UI library
reads exactly that: when it is set, it builds Qualcomm's
`drm_msm_spr_init_cfg` and writes it as a property blob into the CRTC's
`SDE_SPR_INIT_CFG_V2` (the `_V1` of an older driver) in the same commit that
sets the mode. Skyward wrote no such block, so the controller ran its subpixel
renderer without being told the layout, and the dots were lit as plain RGB: the
red dots and the blue dots take the same component of the buffer, which is the
`(R, G, B)` arriving as `(B, G, B)` rule measured off the photographs.

`surface.rs` writes the block, in `mod spr`: it reads the connector's
`capabilities` blob, and a panel that names `spr_pack_type=pentile` there is one
whose subpixel rendering is the controller's. The block is Qualcomm's
`drm_msm_spr_init_cfg` laid out field for field, written into
`SDE_SPR_INIT_CFG_V2` (the `_V1` of an older driver takes the same block without
its last two fields) on the mode commit and again on the switch to the fastest
refresh, since that is a mode set of its own. A panel that names no layout is
left alone, so nothing is written for a phone whose glass renders its own
subpixels. That gate is upstream's (`d7fefd7`).

The figures in the block are the panel's: `cfg13` = `0x155`, every other dot of a
line; `cfg15`, the ramp the twelve bits a pixel are spread over; and the filter
taps `cfg16`/`cfg17`, which decide how much of a pixel each dot takes. The taps
are the part that had to be right, and they were read back out of two libraries:
`target/v013-haotian/system/lib64/librecovery_ui.so` (builder at 0x376b0 for
`V2`, 0x37750 for `V1`) and
`target/yarp-root/system/lib64/libtwrpminui.so` (builder at 0x237f8). Both carry
the same struct and byte identical tables, but they pick different taps: the
stock library keys them on `ro.boot.hardware.sku` and gives `haotian` the set
that weighs a pixel's red and blue alike (`cfg16` = `[0, 256, 256, 0]` four
times, `cfg7` = 5), while TWRP keys them on nothing and writes the set that
keeps the two apart (`cfg16` = `[128, 256, 128, 0, 86, 241, 164, 21, 52, 204,
204, 52, 21, 164, 241, 86]`, `cfg17` = `[-2, 2, …, 2, -2, …]`, `cfg7` = 2). The
block written is TWRP's, which is also what upstream's fix writes.

Whether it took is in the summary the surface writes to the kernel log once it is
up: `…, planes [97, 129] …, link dsc_mode=1,bpp_mode=…,frame_trigger_mode=…,
sub-pixel rendering SDE_SPR_INIT_CFG_V2, 304 bytes held`. `sub-pixel rendering -`
means the panel named no layout; `nothing held` means a block was asked for and
the read-back of the CRTC did not find it; `link` is what the driver chose for
the compressor and the tear signal.

The two builds before the merge are part of why the block is right, and are
worth remembering. The first one to write a block wrote the stock library's
`haotian` values (`c965c23`): the magenta went, but photographs showed every
saturated colour as the average of its red and blue — the red "wipe" icon
(242, 83, 75) and the orange "reboot" icon (255, 146, 47) both came out purple,
while green, grey, white and black came through — which is what the stock
library's `cfg16` box average does. The second (`996f39d`) wrote TWRP's taps,
the same figures upstream's block writes, and was never photographed before
upstream's fix arrived and the merge took it.

What has been ruled out, should the colours still need work: the SDE register
dumps cannot tell these builds apart (the `spr_*` sub-blocks are byte identical
in Skyward's, YARP's and the writing build's captures, and YARP differs from
itself between two captures by as much as it differs from Skyward), so the dump
holds the bootloader's leftovers rather than what a blob writes. The connector's
`bpp_mode` and the plane's `COLOR_ENCODING`/`COLOR_RANGE` writes are gone as
well, on the strength of the phone's own recovery making none of them.

What follows was measured while the fault was still unexplained, and is kept
because it still holds: every frame was wrong from the very first one Skyward
put on the glass, so no later flip, stale buffer or hand-over between two
buffers accounts for it; the canvas and the scanout buffer were **byte for
byte identical** at that hand-over; and the system's own screenshot — a
readback of the framebuffer, not a photograph — was correct, so the values were
right at least as far as the memory the display controller reads. A fault
downstream of the buffer and upstream of the glass is what a subpixel layout
the controller had not been told looks like.

What the glass did with them was read off photographs of two pages. The
rule that fits is this: the panel's red and its blue both take the buffer's
blue byte, and its green takes the green one — `(R, G, B)` in the buffer
arrives as `(B, G, B)`. That is why blue turns magenta, why red keeps only the
green it had and reads as near black, why green, white and black come through
unchanged, and why `(52,130,255)` on the home page's tiles shows as magenta.
`target/color-debug/panel-rule.py` holds the pairs and checks the rule against
all eight of them. Seven agree. The eighth is the text ink, black in the buffer
and black on the glass, where the two disagree only about what to call it.

**An earlier version of this section claimed pure-colour bands came out
correct. That was wrong, and it is worth saying so plainly, because it sent an
attempt in the wrong direction.** A later photograph of the terminal page's
ANSI colour bars — green, magenta, white, cyan, blue and grey bands, put on
screen by the terminal's own drawing rather than by a hand-built test pattern
— showed those bands arriving wrong as well, and the rule above explains them.
So the fault is neither about pages as opposed to patterns nor about a colour
blended over another as opposed to one written straight in; the experiment
that distinguished those (`660e497`) has no answer to give. Every frame is
affected, and so is every way of drawing into one.

Ruled out, by measurement:

- **The display controller's registers.** A full SDE dump, through
  `/sys/kernel/debug/dri/0/debug`: set `panic` to 0 first, since a dump with
  panic on can crash the kernel, then `echo 1 > dump` and read
  `recovery_reg`. Skyward's dump and YARP's are identical word for word
  except in table RAM — the DSPP blocks, the blur LUT and the gamut blocks —
  and a handful of interface counters. Two captures of YARP's own running
  recovery differ from each other by as much, in the same places. Its `spr_*`
  sub-blocks are identical as well — 128 words each, first word `0x20010000`
  — and they are *still* identical in the build that writes the subpixel block,
  so the dump is not reading back what a blob writes. It holds what the
  bootloader's splash left there: the pattern word at `0x6a470`,
  `0x15555555`, is three `0x155` groups, which is the register a block writes
  only when its `cfg3` is non-zero, and Skyward's block carries `cfg3 = 0` and
  would not touch it. Do not try to read the block out of this dump.
- **The DSI controller's registers, and the porch timings**, read the same way.
- **The panel's DCS registers**, read back through
  `/proc/mi_display/mipi_rw_prim` and compared with YARP's; nothing in what
  the panel reports explains a channel swap. Sending the panel its whole
  on-command set again (221 of them) and its PPS changed nothing.
- **The backlight path**, all three of them: the `panel0-backlight` sysfs
  node, `mi_display`'s `brightness_clone`, and a direct DCS `0x51`.
- **The kernel.** `/proc/version` and `msm_drm.ko` are identical to YARP's,
  and the loaded module set differs only in the wifi, audio, camera, nfc, gpu
  and video modules.
- **How the panel is started.** Both recoveries come up through the
  bootloader's splash handoff (`cont_splash`), with no panel on-command at
  start.

Tried, and none of it moved the colours:

- `bpp_mode` taken from the mode's own `mode_properties` (`2e55328`).
- `COLOR_ENCODING` and `COLOR_RANGE` (`e902e11`); on this phone the planes do
  not expose them, so there is nothing to set.
- `XRGB8888` first (`b930c31`), a resolution the mode does not compress
  (`eac6dd8`), one plane across the whole width (`9fc8682`).
- Leaving `dsc_mode`, `bpp_mode` and `frame_trigger_mode` to the driver
  (`9148774`, reverted in `b7e94e4`).

Moved them:

- **The subpixel block** (`c965c23`). The first build to write it lost the
  magenta and the near-black red in one go, which is why the block is the answer
  to the original fault. What is left over is a different question.
- **The filter taps inside that block.** The first build carried the stock
  library's `haotian` taps, which weigh a pixel's red and blue alike; the build
  after it carries TWRP's, which keep them apart. Whether that is the whole of
  the remaining fault is the open measurement.
- The connector's `bpp_mode` and `frame_trigger_mode` writes and the plane's
  `COLOR_ENCODING`/`COLOR_RANGE` writes are now gone as well, on the strength of
  the phone's own recovery making none of them. Whether that moves the colours
  is the open measurement.

The one difference not tried is the way Skyward presents. Skyward sets the
panel up in its preferred mode and then switches it to 120 Hz; it draws into
three dumb buffers in turn, and flips with a non-blocking commit that waits for
the flip event. YARP sets the target mode in a single commit and blocks on it.
None of that has been flashed and measured — a control build shaped like YARP
was designed and never went on the phone. It matters only if the subpixel block
above turns out not to be the whole answer: a difference in how the panel is
driven, rather than in what it is given, is then the last thing measurable from
outside the panel and the next place to look.

The evidence is in `target/color-debug/`: the canvas and scanout dumps
(`frame-*`, `live-*`, `sw-*`, `canvas-new` and `scanout-new`), the DRM state
snapshots, `yarp-drm/` for YARP's side, `regs/regs-skyward`,
`regs/regs-yarp-on` and `regs/regs-yarp-idle` for the register dumps, and
`panel-skyward.txt` beside `panel-yarp.txt` for the DCS readbacks. The scripts
that took them are `regs.sh` and `panel.sh` at the top of the tree, and
`target/lutdiff.py` and `target/unpack_img.py` for reading the dumps back.
Reading the stock library's own block back out is `target/spr_scan.py` (which
files of the two ramdisks carry the strings), `target/spr_xref.py` (where the
code that reads a string is), `target/spr_dis.py` (a window disassembled) and
`target/spr_dump.py` (a piece of the file as bytes and as text); it needs
`capstone` for the disassembly. `target/propnames.py` and
`target/propnames2.py` list every property name the two UI libraries contain at
all — the shortest way to see what the phone's own recovery does and does not
write — `target/dexnames.py` prints every capitalised identifier in the stock
library, `target/blocks.py` and `target/regdiff.py` read and compare register
dumps. None of it is committed — `target/` is a build directory — so it exists
only on the machine that measured it.

## The credential services

`gatekeeper`, `keymaster`/`keymint`, the secure element and the weaver are
started by Skyward itself from `[[service]]` entries in the device's toml —
not by init, so the stock `.rc` files' `disabled`/`enable` rules do not apply.
The same goes for the two programs the weaver's transport needs and no
recovery's init runs: `ssgtzd` and the OMAPI service, both below.

- **`gatekeeper` must run as `root`** on `haotian`. The stock rc says `system`,
  but that rc only applies once init has enabled the service by property;
  Skyward starts the binary directly, and as `system` the trustlet behind
  `libqtigatekeeper` cannot be reached — every call comes back as *"the secure
  world took the credential but wrote no note for the key service"*. YARP runs
  it as root on the same phone.
- A phone whose weaver is a **Thales** secure element reaches it through
  Android's OMAPI service, which lives in `system_server` and which a recovery
  does not run. `skyward-crypto/src/blobs.rs` handles the phone that keeps a
  second blob: HyperOS writes one bound to `secdis` beside the weaver-bound
  one, and the recovery opens that instead whenever it is there. **The
  decisions there are testable on a computer** — that module exists so that
  they are.
- **The weaver itself now answers, so a blob that is only weaver-bound opens
  too.** Three things were missing on `haotian`, and each of them alone left
  every call to the weaver answering with the service's own error 1:
  - **`ssgtzd` must run.** The TEE's trusted apps wait for a mink service; with
    none listening the secure element's `TEEC_OpenSession` returns
    `0xFFFF0000`, so the element never opens and `se_omapi` reports
    `isSecureElementPresent: 0`. The phone's own init starts it at early-boot
    with the two listeners below; a recovery starts no such init, so
    `devices/haotian.toml` starts it. Measured: killing it and letting the HAL
    come up again brings "error 1" straight back.
  - **The eSE's drivers must be in.** `/dev/st54spi_gpio` comes from
    `stm_st54se_gpio.ko`, with `stm_nfc_i2c.ko` beside it; without them the HAL
    logs `eseGetVendorId: Unknown eSE HW` and registers a reader with no card.
    YARP loads both in recovery. They are in `[modules].load`.
  - **The secure element's HAL runs as `root`**, as YARP's init runs it.
- The **ADSP** is the other wall the same HAL hits, and it has two separate
  faults worth telling apart. The kernel boots the DSP itself while the
  modules load, so a phone whose firmware it can read says `running` within a
  second and nothing has to be written. But **writing `boot_adsp/ssr` takes
  the DSP down through `rproc_shutdown` and hangs the writer in the ADSP
  driver's shutdown path** — the recovery process sat in `D` state in
  `spf_core_exit` and the phone had to be forced out of recovery. Never write
  `ssr`, and never `boot=0`. `boot=1` is the safe one: offline it reloads and
  boots the DSP, and on a running one `rproc_boot()` returns early. The
  recovery's firmware search ends with `/firmware/image`, a folder it makes by
  showing the modem partition there, and on a phone whose `/vendor/firmware`
  holds only the first part of the DSP's firmware (`adsp.b28` missing) that is
  the only place the loader can still find the rest — so `main.rs` waits three
  seconds and then asks the loader again by writing `boot_adsp/boot=1`, never
  on a DSP that is already running.
- A service's **VINTF description has to name a version the recovery's own
  `libvintf` knows**. `servicemanager` reads the whole folder and drops *every*
  description over one it cannot parse, so a single description written for a
  newer platform leaves the recovery with no manifest at all: nothing
  registers — not keymint, not gatekeeper, not the weaver — and the only trace
  is `-22 VINTF parse error: Illformed file: … Unrecognized manifest.version
  9.0 (libvintf@8.0)` followed by `NULL VINTF MANIFEST!`. `as_framework` in
  `crates/skyward/src/secure.rs` therefore writes `version="1.0"` on every
  description it publishes, and `resources/omapi/se_omapi.xml` ships that
  version rather than the one the AOSP input names.
- A service started with **init's listening sockets** needs that descriptor to
  reach the exec. `services.rs` duplicates each socket close-on-exec so that no
  program the recovery starts is handed one it was not meant to have, and clears
  it in the child's `pre_exec` alone, for the one exec it is for. Left
  close-on-exec, `ANDROID_SOCKET_<name>` names a descriptor the program never
  sees; a program that cannot start without its listener ends at once instead —
  Qualcomm's `hlosminkdaemon` is one — and the kernel log shows
  `… hlosminkdaemon: pid N ended: exit status: 0` every five seconds for as long
  as the recovery runs.
- What the phone kept beside its blobs goes to the kernel log. When a weaver
  binding is involved, look for
  `the blob the phone records is bound to a weaver; the blobs beside it are …`
  and check whether `secdis` is among them. With the weaver answering (see
  above) a blob that is only weaver-bound opens too; where it still does not,
  the line that follows says which of the three walls was hit:
  `the phone's weaver cannot answer` (the transport: `ssgtzd`, the eSE's
  drivers, or the element's HAL), `the secure element does not recognise that
  credential` (the credential), or `the secure element is holding off further
  tries` (the element's own throttling, after repeated wrong tries).

## Lighting the dark panel

Only the power key lit it until 0.2.1. On the phone's own system the same panel
lights on a double tap, and its controller keeps reporting fingers with the
panel off, so the recovery has it too: `tap()` in `skyward-ui/src/app.rs`
counts a tap only while the display is off, and the second one inside Android's
own window (300 ms) and near enough to the first (8% of the screen's width)
lights it. Neither tap works the page — they are one gesture. **A tap is in
screen pixels, so the slop is multiplied by the width**; comparing it against
`0.08` directly makes a double tap impossible, which is how it was first
written.

**A suspended phone cannot report a tap**, so a phone that names
`double_tap = true` in `[touch]` keeps the phone awake while the panel is off —
`may_sleep` in `skyward/src/ui.rs` is false for it. That is the price: the panel
lights without the power key, and the phone does not sleep. Only `haotian` names
it; a phone whose controller stops reporting fingers with its panel off has
nothing for the feature to work with.

## USB and adb

The recovery's USB configuration carries **adbd**. Anything that takes the port
down takes adb with it:

- `sys.usb.config=none` stops adbd (stock `init.rc`).
- The MTP rules in `skyward-mtp.rc` unbind the gadget and re-enumerate, so a
  computer sees a disconnect and reconnect — **about 400 ms, of which 390 ms is
  USB enumeration itself**. It cannot be avoided when the set of functions
  changes; `usb::switch` only skips the `none` step when nothing has to come
  off.

Practical consequence: **do not toggle MTP while debugging over adb.** Use
`adb push`/`adb pull`.

## Building and testing

The workspace is Android-only: it uses `std::os::unix`, `rustix` and Android
binder. **A Windows host cannot build it** — `cargo check` fails in
`rsproperties`/`skyward-block`/`skyward-lp` before reaching any of this code.
Build with `cargo ndk` for `aarch64-linux-android`, or on Linux.

On a Windows host the phone's binary can still be cross-compiled and the image
packed, which is how the releases below were checked before CI:
`cargo ndk -t arm64-v8a --platform 30 build --release -p skyward` (with
`ANDROID_NDK_HOME` set), then `cargo build --release -p skyward-pack` and
`skyward-pack recovery`. `skyward-ui`'s *tests* do not compile there either —
they pull `rsproperties` in — so `cargo test -p skyward-config` and clippy on
the Android target are what can be run locally; the rest is CI's.

Two things the packer used to need and no longer does: it wrote the ramdisk's
legacy LZ4 frames by running the `lz4` program, which a Windows host does not
have (it now writes them itself with `lz4_flex`, and `repack` comparing the
entry lists is the check that it does), and `DeviceConfig::parse` asked
`Path::is_absolute()`, which is false for `/data/x` off Linux (it asks
`has_root()` now).

`skyward-preview` draws every page to PNG on any host, and `skyward-ui`'s tests
run anywhere — that is the fast loop for anything about layout or wording.

CI (`.github/workflows/build-haotian.yml`) builds the image **and** runs
`cargo test --workspace` + `cargo clippy --workspace --all-targets -D warnings`.
Clippy is `-D warnings`, so an unused import or a needless borrow fails the
build; `cargo fmt` habits matter.

A release is a tag: `git push ssh://git@github.com/Rinrroy/skyward-for-haotian.git main v0.2.2`
builds the same image and publishes it, with `SHA256SUMS` beside it, as a GitHub
release named for the tag — the artifact a branch run leaves expires after
thirty days, a release does not. Bump `[workspace.package] version` in
`Cargo.toml` first (and let `cargo metadata` rewrite `Cargo.lock`), and put the
release's own notes in `docs/releases/<tag>.md`; the workflow uses that file
when it exists and the commits since the last release when it does not.

## Reading this repo's history

Commit messages explain *why*, in plain prose, often at length. They are worth
reading before changing a subsystem — several contain the measurements that
settled a question (see `fix(ui): tell the panel how to read the pixels`,
`fix(haotian): run the credential service as root`, and this file's own
subject: `chore(ui): take the colour diagnostics back out`).
