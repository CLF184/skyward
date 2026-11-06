# OMAPI service for haotian

haotian's weaver HAL is Thales's, and it reaches the secure element through
`android.se.omapi.ISecureElementService/default`. On Android that service lives
in system_server; a recovery runs no framework, so without this folder the
weaver answers every call with its own error 1 and a lock-screen secret behind
a weaver slot cannot be read (the data stays encrypted).

`skyward-pack recovery --omapi resources/omapi` puts these files into the
ramdisk along with `crates/skyward-pack/src/omapi.rc`, which starts the service
at late-init. The packer takes only the files the stock ramdisk lacks; the
other libraries se_omapi loads (libbinder_ndk, libbinder, libbase, libutils,
libcutils, liblog) come from the stock recovery itself.

| File | Source |
| --- | --- |
| `system/bin/se_omapi` | YARP's sm8750 recovery ramdisk |
| `system/lib64/android.se.omapi-V1-ndk.so` | YARP's sm8750 recovery ramdisk |
| `system/lib64/android.hardware.secure_element-V1-ndk.so` | YARP's sm8750 recovery ramdisk |
| `system/lib64/libvndksupport.so` | YARP's sm8750 recovery ramdisk |

se_omapi sha256: `DC9AB7085A958CE673030FEC6FC185F41BB15B56CE0706BED1132F4A55BD7D2F`.
