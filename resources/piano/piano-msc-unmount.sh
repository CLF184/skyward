#!/system/bin/sh
set -eu

disk=$(readlink -f "$1")
disk=${disk##*/}

on_export_disk() {
    if [ "$1" = "$disk" ] || [ -f "/sys/class/block/$disk/$1/partition" ]; then
        return 0
    fi
    for slave in /sys/class/block/"$1"/slaves/*; do
        [ -e "$slave" ] || continue
        if on_export_disk "${slave##*/}"; then return 0; fi
    done
    return 1
}

sync
while read -r source target rest; do
    case "$source" in /dev/block/*) ;; *) continue ;; esac
    device=$(readlink -f "$source")
    if on_export_disk "${device##*/}"; then printf '%s\n' "$target"; fi
done < /proc/mounts | sort -r | while read -r target; do
    printf 'Unmounting %s\n' "$target"
    umount "$target" || exit 1
done
sync
echo "All mounted filesystems on $disk were unmounted normally."
