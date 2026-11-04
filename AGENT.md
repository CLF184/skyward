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

Found by comparing against YARP's live DRM state. All of these matter:

| What | Value | Why |
| --- | --- | --- |
| Format | `XB24` (`Xbgr8888`), first in `SCANOUT` | what the phone's own recovery hands over |
| Planes | one per layer mixer, 720 wide each | `sde_dualpipe_dscmerge` topology |
| `bpp_mode` | `2` (12 bits a pixel), from `mode_properties` | driven at 8 the panel decompresses with the wrong arithmetic |
| `dsc_mode` | `dsc_enabled` | the mode is compressed |
| `COLOR_ENCODING` | `ITU-R BT.601 YCbCr` | otherwise the plane hands pixels over unconverted |
| `COLOR_RANGE` | `YCbCr limited range` | same |

`bpp_mode`, `COLOR_ENCODING` and `COLOR_RANGE` are **enumerations on the
connector/plane**, whose choice names are the driver's business — set them
through `enum_value`/`enum_choice`, which fall back to matching by value or
position, and skip anything the connector does not offer.

Everything is set in `surface.rs::KmsSurface::open`. `SdeMode::find` reads the
topology, DSC flag and rate out of the connector's `mode_properties` blob.

### Known open problem: colours on the pages

On `haotian` the pages still show blue as magenta and red as near black, and
**this is not yet explained**. What has been ruled out, by measurement:

- The canvas holds exactly the right values (dumped from the running recovery).
- The canvas and the scanout buffer are **byte for byte identical**.
- The DRM state matches YARP's property for property.
- Pure-colour bands drawn through the same `present()` come out **correct**,
  including `(52,130,255)` — which the home page's tiles show as magenta.

That last point is the crux and it does not fit any channel-order or
format explanation. Anyone starting here should get a *photograph* of a test
pattern beside a *photograph* of a page before changing anything.

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
