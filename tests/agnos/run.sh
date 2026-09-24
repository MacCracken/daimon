#!/bin/bash
# daimon's AGNOS guest test (2.4.0): boots the agnos kernel under QEMU and reads
# the verdict from the serial console. kybernet (PID 1) execs /bin/agnsh, the
# launcher (tests/agnos/launcher.cyr), which runs
#   /bin/guest   tests/agnos/guest.cyr: src/agent.cyr against the real kernel,
#                with the fixture agents under /agents;
#   /bin/daimon  the real daimon, `serve 8090 --agents-dir /agents`;
#   /bin/client  tests/agnos/http_client.cyr: daimon's API over TCP to the
#                guest's own address (the kernel's loopback path).
#
#   sh tests/agnos/run.sh            # exit 0 = every assertion passed
#   sh tests/agnos/run.sh --release  # on the released binaries pinned below (CI)
#
# Needs an agnos kernel and gnoboot (they are read, never written):
#   AGNOS_KERNEL  default ../agnos/build/agnos
#   GNOBOOT_EFI   default ../gnoboot/build/BOOTX64.EFI
# or, with --release, the published releases below, downloaded into
# build/agnos-release/ and refused unless they match the SHA-256 recorded here.
# Also qemu-system-x86_64, OVMF, parted, sgdisk, mtools and mkfs.ext2 (the same
# tools as agnos's scripts/smoke/agnsh-smoke.sh, whose image layout this
# follows). CI runs it with --release on every push (2.4.1).
set -u
cd "$(dirname "$0")/../.."
ROOT=$(pwd)
# The releases --release boots (2.4.1). Each is byte-identical to the build 2.4.0
# was first measured on. Move a pin by changing the version and its SHA-256 together,
# from the release's own SHA256SUMS.
AGNOS_RELEASE=1.57.5
AGNOS_RELEASE_SHA256=5b73d81c2e872f72396840f9d25036e7245f50f231b1cf4873fd27432ad079be
GNOBOOT_RELEASE=0.7.2
GNOBOOT_RELEASE_SHA256=7420e59dcd26da498babc8ad0ff24d37b3bea8d832f670be3225ea5162c6d7e3
KERNEL=${AGNOS_KERNEL:-$ROOT/../agnos/build/agnos}
GNOBOOT=${GNOBOOT_EFI:-$ROOT/../gnoboot/build/BOOTX64.EFI}
WORK=$ROOT/build/agnos-guest
TIMEOUT=${GUEST_TIMEOUT:-240}

# fetch URL FILE SHA256: download FILE unless it is already there with that hash.
fetch() {
    if [ -f "$2" ] && echo "$3  $2" | sha256sum -c --status 2>/dev/null; then return 0; fi
    curl -fsSL --retry 5 --retry-all-errors -o "$2.part" "$1" \
        || { echo "agnos-guest: download of $1 failed"; rm -f "$2.part"; return 1; }
    if ! echo "$3  $2.part" | sha256sum -c --status; then
        echo "agnos-guest: $1 does not match its pinned SHA-256"; rm -f "$2.part"; return 1
    fi
    mv "$2.part" "$2"
}
if [ "${1:-}" = "--release" ]; then
    REL=$ROOT/build/agnos-release
    mkdir -p "$REL"
    KERNEL=$REL/agnos-$AGNOS_RELEASE
    GNOBOOT=$REL/gnoboot-$GNOBOOT_RELEASE.efi
    fetch "https://github.com/MacCracken/agnos/releases/download/$AGNOS_RELEASE/agnos-x86_64" \
        "$KERNEL" "$AGNOS_RELEASE_SHA256" || exit 2
    fetch "https://github.com/MacCracken/gnoboot/releases/download/$GNOBOOT_RELEASE/BOOTX64.EFI" \
        "$GNOBOOT" "$GNOBOOT_RELEASE_SHA256" || exit 2
    echo "agnos-guest: agnos $AGNOS_RELEASE and gnoboot $GNOBOOT_RELEASE (released, SHA-256 checked)"
fi
[ -f "$KERNEL" ] || { echo "agnos-guest: no agnos kernel at $KERNEL (set AGNOS_KERNEL, or use --release)"; exit 2; }
[ -f "$GNOBOOT" ] || { echo "agnos-guest: no gnoboot at $GNOBOOT (set GNOBOOT_EFI, or use --release)"; exit 2; }
for tool in qemu-system-x86_64 parted sgdisk mformat mmd mcopy mkfs.ext2 strings; do
    command -v "$tool" >/dev/null 2>&1 || { echo "agnos-guest: missing tool $tool"; exit 2; }
done
OVMF_CODE=""; for c in /usr/share/edk2/x64/OVMF_CODE.4m.fd /usr/share/edk2/x64/OVMF_CODE.fd /usr/share/OVMF/OVMF_CODE.fd /usr/share/OVMF/OVMF_CODE_4M.fd; do [ -f "$c" ] && { OVMF_CODE=$c; break; }; done
OVMF_VARS=""; for c in /usr/share/edk2/x64/OVMF_VARS.4m.fd /usr/share/edk2/x64/OVMF_VARS.fd /usr/share/OVMF/OVMF_VARS.fd /usr/share/OVMF/OVMF_VARS_4M.fd; do [ -f "$c" ] && { OVMF_VARS=$c; break; }; done
[ -n "$OVMF_CODE" ] && [ -n "$OVMF_VARS" ] || { echo "agnos-guest: OVMF not found"; exit 2; }

rm -rf "$WORK"; mkdir -p "$WORK/seed/bin" "$WORK/seed/agents"
for p in launcher guest http_client agent_user agent_system agent_service; do
    cyrius build --agnos "tests/agnos/$p.cyr" "$WORK/$p" > "$WORK/$p.build" 2>&1 \
        || { echo "agnos-guest: build of tests/agnos/$p.cyr failed:"; tail -5 "$WORK/$p.build"; exit 2; }
done
cyrius build --agnos src/main.cyr "$WORK/daimon" > "$WORK/daimon.build" 2>&1 \
    || { echo "agnos-guest: build of daimon failed:"; tail -5 "$WORK/daimon.build"; exit 2; }
cp "$WORK/launcher" "$WORK/seed/bin/agnsh"
cp "$WORK/guest" "$WORK/seed/bin/guest"
cp "$WORK/daimon" "$WORK/seed/bin/daimon"
cp "$WORK/http_client" "$WORK/seed/bin/client"
cp "$WORK/agent_user" "$WORK/seed/agents/agnos-agent-user-agent"
cp "$WORK/agent_system" "$WORK/seed/agents/agnos-agent-system-agent"
cp "$WORK/agent_service" "$WORK/seed/agents/agnos-agent-service-agent"

IMG="$WORK/agnos.img"
PART_OFFSET=$(( 33 * 1048576 )); PART_BYTES=$(( 67 * 1048576 )); PART_BLOCKS=$(( PART_BYTES / 4096 ))
dd if=/dev/zero of="$IMG" bs=1M count=128 status=none
parted -s "$IMG" mklabel gpt mkpart ESP fat32 1MiB 33MiB set 1 esp on mkpart agnos-fs ext2 33MiB 100MiB
sgdisk -t 2:8300 "$IMG" >/dev/null
mformat -i "$IMG"@@1048576 -F
mmd -i "$IMG"@@1048576 ::EFI ::EFI/BOOT ::boot
mcopy -i "$IMG"@@1048576 "$GNOBOOT" ::EFI/BOOT/BOOTX64.EFI
mcopy -i "$IMG"@@1048576 "$KERNEL" ::boot/agnos
mkfs.ext2 -F -q -L DAIMON-GUEST -b 4096 -m 0 -O "^resize_inode,^dir_index,^metadata_csum,^64bit,^uninit_bg" \
    -d "$WORK/seed" -E offset=$PART_OFFSET "$IMG" $PART_BLOCKS

LOG="$WORK/serial.log"
QP=0
try=0
while [ $try -lt 3 ]; do
    try=$((try + 1))
    cp "$OVMF_VARS" "$WORK/vars.fd"; chmod +w "$WORK/vars.fd"
    : > "$LOG"
    qemu-system-x86_64 -machine q35 -m 512M -cpu max \
        -drive "if=pflash,format=raw,readonly=on,file=$OVMF_CODE" \
        -drive "if=pflash,format=raw,file=$WORK/vars.fd" \
        -drive "file=$IMG,format=raw,if=none,id=disk0" -device "nvme,drive=disk0,serial=DAIMON-GUEST" \
        -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
        -serial "file:$LOG" -display none -no-reboot \
        -monitor "unix:build/agnos-guest/mon.sock,server,nowait" > "$WORK/qemu.out" 2>&1 &
    QP=$!
    # The firmware sometimes never hands off to the kernel (agnsh-smoke's measured
    # flake): retry only then, never after the kernel has run.
    i=0; while [ $i -lt 40 ]; do strings "$LOG" | grep -q "AGNOS kernel v" && break; i=$((i + 1)); sleep 0.5; done
    if strings "$LOG" | grep -q "AGNOS kernel v"; then break; fi
    kill $QP 2>/dev/null; wait $QP 2>/dev/null; QP=0
    echo "agnos-guest: firmware did not hand off (try $try)"
done
[ $QP -ne 0 ] || { echo "agnos-guest: VOID: the kernel never started"; exit 2; }
i=0; while [ $i -lt "$TIMEOUT" ]; do strings "$LOG" | grep -q "LAUNCHER DONE" && break; i=$((i + 1)); sleep 1; done
# A guest that did not finish: where it is, from QEMU's monitor, before QEMU goes
# (tests/agnos/monitor.py says how to read it). The socket's path is relative: a
# Unix socket's must be under 108 bytes.
if ! strings "$LOG" | grep -q "LAUNCHER DONE" && command -v python3 >/dev/null 2>&1; then
    python3 tests/agnos/monitor.py build/agnos-guest/mon.sock > "$WORK/monitor.txt" 2>&1
fi
kill $QP 2>/dev/null; wait $QP 2>/dev/null

strings "$LOG" | grep -E "AGNOS kernel v|tsc: |^GUEST|^AGENT|^CLIENT|^LAUNCHER|daimon v|FAIL|passed, |response:" | sed 's/^/  /'
rc=0
if ! strings "$LOG" | grep -q "LAUNCHER DONE"; then
    echo "agnos-guest: FAIL: the run did not finish within ${TIMEOUT}s"; rc=1
    [ -f "$WORK/monitor.txt" ] && sed 's/^/  /' "$WORK/monitor.txt"
fi
if ! strings "$LOG" | grep -q "GUEST DONE 0"; then echo "agnos-guest: FAIL: the agent test"; rc=1; fi
if ! strings "$LOG" | grep -q "CLIENT DONE 0"; then echo "agnos-guest: FAIL: the HTTP test"; rc=1; fi
# The fixture's own report: the argv daimon built, and the environment it passed.
if ! strings "$LOG" | grep -q "AGENT user argv: /agents/agnos-agent-user-agent --agent-id 1 --agent-name worker"; then
    echo "agnos-guest: FAIL: the user agent did not report the argv daimon builds"; rc=1
fi
if ! strings "$LOG" | grep -q "AGENT user got SIGTERM"; then
    echo "agnos-guest: FAIL: the user agent never saw SIGTERM"; rc=1
fi
if [ $rc -eq 0 ]; then echo "agnos-guest: PASS"; else echo "agnos-guest: FAIL (serial log: $LOG)"; fi
exit $rc
