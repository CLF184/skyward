# piano recovery resources

The `gdisk` binary is extracted from the hardware-tested `twrp-GPT-piano.img`
release in `CLF184/android_device_xiaomi_piano`, tag
`twrp-gpt-41880772739-1`. The `dmctl` used for GPT preparation is taken
unchanged from that image as well.

The HyperOS 3 Weaver service is the unchanged `prebuilt/weaver-service-hos3`
from commit `8aa37fc` of the same device tree.

The `touch_report` binary and its libraries are extracted from the same tested
recovery image and are used unchanged. Their SHA-256 values are checked by
`skyward-pack` before they are added to a piano image.

GPT fdisk source and licensing:

- https://android.googlesource.com/platform/external/gptfdisk/
- https://github.com/CLF184/android_device_xiaomi_piano/tree/feat/twrp-fix-gpt

Xiaomi ODM touch binaries remain subject to their original vendor terms.
