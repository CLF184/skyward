#!/system/bin/sh
set -eu

dmctl=$1
if [ -b /dev/block/mapper/userdata ]; then
    "$dmctl" delete userdata || exit 1
fi
while read -r source target rest; do
    case "$source" in /dev/block/*) umount "$target" || exit 1 ;; esac
done < /proc/mounts
sync
