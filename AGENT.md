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

Found by comparing against YARP's live DRM state. The format and the planes are
what both recoveries scan out. The properties under them were set to match
YARP, and changing them did not move the colours at all — the open problem
below says how that was measured.

| What | Value | Why |
| --- | --- | --- |
| Format | `XB24` (`Xbgr8888`), first in `SCANOUT` | what the phone's own recovery hands over |
| Planes | one per layer mixer, 720 wide each | `sde_dualpipe_dscmerge` topology |
| `bpp_mode` | `2` (12 bits a pixel), from `mode_properties` | the rate the mode's own description names |
| `dsc_mode` | `dsc_enabled` | the mode is compressed |
| `COLOR_ENCODING` | `ITU-R BT.601 YCbCr` | what both recoveries' state reads; the planes here expose no such property |
| `COLOR_RANGE` | `YCbCr limited range` | same |

`bpp_mode`, `COLOR_ENCODING` and `COLOR_RANGE` are **enumerations on the
connector/plane**, whose choice names are the driver's business — set them
through `enum_value`/`enum_choice`, which fall back to matching by value or
position, and skip anything the connector does not offer.

Everything is set in `surface.rs::KmsSurface::open`. `SdeMode::find` reads the
topology, DSC flag and rate out of the connector's `mode_properties` blob.

### Known open problem: colours on the pages

On `haotian` the pages still show blue as magenta and red as near black, and
**this is not yet explained**. Every frame is wrong, from the very first one
Skyward puts on the glass: the first photograph of the panel is already of a
mangled picture, so no later flip, stale buffer or hand-over between two
buffers can account for it. That first frame was also dumped on both sides of
the hand-over, and the canvas and the scanout buffer are **byte for byte
identical** — the buffer holds exactly the values the pages drew. The system's
own screenshot — a readback of the framebuffer, not a photograph — is correct
as well, so the values are right at least as far as the memory the display
controller reads.

What the glass then does with them was read off photographs of two pages. The
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
  recovery differ from each other by as much, in the same places.
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

What has **not** been tried is the way Skyward presents. Skyward sets the panel
up in its preferred mode and then switches it to 120 Hz; it draws into three
dumb buffers in turn, and flips with a non-blocking commit that waits for the
flip event. YARP sets the target mode in a single commit and blocks on it. None
of that difference has been flashed and measured — a control build shaped like
YARP was designed and never went on the phone. It is the last difference
measurable from outside the panel that is left, and it is a difference in how
the panel is driven rather than in what it is given, so it is where the next
attempt should start.

The evidence is in `target/color-debug/`: the canvas and scanout dumps
(`frame-*`, `live-*`, `sw-*`, `canvas-new` and `scanout-new`), the DRM state
snapshots, `yarp-drm/` for YARP's side, `regs/regs-skyward`,
`regs/regs-yarp-on` and `regs/regs-yarp-idle` for the register dumps, and
`panel-skyward.txt` beside `panel-yarp.txt` for the DCS readbacks. The scripts
that took them are `regs.sh` and `panel.sh` at the top of the tree, and
`target/lutdiff.py` and `target/unpack_img.py` for reading the dumps back.
None of it is committed — `target/` is a build directory — so it exists only
on the machine that measured it.

## The credential services

`gatekeeper`, `keymaster`/`keymint`, the secure element and the weaver are
started by Skyward itself from `[[service]]` entries in the device's toml —
not by init, so the stock `.rc` files' `disabled`/`enable` rules do not apply.

- **`gatekeeper` must run as `root`** on `haotian`. The stock rc says `system`,
  but that rc only applies once init has enabled the service by property;
  Skyward starts the binary directly, and as `system` the trustlet behind
  `libqtigatekeeper` cannot be reached — every call comes back as *"the secure
  world took the credential but wrote no note for the key service"*. YARP runs
  it as root on the same phone.
- A phone whose weaver is a **Thales** secure element reaches it only through
  Android's OMAPI service, which lives in `system_server` and which a recovery
  does not run. `skyward-crypto/src/blobs.rs` handles this: HyperOS keeps a
  second `spblob` bound to `secdis` beside the weaver-bound one, and the
  recovery opens that instead whenever it is there. **The decisions there are
  testable on a computer** — that module exists so that they are.
- What the phone kept beside its blobs goes to the kernel log. When a weaver
  binding is involved, look for
  `the blob the phone records is bound to a weaver; the blobs beside it are …`
  and check whether `secdis` is among them. Without it the user's files cannot
  be opened in a recovery at all, and no code change will help.

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

`skyward-preview` draws every page to PNG on any host, and `skyward-ui`'s tests
run anywhere — that is the fast loop for anything about layout or wording.

CI (`.github/workflows/build-haotian.yml`) builds the image **and** runs
`cargo test --workspace` + `cargo clippy --workspace --all-targets -D warnings`.
Clippy is `-D warnings`, so an unused import or a needless borrow fails the
build; `cargo fmt` habits matter.

## Reading this repo's history

Commit messages explain *why*, in plain prose, often at length. They are worth
reading before changing a subsystem — several contain the measurements that
settled a question (see `fix(ui): tell the panel how to read the pixels`,
`fix(haotian): run the credential service as root`, and this file's own
subject: `chore(ui): take the colour diagnostics back out`).
