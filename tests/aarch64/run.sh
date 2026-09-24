#!/bin/sh
# daimon's aarch64 VM test (2.4.2): boots a real aarch64 Linux kernel under
# qemu-system-aarch64 and runs daimon's suites, built for aarch64, in it as an
# ordinary user (nobody). It exists for what the user-mode run cannot check:
# qemu-aarch64, which CI's "Syscall portability under qemu-aarch64" step uses,
# passes prlimit64's RLIMIT_AS through as a no-op and runs a thread of its own,
# so six of the agent suite's checks fail there whatever daimon does. Under a
# real kernel every one must pass.
#
#   sh tests/aarch64/run.sh      # exit 0 = every suite passed in the VM
#
# The kernel and the userland are Alpine's 3.24.2 aarch64 release files: the
# netboot kernel (vmlinuz-virt) and the minirootfs (busybox, musl). They are
# downloaded into build/aarch64-release/ and refused unless their SHA-256
# matches the pins below. Alpine publishes one for the minirootfs; for the
# netboot kernel it publishes none, so its pin is the hash recorded at 2.4.2
# from Alpine's CDN over HTTPS.
# Needs qemu-system-aarch64, cpio, gzip, curl and sha256sum. VM_SUITES picks the
# suites (default: agent syscall_portability), VM_TIMEOUT the seconds allowed.
set -u
cd "$(dirname "$0")/../.."
ROOT=$(pwd)
ALPINE=3.24.2
ALPINE_BRANCH=v3.24
KERNEL_SHA256=e45e1f6083d1ed45db6647b422e32b6ae6dc54de7b8190b7b97744fb293412e3
ROOTFS_SHA256=9bf70a7f18ea44094cbb5f70c58f9af129c8214745743db0e68e5502cc2ce773
REL=$ROOT/build/aarch64-release
WORK=$ROOT/build/aarch64-vm
SUITES=${VM_SUITES:-"agent syscall_portability"}
TIMEOUT=${VM_TIMEOUT:-900}
URL=https://dl-cdn.alpinelinux.org/alpine/$ALPINE_BRANCH/releases/aarch64

# fetch URL FILE SHA256: download FILE unless it is already there with that hash.
fetch() {
    if [ -f "$2" ] && echo "$3  $2" | sha256sum -c --status 2>/dev/null; then return 0; fi
    curl -fsSL --retry 5 --retry-all-errors -o "$2.part" "$1" \
        || { echo "aarch64-vm: download of $1 failed"; rm -f "$2.part"; return 1; }
    if ! echo "$3  $2.part" | sha256sum -c --status; then
        echo "aarch64-vm: $1 does not match its pinned SHA-256"; rm -f "$2.part"; return 1
    fi
    mv "$2.part" "$2"
}
for tool in qemu-system-aarch64 cpio gzip curl sha256sum; do
    command -v "$tool" >/dev/null 2>&1 || { echo "aarch64-vm: missing tool $tool"; exit 2; }
done
mkdir -p "$REL"
KERNEL=$REL/vmlinuz-virt-$ALPINE
ROOTFS=$REL/alpine-minirootfs-$ALPINE-aarch64.tar.gz
fetch "$URL/netboot-$ALPINE/vmlinuz-virt" "$KERNEL" "$KERNEL_SHA256" || exit 2
fetch "$URL/alpine-minirootfs-$ALPINE-aarch64.tar.gz" "$ROOTFS" "$ROOTFS_SHA256" || exit 2

rm -rf "$WORK"; mkdir -p "$WORK/root/daimon"
tar -xzf "$ROOTFS" -C "$WORK/root" 2>/dev/null || { echo "aarch64-vm: cannot unpack the minirootfs"; exit 2; }
for s in $SUITES; do
    cyrius build --aarch64 "tests/$s.tcyr" "$WORK/root/daimon/$s" > "$WORK/$s.build" 2>&1 \
        || { echo "aarch64-vm: build of tests/$s.tcyr failed:"; tail -5 "$WORK/$s.build"; exit 2; }
done
# PID 1. The initramfs has no /dev/console for the kernel to open, so init's
# output goes to the console only once devtmpfs is mounted. The suites run in a
# cgroup handed to nobody, as CI's smoke step hands one to its user, so the agent
# suite's containment checks take the contained path: the root cgroup is root's.
cat > "$WORK/root/init" <<'INIT'
#!/bin/sh
mount -t proc proc /proc
mount -t sysfs sys /sys
mount -t devtmpfs dev /dev
exec > /dev/console 2>&1 < /dev/console
mount -t tmpfs tmp /tmp
mount -t cgroup2 cgroup2 /sys/fs/cgroup
mkdir /sys/fs/cgroup/suites
chown -R nobody /sys/fs/cgroup/suites
echo $$ > /sys/fs/cgroup/suites/cgroup.procs
echo "VM START $(uname -m) $(uname -r)"
echo "VM cgroup $(cat /proc/self/cgroup)"
rc=0
for s in /daimon/*; do
    n=${s##*/}
    su -s /bin/sh nobody -c "export PATH=/usr/bin:/bin; cd /tmp && exec $s" > "/tmp/$n.out" 2>&1
    r=$?
    sed "s/^/VM $n | /" "/tmp/$n.out"
    echo "VM SUITE $n exit $r"
    [ $r -eq 0 ] || rc=1
done
echo "VM DONE $rc"
poweroff -f
INIT
chmod 755 "$WORK/root/init" "$WORK/root/daimon" "$WORK/root/daimon"/*
(cd "$WORK/root" && find . | cpio -o -H newc -R 0:0 --quiet | gzip -1 > "$WORK/initramfs.gz") \
    || { echo "aarch64-vm: cannot pack the initramfs"; exit 2; }

LOG=$WORK/serial.log
timeout "$TIMEOUT" qemu-system-aarch64 -M virt -cpu cortex-a57 -smp 2 -m 1024M -nographic -no-reboot -nic none \
    -kernel "$KERNEL" -initrd "$WORK/initramfs.gz" \
    -append "console=ttyAMA0 rdinit=/init quiet" > "$LOG" 2>&1
qrc=$?

grep -E "^VM (START|cgroup|SUITE|DONE)|not contained|FAIL|[0-9]+ passed, [0-9]+ failed" "$LOG" | sed 's/^/  /'
if [ $qrc -eq 124 ]; then echo "aarch64-vm: FAIL: the VM did not finish within ${TIMEOUT}s (log: $LOG)"; exit 1; fi
if ! grep -q "^VM START aarch64" "$LOG"; then echo "aarch64-vm: VOID: the kernel never ran init (log: $LOG)"; exit 2; fi
if grep -q "^VM DONE 0" "$LOG"; then echo "aarch64-vm: PASS"; exit 0; fi
echo "aarch64-vm: FAIL (log: $LOG)"
exit 1
