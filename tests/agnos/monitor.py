# daimon's AGNOS guest test: where a guest that stopped making progress is, read
# from QEMU's monitor. tests/agnos/run.sh runs this when the guest does not finish
# in time, before it stops QEMU, and prints the lines it writes.
#
#   python3 tests/agnos/monitor.py build/agnos-guest/mon.sock
#
# It takes three samples of the vCPU, two seconds apart, and then follows the
# kernel stack's saved-RBP chain for return addresses. HLT=1 and CPL=0 in every
# sample, with CR3 unchanged, means one process holds the CPU in a kernel wait
# and nothing else is scheduled. On agnos 1.57.5 the frames 0x1bd797, 0x1bf6b8
# and 0x21a616 are net_wait_backoff, tcp_send's ACK wait and the sock_send#48
# arm: the hold filed with agnos (2026-09-23-sock-send-and-connect-hold-the-cpu).
import os
import re
import socket
import sys
import time

path = sys.argv[1]
# A socket path must be under 108 bytes, so connect by name from its directory.
os.chdir(os.path.dirname(os.path.abspath(path)))
mon = socket.socket(socket.AF_UNIX)
mon.settimeout(20)
try:
    mon.connect(os.path.basename(path))
except OSError as e:
    print(f"MONITOR unreachable: {e}")
    sys.exit(0)


def until_prompt():
    b = b""
    while not b.endswith(b"(qemu) "):
        d = mon.recv(1 << 20)
        if not d:
            break
        b += d
    return re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", b.decode(errors="replace"))


def cmd(c):
    mon.sendall((c + "\n").encode())
    out = until_prompt()
    return out.split("\n", 1)[1] if "\n" in out else out


def word(addr):
    m = re.search(r":\s+0x([0-9a-f]+)", cmd(f"x/1gx {addr:#x}"))
    return int(m.group(1), 16) if m else None


until_prompt()
rbp = None
for i in range(3):
    regs = cmd("info registers")
    f = dict(re.findall(r"\b(RIP|RBP|CR3|HLT|CPL)=([0-9a-f]+)", regs))
    print(f"MONITOR sample {i + 1}: RIP={f.get('RIP')} HLT={f.get('HLT')} CPL={f.get('CPL')} CR3={f.get('CR3')}")
    rbp = int(f["RBP"], 16) if "RBP" in f else None
    if i < 2:
        time.sleep(2)
frame = rbp
for _ in range(6):
    if frame is None:
        break
    ret, up = word(frame + 8), word(frame)
    if ret is None or up is None:
        break
    print(f"MONITOR frame {frame:#x} returns to {ret:#x}")
    if up <= frame:
        break
    frame = up
