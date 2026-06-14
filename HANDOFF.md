# OSTree → ComposeFS Migration Tool — Handoff

**Repository:** `hanthor/ostree-composefs-rebase`  
**Goal:** In-place migration from OSTree-booted Bluefin:stable to ComposeFS-booted Dakota:stable  
**Date:** 2026-06-14  

---

## Architecture

```
bootc-migrate-composefs (Rust CLI)
├── main.rs          — CLI entrypoint, preflight, orchestration
├── preflight.rs     — System checks (OSTree? UEFI? ESP? Btrfs? reflink?)
├── reflink.rs       — FICLONE ioctl for zero-copy block cloning
├── ostree.rs        — Scan OSTree repo objects, compute SHA-512
├── composefs.rs     — pull_image, create_image, seal_image wrappers
└── migration.rs     — 5-phase migration pipeline + /var migration
```

## 5-Phase Migration Pipeline

| Phase | What | Status |
|-------|------|--------|
| **1** | Import OSTree file objects → ComposeFS object store (SHA-512 keyed) | ✅ Working |
| **2** | Pull target OCI image via `bootc internals cfs oci pull` | ✅ Working |
| **3** | Create EROFS image via `bootc internals cfs oci create-image` + seal | ✅ Working |
| **4** | Stage deployment: copy /etc, write .origin, .imginfo, var symlink | ✅ Working |
| **5** | Mount EROFS, extract kernel/initrd, write bootloader entries | ⚠️ Artifacts correct, GRUB doesn't pick them up |

## What Works

All migration phases complete without errors. The tool successfully:

1. **Detects** the system is OSTree-booted, UEFI-capable, Btrfs-backed
2. **Remounts** `/sysroot` and `/boot` read-write
3. **Imports** ~30,900 OSTree file objects into ComposeFS store
4. **Pulls** the target OCI image into the ComposeFS repository
5. **Creates** an EROFS filesystem image from the pulled layers
6. **Seals** the image (generates verity metadata)
7. **Mounts** the raw EROFS backing file directly (`mount -t erofs`) to extract kernel/initrd
8. **Copies** `/etc` configuration to the new deployment
9. **Writes** `.origin` and `.imginfo` metadata files
10. **Migrates** `/var` data from OSTree deployment to ComposeFS state directory

## Verified Artifacts (confirmed in CI)

After migration completes, the following exist on disk:

- **Deployment dir:** `/sysroot/state/deploy/<sha512_hash>/`
- **BLS entry:** `/boot/loader/entries/bootc_bluefin_dakota-<hash>.conf`
- **Kernel:** `/boot/bootc_composefs-<hash>/vmlinuz` (~18 MB)
- **Initrd:** `/boot/bootc_composefs-<hash>/initrd` (~120 MB)
- **EROFS image:** `/sysroot/composefs/images/<hash>` → objects symlink
- **.origin file:** Points to target OCI image with composefs digest
- **.imginfo file:** OCI config JSON for `bootc status`

## Current Blocker: GRUB Doesn't Boot the ComposeFS Entry

**Symptom:** After reboot, `bootc status --json` shows `store: ostreeContainer` and `composefs: null` — the system boots back to the old OSTree deployment.

**What we've tried:**
1. `grub2-mkconfig -o /boot/grub2/grub.cfg` — fails with `grub2-probe: error: failed to get canonical path of 'composefs'` (probe tries to resolve the `composefs=` kernel option as a device)
2. `grub2-set-default <entry-id>` — doesn't change boot behavior
3. Writing `saved_entry=<id>` directly to `/boot/grub2/grubenv` — no effect
4. `sort-key bootc-bluefin-dakota-0` in BLS entry — doesn't take priority

**Root cause hypothesis:** GRUB's `blscfg` module reads BLS entries from `/boot/loader/entries/` but may not be parsing our entry correctly, OR the existing OSTree entry has a higher priority sort-key.

**Next steps to investigate:**
- Dump the OSTree BLS entry (`ostree-1.conf`) to compare format/sort-key
- Check if GRUB is in `savedefault` mode (`GRUB_DEFAULT=saved` in `/etc/default/grub`)
- Try modifying the OSTree entry's sort-key to sort AFTER the composefs entry
- Consider using `grub2-reboot` with the entry title instead of `grub2-set-default`
- Verify the BLS entry content is valid by parsing with grubby or bootctl

## Previous Issues Resolved

| Issue | Fix | Commit |
|-------|-----|--------|
| `/sysroot` read-only | `mount -o remount,rw /sysroot` | `356ab23` |
| Missing `docker://` scheme | Auto-prepend transport prefix | `a7bfd38` |
| Multi-line pull output parsing | Parse manifest/config from labeled lines | `f981019`, `ce25a83` |
| Manifest vs config digest confusion | Separate manifest_digest from config_digest | `ff25839` |
| `/usr` read-only (`scp` destination) | Copy binary to `/var/tmp` | `4386461` |
| `/etc` copy fails on symlinks/sockets | Handle symlinks, skip special files | `53f2cd4` |
| `bootc internals cfs oci mount` needs sealed | Call `seal` after `create-image` | `6327e54` |
| Still "Can only mount sealed containers" | Mount raw EROFS file directly | `c0b8978` |
| `/boot` read-only (OSTree default) | `mount -o remount,rw /boot` | `18c7f85` |
| Colon in hash dir names confuses GRUB | Strip `sha512:` prefix from paths | `ab9ae0d` |
| `grub2-mkconfig` breaks on composefs= | Use grub2-set-default + grubenv | `bb55ed2`, `35899f9` |

## E2E Testing

### CI (GitHub Actions)
- **Workflow:** `.github/workflows/e2e-tests.yml`
- **Base:** `quay.io/fedora/fedora-bootc:44` (OSTree)
- **Target:** `quay.io/fedora/fedora-bootc:44` (ComposeFS, same image)
- **Runner:** `ubuntu-latest` with QEMU + KVM + OVMF
- **Status:** Migration completes, artifacts verified, but post-reboot composefs detection fails

### Local (Bluefin → Dakota)
- **Script:** `tests/run-e2e.sh`
- **Base:** `ghcr.io/projectbluefin/bluefin:stable` with derived image (sshd enabled)
- **Target:** `ghcr.io/projectbluefin/dakota:stable`
- **Blocker:** Bluefin's kernel doesn't output to serial console AND sshd is disabled by default
- **Fix attempted:** Build derived image FROM bluefin:stable with systemd preset for sshd + PermitRootLogin
- **Status:** Derived image builds, GRUB boots, but no SSH — likely sshd still not starting or kernel console issue

### Test Fixtures (injected before migration, verified after)
The test script creates these in /var and verifies post-migration:
- `/var/lib/migration-test/data` — basic persistence
- `/var/home/testuser/` — user home with dotfiles
- `/var/home/devuser/` — nested project structure + SSH keys
- `/var/lib/systemd/timers/` — system state
- `/var/lib/alternatives/` — symlinks
- `/var/cache/.hidden-dir/` — hidden directories

## Key Design Decisions

1. **EROFS direct mount** instead of `bootc internals cfs oci mount` — avoids the "sealed container" requirement and works more reliably
2. **Clean hex hashes** for directory names — stripped `sha512:` prefix to avoid GRUB parsing issues
3. **Manual `/boot` management** — writing kernel/initrd and BLS entries directly rather than delegating to `prepare-boot` (which has caveats about bootc compatibility)
4. **Remounting `/sysroot` and `/boot` read-write** — required on OSTree systems where both are mounted read-only by default

## Files

```
.
├── SPECIFICATION.md              # Detailed technical specification
├── HANDOFF.md                    # This file
├── Cargo.toml
├── src/
│   ├── main.rs                   # CLI entrypoint
│   ├── migration.rs              # 5-phase pipeline orchestrator
│   ├── composefs.rs              # bootc internals cfs wrappers
│   ├── ostree.rs                 # OSTree repo scanner
│   ├── preflight.rs              # System preflight checks
│   └── reflink.rs                # FICLONE ioctl
├── tests/
│   └── run-e2e.sh                # QEMU-based E2E test script
└── .github/workflows/
    └── e2e-tests.yml             # CI workflow
```
