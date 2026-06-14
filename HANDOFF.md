# OSTree → ComposeFS Migration Tool — Handoff

**Repository:** `hanthor/ostree-composefs-rebase`  
**Goal:** In-place migration from OSTree-booted Bluefin:stable to ComposeFS-booted Dakota:stable via systemd-boot  
**Last updated:** 2026-06-14 (sd-boot now sourced from target image)  

---

## What Works

The migration binary successfully completes all 5 phases end-to-end in E2E testing. The last run produced:

```
=== MIGRATION COMPLETED ===
Primary bootloader: systemd-boot
```

All phases complete without errors:
- **Phase 0**: Free-space check passes
- **Phase 1**: OSTree object import (skipped via --skip-import)
- **Phase 2**: OCI pull from local registry (fast)
- **Phase 3**: ComposeFS EROFS image creation + seal (idempotent on re-runs)
- **Phase 4**: /etc 3-way merge, .origin, .imginfo, /var migration
- **Phase 5**: Bootloader setup (systemd-boot on ESP + GRUB2 fallback on /boot)

## Previously Solved

| Blocker | Resolution | SHA |
|---------|------------|-----|
| Phase 5 silently writes ESP BLS entries with no systemd-boot binary on ESP → VM falls back to OSTree | Preflight `systemd_boot_binaries_present` field added; Phase 5 originally routed to GRUB2 when source binary absent | a4b231a |
| GRUB2 fallback path set `next_entry` via `grub2-reboot` but bootupd's grub.cfg has no `if [ "${next_entry}" ]` block, so the one-shot was silently ignored | Phase 5 now writes `saved_entry` directly via `grub2-editenv` (with `grub2-set-default` as backup) instead of relying on grub2-reboot | _pending_ |
| Required systemd-boot package on source (Bluefin) OS | Phase 5 now lifts `systemd-bootx64.efi` out of the mounted Dakota composefs image and copies it to `<ESP>/EFI/systemd/` + `<ESP>/EFI/BOOT/BOOTX64.EFI`; `efibootmgr --create` registers `Linux Boot Manager`; the source OS no longer needs systemd-boot | _pending_ |

## Current Blocker: verify end-to-end systemd-boot migration on Bluefin → Dakota

The primary migration path now installs systemd-boot from the target image:
- Writes `bootc_*.conf` (composefs default) and `ostree-fallback-0.conf` (Bluefin OSTree) to `<ESP>/loader/entries/`.
- Writes `<ESP>/loader/loader.conf` with `timeout 3` so the user can pick the fallback during evaluation.
- Falls back to the GRUB2 path automatically if the target image doesn't ship systemd-boot.

Need to re-run the E2E and confirm:
1. The VM boots into the composefs entry via systemd-boot.
2. `bootc status` reports the composefs deployment.
3. `bootc-migrate-composefs commit` removes the OSTree fallback from the ESP cleanly.

### Diagnostics to run

- Pre-reboot, on the VM: `ls <ESP>/EFI/systemd/`, `ls <ESP>/EFI/BOOT/`, `cat <ESP>/loader/loader.conf`, `efibootmgr -v | grep -i 'Linux Boot Manager'`, `ls <ESP>/loader/entries/`.
- After reboot: `cat /proc/cmdline` should contain `composefs=<hex>` and the booted loader (visible at `/run/systemd/efi/`) should be systemd-boot.

### Next candidate fixes

1. If `efibootmgr` fails to parse the ESP device path (LVM/dm-crypt), `\EFI\BOOT\BOOTX64.EFI` removable-media path acts as a fallback — confirm firmware picks it up.
2. If target image lacks systemd-boot, the GRUB2 branch should fire automatically; verify the warning message surfaces.
3. The `efibootmgr --create` call inserts at the front of `BootOrder` by default — confirm Fedora\shimx64.efi remains accessible by selecting it from firmware menu if composefs fails.

## E2E Test Infrastructure

### Local Registry (fast pulls)

```bash
# Start registry (one-time)
sudo podman run -d --name e2e-registry --network=host docker.io/library/registry:2

# Cache images (one-time)
sudo podman tag ghcr.io/projectbluefin/bluefin:stable 127.0.0.1:5000/bluefin:stable
sudo podman tag ghcr.io/projectbluefin/dakota:stable 127.0.0.1:5000/dakota:stable
sudo podman push --tls-verify=false 127.0.0.1:5000/bluefin:stable
sudo podman push --tls-verify=false 127.0.0.1:5000/dakota:stable
```

### Run

```bash
cd /var/home/james/dev/ostree-composefs-rebase && \
sudo -E env PATH=$PATH \
  BASE_IMAGE=ghcr.io/projectbluefin/bluefin:stable \
  TARGET_IMAGE=ghcr.io/projectbluefin/dakota:stable \
  ./tests/run-e2e.sh
```

### Optimizations

| Feature | Status | Time Saved |
|---------|--------|------------|
| Podman build cache (base image) | ✅ | ~30s |
| Local registry (target pull) | ✅ | ~20 min → ~30s |
| Disk checkpoint (skip install) | ✅ | ~5 min |
| --skip-import (skip Phase 1) | ✅ | ~10 min |
| Podman system prune (disk cleanup) | Manual | Frees ~100GB |

### Cleanup

```bash
# Kill stale QEMU processes
sudo kill $(pgrep -f 'qemu-system.*disk.raw') 2>/dev/null

# Free disk space
sudo podman system prune -af
rm -f disk.raw disk.raw.pre-migration qemu.log test_key*
```

## Architecture

```
src/
├── main.rs              — CLI: --bootloader, --dry-run, --skip-import, commit subcommand
├── preflight.rs         — System checks: ESP detection via lsblk partition GUID
├── reflink.rs           — FICLONE ioctl
├── ostree.rs            — OSTree repo scanner
├── composefs.rs         — bootc CLI wrappers for OCI operations
├── types.rs             — VerityDigest newtype (bare hex vs sha512: prefix)
├── xattr.rs             — xattr-preserving file/dir copy
├── mergetc.rs           — 3-way /etc merge with symlink support
└── migration/
    ├── mod.rs           — Orchestrator: 5 phases + lock file + mount guard
    ├── kernel_options.rs — composefs= cmdline builder (filters OSTree args)
    ├── os_release.rs    — /etc/os-release reader + BLS filename builder
    └── bootloader/
        ├── mod.rs       — BlsEntry struct
        ├── grub.rs      — GRUB2 operations (stub)
        └── systemd_boot.rs — systemd-boot operations (stub)
```

## Key Design Decisions

1. **VerityDigest newtype** — Prevents sha512: prefix bugs
2. **3-way /etc merge** — Falls back to flat copy on failure
3. **Dual-bootloader setup** — systemd-boot (primary, ESP) + GRUB2 (fallback, /boot)
4. **ESP auto-discovery** — Via lsblk partition type GUID when not auto-mounted
5. **Staged entries** — entries.staged/ → entries/ atomic rename
6. **Lock file** — F_OFD_SETLK at /var/run/bootc-migrate-composefs.lock
7. **MountGuard** — Drop-guard ensures umount on panic
8. **Free-space precheck** — Phase 0 before any mutations
9. **Idempotency** — Phase 3 skips seal if image exists; Phase 4 skips if .origin exists
10. **Local registry** — 10.0.2.2:5000 for fast VM pulls in E2E tests

## Test Suite

55 unit tests, 0 failures. Coverage includes:
- VerityDigest construction/formatting/panics (7)
- xattr-preserving copy + symlinks (5)
- 3-way /etc merge all cases + symlinks (13)
- Kernel option filtering + representative Bluefin cmdline (11)
- os-release parsing + BLS filename construction (8)
- ESP parsing, preflight, BLS rendering, reflink, OSTree scan

## CLI

```
bootc-migrate-compose --target-image <image>
  --force              Skip interactive prompts
  --dry-run            Print actions without executing
  --skip-import        Skip Phase 1 (OSTree object hashing)
  --bootloader <name>  "systemd-boot" (default) or "grub2"
  --skip-preflight     Skip preflight validation

bootc-migrate-composefs commit   # Make composefs permanent after successful boot
```

## Preflight Report Example

```
=== Migration Readiness ===
  ✓ All preflight checks passed.
  - ESP: auto-detected (498 MB free, mounts during migration)
  - ESP ready for sd-boot: Yes (>=150 MB)
  - GRUB tools available: Yes
  - Reflink (CoW) Support: Yes

Bootloader: Will migrate to systemd-boot (ESP ready, NVRAM writable).
```

## Next Steps

1. **Re-run E2E with target-sourced systemd-boot** — Confirm Phase 5 extracts systemd-boot from Dakota, registers NVRAM, and VM boots into composefs on next reboot
2. **Exercise `commit` subcommand** — After verified composefs boot, run `bootc-migrate-composefs commit` and confirm the OSTree fallback is removed from the ESP cleanly
3. **Realistic Bluefin user setup in E2E** — Add a primary `bluefin` user via useradd inside the VM pre-migration, drop `gnome-initial-setup-done` markers, populate dconf/.local/share to mirror a real first-boot state
4. **Post-reboot validation** — Verify /var, /etc, /home persistence after successful composefs boot
