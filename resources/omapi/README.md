# OMAPI service for haotian

haotian's weaver HAL is Thales's, and it reaches the secure element through
`android.se.omapi.ISecureElementService/default`. On Android that service lives
in system_server; a recovery runs no framework, so without this folder the
weaver answers every call with its own error 1 and a lock-screen secret behind
a weaver slot cannot be read (the data stays encrypted).

`skyward-pack recovery --omapi resources/omapi` puts `system/bin/se_omapi` into
the ramdisk and `se_omapi.xml` into `system/etc/vintf/manifest/`. The service
directory refuses to register a name that no description declares, so the
description is required.

No init rule starts se_omapi. `devices/haotian.toml` starts it as a service
after the secure element HAL and before the weaver, with
`LD_LIBRARY_PATH=/system/lib64:/vendor/lib64`. It loads
`android.se.omapi-V1-ndk.so` and `android.hardware.secure_element-V1-ndk.so`
from the vendor partition and libbinder_ndk and below from the stock recovery.
YARP's own copies of those two libraries are newer than the stock
libbinder_ndk and fail with `cannot locate symbol
"AIBinder_Class_setTransactionCodeToFunctionNameMap"`, so they are not shipped.

Tested by hand on haotian in Skyward: se_omapi got `eSE1` from the secure
element HAL with the vendor libraries. It then stopped only because the
description was missing (`Could not find android.se.omapi.ISecureElementService/default
in the VINTF manifest`).

| File | Source |
| --- | --- |
| `system/bin/se_omapi` | YARP's sm8750 recovery ramdisk |
| `se_omapi.xml` | YARP's sm8750 recovery ramdisk, `system/etc/vintf/manifest/se_omapi.xml` |

se_omapi sha256: `DC9AB7085A958CE673030FEC6FC185F41BB15B56CE0706BED1132F4A55BD7D2F`.
