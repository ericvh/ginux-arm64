#!/usr/bin/env python3
"""What depends on the host kernel's page size (aarch64, Linux, root).

Runs tests/fixtures/page-size-fixture.c as an instance's init and checks
what it reports. arm64 kernels have 4K, 16K, or 64K pages; on a 4K-page
kernel every check passes whatever the supervisor assumes, so this only
means something run on one of the others.
"""
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile


def main():
    if len(sys.argv) != 3 or os.geteuid() != 0:
        sys.exit("usage (as root): test-native-page-size.py <ginux> <page-size fixture>")
    binary, fixture = (Path(arg).resolve() for arg in sys.argv[1:])
    template = (Path(__file__).resolve().parent.parent / "config/instance-gate5.toml").read_text()
    page = os.sysconf("SC_PAGE_SIZE")

    with tempfile.TemporaryDirectory(prefix="ginux-page-size-") as directory:
        work = Path(directory)
        # The tracee drops to UID 65534 and must be able to execute the copies.
        work.chmod(0o755)
        shutil.copyfile(binary, work / "ginux")
        (work / "ginux").chmod(0o755)
        root = work / "root"
        (root / "usr/bin").mkdir(parents=True)
        (root / "tmp").mkdir()
        (root / "tmp").chmod(0o1777)
        shutil.copyfile(fixture, root / "usr/bin/bash")
        (root / "usr/bin/bash").chmod(0o755)
        (root / "scenario").write_text("page-size")
        config = work / "instance.toml"
        config.write_text(template.replace('init = "/bin/bash"', 'init = "/usr/bin/bash"')
                          .replace('[network]\nbackend = "none"', '[network]\nbackend = "shared"') + """
[contract.filesystem]
read = ["/"]
write = ["/"]
create = ["/"]
[contract.kernel]
proc_read = ["/"]
sys_read = ["/"]
""")
        path = root / "tmp/page-size.sock"
        with socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM) as receiver:
            receiver.bind(str(path))
            path.chmod(0o777)
            receiver.settimeout(2)
            proc = subprocess.run([str(work / "ginux"), "native", str(config), str(root), "configured"],
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
            output = (proc.stdout + proc.stderr).decode(errors="replace")
            try:
                received = receiver.recv(16)
            except socket.timeout:
                received = b""

    reported = dict(line[len("GINUX_FIXTURE: "):].split("=", 1)
                    for line in output.splitlines() if line.startswith("GINUX_FIXTURE: "))

    def check(name, ok):
        if not ok:
            raise RuntimeError(f"{name}: exit={proc.returncode} page={page}\n{output}")
        print(f"PASS native {name}")

    check("the instance sees the host's page size",
          proc.returncode == 0 and reported.get("page-size") == str(page))
    check("getcwd takes a buffer of one page", reported.get("getcwd-page") == "1")
    check("statm counts pages of the host's size", reported.get("statm-matches-status") == "1")
    check("a second task's clone3 snapshot is mapped", reported.get("second-task-clone3") == "1")
    check("a second task's send snapshot is mapped",
          reported.get("second-task-send") == "1" and received == b"p")
    print(f"test-native-page-size: PASS ({page}-byte pages)")


if __name__ == "__main__":
    main()
