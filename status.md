# XFS Migration Status

## Current Blocker: VM boots OSTree instead of ComposeFS after migration

**Root cause**: Migration uses systemd-boot path (ESP), but OVMF/GRUB boots centos shim → GRUB → which reads from `/boot/loader/entries/`. Composefs BLS entry and kernel+initrd are on ESP only.

**Fix needed**: Even when systemd-boot is installed, ALSO write composefs BLS + kernel+initrd to `/boot/` for GRUB fallback. Mirror what the GRUB2 path already does.

## What works
- ✅ Migration Phases 0-5
- ✅ Initrd rebuild (registry streaming, xfs.ko + mount cpio)
- ✅ verify_migration (in-VM)
- ✅ Host-side .raw scan (vmlinuz 19.6MB MZ, initrd 220MB, systemd-boot, .origin, BLS)

## Attempted fixes for composefs boot
1. ❌ efibootmgr BootOrder — OVMF VARS doesn't persist across QEMU restarts
2. ❌ GRUB saved_entry set in migration — GRUB ignores it (blscfg?)
3. ❌ Direct menuentry in /boot/grub2/grub.cfg — GRUB ignores modified config
4. ❌ Direct menuentry in ESP grub.cfg — file write works, but GRUB still shows old blscfg entries

## Next step
EROFS composefs image mounts during boot (`erofs: mounted...`) but is not used as root — system boots OSTree fallback. Likely cause: ext4 loopback at /sysroot/composefs isn't mounted early enough in initrd for ostree-prepare-root to find the composefs images. The xfs-mount.cpio mount unit may need `Before=ostree-prepare-root.service` ordering.

**Confirmed**: Direct `-kernel` boot with composefs cmdline → EROFS mounts but OSTree is root. Migration artifacts are correct. Boot ordering is the issue.

**Verified working**:
- Migration Phases 0-5 ✅
- Initrd rebuild (xfs.ko + loopback mount unit) ✅ 
- Host-side .raw scan (all 5 checks) ✅
- EROFS image at /sysroot/composefs/images/ ✅
- EROFS mounts during boot ✅
- Not used as root ❌
