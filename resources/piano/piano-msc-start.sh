#!/system/bin/sh
set -e

g=/config/usb_gadget/g1
trap 'status=$?; if [ "$status" -ne 0 ]; then
    set +e
    if [ -n "$(cat "$g/UDC")" ]; then echo "" > "$g/UDC"; fi
    rm -f "$g/configs/b.1/mass_storage.0"
    echo "" > "$g/functions/mass_storage.0/lun.0/file"
    setprop sys.usb.config adb
fi' EXIT

setprop sys.usb.config none
retries=0
while [ "$(getprop sys.usb.state)" != none ]; do
    retries=$((retries + 1))
    [ "$retries" -lt 50 ] || exit 1
    sleep 0.1
done
if [ -n "$(cat "$g/UDC")" ]; then echo "" > "$g/UDC"; fi
setenforce 0
echo 0x2717 > "$g/idVendor"
echo 0x9039 > "$g/idProduct"
echo 0xEF > "$g/bDeviceClass"
echo 0x02 > "$g/bDeviceSubClass"
echo 0x01 > "$g/bDeviceProtocol"
mkdir -p "$g/functions/mass_storage.0"
echo 0 > "$g/functions/mass_storage.0/lun.0/ro"
echo 0 > "$g/functions/mass_storage.0/lun.0/removable"
echo /dev/block/sda > "$g/functions/mass_storage.0/lun.0/file"
ln -s "$g/functions/mass_storage.0" "$g/configs/b.1/mass_storage.0"
setprop ctl.start adbd
retries=0
while [ "$(getprop sys.usb.ffs.ready)" != 1 ]; do
    retries=$((retries + 1))
    [ "$retries" -lt 50 ] || exit 1
    sleep 0.1
done
ln -s "$g/functions/ffs.adb" "$g/configs/b.1/f1"
echo a600000.dwc3 > "$g/UDC"
