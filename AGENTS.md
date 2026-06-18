# AGENTS.md — Testing & Verification Strategy

## Two-sided testing

Every E2E run validates migration correctness from **both sides**:

| Side | What | Where | Executes |
|------|------|-------|----------|
| **In-VM** | `verify_migration()` in the migrator binary | `src/migration/mod.rs` | Inside QEMU, after Phase 5 |
| **Host-side** | `.raw` disk image scan | `tests/run-e2e.sh` (TODO) | On the CI/laptop host |

The **in-VM** check runs immediately after bootloader setup, before the
reboot. It verifies the migration binary's own view of the world.

The **host-side** scan mounts `disk.raw` from outside the VM and inspects
the actual bytes on disk. This catches filesystem-level bugs the VM can't
see (e.g. 0-byte initrd written through registry extraction, ESP not
mounted, BLS entries on wrong partition).

## What verify_migration checks (in-VM)

See `fn verify_migration()` in `src/migration/mod.rs`:

1. **`.origin` file** — exists in `/sysroot/state/deploy/<verity>/`, valid
   INI format matching bootc's schema.
2. **vmlinuz** — found in ESP or `/boot`, has `MZ` magic, non-zero size,
   not all-zeros.
3. **initrd** — found in same boot dir as vmlinuz, non-zero size. If LVM
   root is detected, also checks for `dm-mod` / `dm_mod` inside the cpio.
4. **BLS entries** — at least one `bootc_*.conf` file in
   `/boot/efi/loader/entries/`, `/efi/loader/entries/`, or
   `/boot/loader/entries/` that contains `linux ` and `composefs=`.

Failures here mean the migration is incomplete — no reboot.

## What the E2E script verifies (in-VM, post-reboot)

After reboot into composefs, `tests/run-e2e.sh` checks:

- `bootc status` returns `composefs` backend
- `/var` data persisted (test fixtures)
- `/home` user directories intact
- `/etc` custom configs preserved through 3-way merge
- Full-fat user state (wallpaper, GNOME extensions, flatpak, dconf,
  Homebrew prefix)
- OSTree rollback: BootOrder swap → boot OSTree → swap back → boot
  composefs
- Commit subcommand: `/sysroot/ostree` removed, OSTree BLS dropped, GRUB2
  cleaned from `/boot`, composefs store intact
- Post-commit diff against fresh Dakota container image

## Host-side .raw scan (to implement)

After migration completes inside the VM, before reboot:

```bash
# 1. Shut down QEMU cleanly
ssh root@localhost reboot
wait_for_qemu_exit

# 2. Mount the disk image
LOOP=$(sudo losetup --show -f -P disk.raw)
ESP=${LOOP}p2   # or find by PARTLABEL=EFI-SYSTEM
ROOT=${LOOP}p3  # or find by FILESYSTEM

sudo mount $ESP /tmp/mnt-esp
sudo mount $ROOT /tmp/mnt-root

# 3. Assertions
# a) vmlinuz on ESP: exists, MZ magic, >0 bytes
VMLINUZ=$(find /tmp/mnt-esp/EFI/Linux/bootc_composefs-*/vmlinuz)
[ -n "$VMLINUZ" ] || die "no vmlinuz on ESP"
SIZE=$(stat -c%s "$VMLINUZ")
[ "$SIZE" -gt 0 ] || die "vmlinuz is 0 bytes"
MAGIC=$(xxd -l2 -p "$VMLINUZ")
[ "$MAGIC" = "4d5a" ] || die "vmlinuz has bad magic: $MAGIC"

# b) initrd on ESP: exists, >0 bytes (not empty)
INITRD=$(find /tmp/mnt-esp/EFI/Linux/bootc_composefs-*/initrd)
[ -n "$INITRD" ] || die "no initrd on ESP"
ISIZE=$(stat -c%s "$INITRD")
[ "$ISIZE" -gt 0 ] || die "initrd is 0 bytes (registry extraction failed)"

# c) systemd-boot EFI present
[ -f /tmp/mnt-esp/EFI/systemd/systemd-bootx64.efi ] || \
  die "systemd-bootx64.efi missing"

# d) .origin file valid INI
ORIGIN=$(find /tmp/mnt-root/sysroot/state/deploy/ -name '*.origin')
[ -n "$ORIGIN" ] || die "no .origin file"
grep -q 'container-image-reference' "$ORIGIN" || die ".origin invalid"

# e) composefs objects/images present
[ -d /tmp/mnt-root/sysroot/composefs/objects ] || die "no composefs objects"
[ -d /tmp/mnt-root/sysroot/composefs/images ] || die "no composefs images"

# 4. Cleanup
sudo umount /tmp/mnt-esp /tmp/mnt-root
sudo losetup -d $LOOP
```

## CI matrix

| Scenario | Base | Target | Filesystem | --skip-import |
|----------|------|--------|------------|---------------|
| btrfs + composefs | bluefin:stable | dakota:stable | btrfs | yes |
| xfs + loopback | bluefin:lts | dakota:stable | xfs | yes |

Both run on every PR → main. See `.github/workflows/e2e-tests.yml`.

## Local development

```bash
# BTRFS test (Bluefin stable → Dakota)
just e2e

# XFS test (Bluefin LTS → Dakota, loopback workaround)
just e2e-lts

# Run linters (shellcheck + cargo fmt --check + clippy)
just lint
```

The `just e2e` recipe builds (debug), runs unit tests, then executes
`tests/run-e2e.sh` with env-var-driven image/filesystem selection.

In CI: `PROFILE_FLAG=--release` gives release builds; the `CI` matrix
sets `BASE_IMAGE`, `TARGET_IMAGE`, `FILESYSTEM`, and `SKIP_REGISTRY=true`
so the VM pulls directly from ghcr.io (no local registry overhead).

## Common failure modes

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `vmlinuz not found in ESP or /boot` | ESP not mounted at `/boot/efi` inside VM after Phase 5 | `find_esp_device()` + temp mount in verify_migration |
| `initrd is 0 bytes` | Registry extraction succeeded but wrote empty file (tar found the path but no content) | Check `extract_one_from_layer` returned bytes; add host-side .raw scan |
| `podman cp for kernel modules failed` | Disk full inside VM (14 GB loopback + 6 GB composefs + podman storage > 20 GB) | Skip initrd rebuild when free space < 6 GB; use registry streaming instead |
| QEMU killed during migration | `set -e` in E2E script triggered by failed pipeline | Use robust tail-log approach instead of SSH-pipe+awk |
| `pull_image` fails with "Invalid transport" | Image ref has `:` from port number, treated as transport | Only treat `://` as transport prefix; always prepend `docker://` for registry refs |
