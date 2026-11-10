# piano recovery resources

The `gdisk` binary is extracted from the hardware-tested `twrp-GPT-piano.img`
release in `CLF184/android_device_xiaomi_piano`, tag
`twrp-gpt-41880772739-1`. The `dmctl` used for GPT preparation is taken
unchanged from that image as well.

GPT fdisk source and licensing:

- https://android.googlesource.com/platform/external/gptfdisk/
- https://github.com/CLF184/android_device_xiaomi_piano/tree/feat/twrp-fix-gpt

The `parted` program and its bundled libraries are GNU Parted 3.8, built for
Android/aarch64 by Termux and extracted from these packages:

- https://packages.termux.dev/apt/termux-main/pool/main/p/parted/parted_3.8_aarch64.deb
- https://packages.termux.dev/apt/termux-main/pool/main/libb/libblkid/libblkid_2.42.4_aarch64.deb
- https://packages.termux.dev/apt/termux-main/pool/main/libu/libuuid/libuuid_2.42.4_aarch64.deb
- https://packages.termux.dev/apt/termux-main/pool/main/libi/libiconv/libiconv_1.19.1_aarch64.deb
- https://packages.termux.dev/apt/termux-main/pool/main/n/ncurses/ncurses_6.6.20260307+really6.5.20250830_aarch64.deb
- https://packages.termux.dev/apt/termux-main/pool/main/r/readline/readline_8.3.6_aarch64.deb
- https://packages.termux.dev/apt/termux-main/pool/main/liba/libandroid-support/libandroid-support_29-1_aarch64.deb
- https://packages.termux.dev/apt/termux-main/pool/main/liba/libandroid-posix-semaphore/libandroid-posix-semaphore_0.1-4_aarch64.deb

GNU Parted source and licensing:

- https://www.gnu.org/software/parted/
- https://ftp.gnu.org/gnu/parted/parted-3.8.tar.xz
- SHA-256: `a2b7811f47b0ddb1f7b1d0aa456f7c1270da70708ce231c2fe054c7199eafa63`
