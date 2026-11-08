![Skyward Recovery](assets/hero.jpg)

# Skyward for the Xiaomi 15 Pro

English | [简体中文](README.zh-CN.md)

Skyward is a recovery written in Rust, from scratch. This fork is built for one
phone: **Xiaomi 15 Pro (`haotian`)**, on the Chinese system's OS4.0.0.19.

<p align="center">
  <img src="assets/home.png" width="30%" alt="Home">
  <img src="assets/settings-dark.png" width="30%" alt="Settings in dark mode">
  <img src="assets/time-zone.png" width="30%" alt="Time zone">
</p>

## What this fork adds to upstream

Upstream Skyward is a multi-device recovery. This fork keeps its own
`devices/haotian.toml` only, and adds what decrypting this phone's data
partition turned out to need:

- **The weaver answers, so a data partition that is only weaver-bound opens.**
  A phone whose secret is guarded by the secure element's weaver reaches it
  through three things a recovery does not run by itself, and all three are now
  started or loaded by Skyward:
  - `ssgtzd`, the TEE's mink daemon. Without it the secure element's
    `TEEC_OpenSession` returns `0xFFFF0000` and the element never opens.
  - `stm_st54se_gpio` and `stm_nfc_i2c`, so `/dev/st54spi_gpio` exists at all.
  - The secure element's HAL as `root`, after the ADSP is up.
- **Two taps light the dark panel**, as the phone's own system's double tap
  does. The phone stays awake while the panel is off, since a suspended phone
  is one the touchscreen cannot report a tap to.
- **The ADSP is given three seconds, then asked for a retry.** On a build whose
  `/vendor/firmware` carries only the first part of the DSP's firmware, the
  kernel stops at the first missing segment; Skyward asks the loader again once
  the modem partition is shown at `/firmware`, which is the last folder the
  kernel's own firmware search looks in.

## Features

### Implemented

- Decrypt the data partition with the screen lock's credential, including users
  whose secret is bound to the weaver and has no `secdis` fallback
- Install zips, system update packages and partition images, from internal
  storage, a USB drive or adb sideload
- `7za` and `zstd` for install scripts, and an `unzip` whose `-p` streams the
  image as it reads it
- `avbctl` and `resetprop` for install scripts
- Online system updates
- Virtual A/B updates: read where an update's snapshots stand, map the
  partitions through them, and merge them back
- Skyward's own updates, from the newest release in
  [this repository](https://github.com/Rinrroy/skyward-for-haotian/releases)
- Backup and restore
- Wipe data and factory reset
- Mount partitions, repair and format filesystems
- File manager and terminal
- WLAN
- MTP file transfer, adb and fastbootd
- Reboot to system, recovery, fastboot, bootloader or EDL, and switch slots
- Dual boot: a second system beside the installed one, picked from two cards on
  the home page and switched to with a swipe
- Light and dark themes; Simplified Chinese, Traditional Chinese and English

### Not implemented

- Updates whose snapshots are compressed and served by `snapuserd`: this
  recovery maps and merges kernel snapshots only
- Incremental update packages
- Update packages for phones without A/B slots

## Supported devices

| Device | Codename | Built on | Tested |
| --- | --- | --- | --- |
| Xiaomi 15 Pro | `haotian` | OS4.0.0.19.XOBCNXM (Android 17) | yes |

## Notes on the Xiaomi 15 Pro

These have been tried on the phone and work: the screen (including its subpixel
rendering, which this panel leaves to the display controller), the touchscreen
and its double tap, adb and fastbootd, MTP, the motor, WLAN, and decrypting the
data partition with the screen lock's credential.

The stock build the image is made on is OS4.0.0.19.XOBCNXM.

### If decryption still fails

The kernel log says which of the three walls was hit. Look for the line
`the blob the phone records is bound to a weaver; the blobs beside it are …`,
then:

- `the phone's weaver cannot answer` — the transport: `ssgtzd`, the eSE's
  drivers, or the secure element's HAL.
- `the secure element does not recognise that credential` — the credential.
- `the secure element is holding off further tries` — the element's own
  throttling, after repeated wrong tries.

If the DSP is the fault, the kernel log carries `the ADSP is offline; asked the
loader again now that /firmware is in place`, followed by either a running DSP
or `it is not running, even after the loader was asked again`.

## Installing

Skyward needs an unlocked bootloader. Download the image from
[Releases](https://github.com/Rinrroy/skyward-for-haotian/releases), check it
against `SHA256SUMS`, and flash it to the recovery partition of the slot the
phone runs from, with the phone in fastboot mode:

```sh
sha256sum --check SHA256SUMS
fastboot flash recovery skyward-v0.2.1-haotian-OS4.0.0.19.XOBCNXM.img
fastboot reboot recovery
```

Afterwards Skyward updates itself over Wi-Fi from the newest release **in this
repository**, on the Skyward update page in Settings; a newer image also installs
from its own Install page. With **Flash Skyward again** on, a system update or
zip that writes over the recovery partitions gets Skyward back in them.

The update page asks `api.github.com/repos/Rinrroy/skyward-for-haotian` for its
newest release, and takes the asset named `skyward-<tag>-haotian-<stock>.img` —
the image itself, published as it is rather than zipped. A fork of this fork has
to change `skyward_net::github::LATEST` to its own repository, or the phone will
be offered images built for `haotian` that it cannot use.

Without a system in super, Skyward still starts, with its screen and keys, adb,
`adb sideload` and fastbootd, enough to install one. Touch, WLAN and decryption
run from the installed system and come back with it.

adb in Skyward is a root shell that needs no authorization on the phone: anyone
with the phone and a cable can reach its storage while it is in recovery.

## Dual boot

Skyward installs a second system beside the one in the phone's own partitions
and switches between them from **Dual boot** on its home page. The second
system's root is a file in the data partition that the filesystem may neither
move nor delete, read and written through the partition itself; the boot images
of both systems are kept in another such file. A switch writes only the boot and
init_boot partitions of the slot that starts next, and keeps each step on
`/metadata` first, so a switch cut off by a loss of power is finished when
Skyward next starts. vendor_boot, dtbo, recovery, vbmeta, the firmware and super
stay as they are.

A second system installs from a zip on the Install page:

| File | What it holds |
| --- | --- |
| `system.toml` | `id`, `name`, `version` and `filesystem`; `init`; `minimum`, the least room the root takes |
| `boot.img` | The kernel the bootloader starts |
| `rootfs.img` | The root filesystem's image |
| `init_boot.img` | Only for a system with a first ramdisk of its own |

A system without an init_boot image of its own starts from one Skyward makes: a
statically linked program that loads the installed system's first-stage kernel
modules, finds the root in the data partition, mounts it and starts `init`
there. If anything stops it, it brings Skyward up instead, with the reason.

## Building

Skyward builds with the stable Rust that `rust-toolchain.toml` names, with its
`aarch64-linux-android` target, the Android NDK and
[cargo-ndk](https://github.com/bbqsrc/cargo-ndk). The recovery service and the
second system's first program are built for the phone, and the packer for the
computer:

```sh
cargo ndk -t arm64-v8a --platform 30 build --release -p skyward
RUSTFLAGS="-C target-feature=+crt-static" cargo ndk -t arm64-v8a --platform 30 build --release -p skyward-system-init --target-dir target/system-init
cargo build --release -p skyward-pack
```

The packer takes the stock recovery partition out of the build's full update
package on Xiaomi's server, reading only the part of the package the partition
is in, and checks its SHA-256 against the device file:

```sh
target/release/skyward-pack stock --config devices/haotian.toml --out target/stock/haotian/recovery.img
target/release/skyward-pack stock --config devices/haotian.toml --out target/stock/haotian/recovery.img --partition odm
```

Then it builds the image on top of it:

```sh
target/release/skyward-pack recovery \
  --stock target/stock/haotian/recovery.img \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --config devices/haotian.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --firmware cs40l26.wmfw=stock/firmware/cs40l26.wmfw \
  --firmware cs40l26.bin=stock/firmware/cs40l26.bin \
  --firmware cs40l26-svc.bin=stock/firmware/cs40l26-svc.bin \
  --firmware cs40l26-dvl.bin=stock/firmware/cs40l26-dvl.bin \
  --firmware cs40l26-dbc.bin=stock/firmware/cs40l26-dbc.bin \
  --firmware cs40l26-a2h.bin=stock/firmware/cs40l26-a2h.bin \
  --omapi resources/omapi \
  --out target/skyward-haotian.img
```

The motor's firmware has to be in the ramdisk before init's first probe, and
`--omapi` puts in the OMAPI service the Thales weaver reaches the secure element
through. `.github/workflows/build-haotian.yml` does all of this on a tag and
publishes the image with its checksum.

Tests and lints run on the computer; what runs only on the phone is linted for
it:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo ndk -t arm64-v8a --platform 30 clippy -p skyward -p skyward-install -p skyward-systems -p skyward-system-init -- -D warnings
```

A Windows host can cross-compile the phone's side and run the tests for
`skyward-config`, but neither packs an image (`skyward-bootimg` uses unix-only
calls) nor builds `skyward-ui`'s tests (`rsproperties` does not build there).
Use Linux or CI for those.

`skyward-preview` draws the pages on the computer, as PNG files, without a
phone. `--lang` picks the language by a locale such as `en` or `zh-TW`:

```sh
cargo run --release -p skyward-preview -- target/preview --lang en
```

## Reading this repo

`AGENT.md` is the working notes: what each part costs to learn, the rules a
diagnostic has to keep, and — for this phone — why the weaver, the ADSP and the
panel behave the way they do. Commit messages explain *why*, often at length.

## License

[GPL-3.0-or-later](LICENSE)
