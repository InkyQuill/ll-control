# Group-independent HID access on Arch, 2026-11-14

A clean KVM guest booted the official 2026-11-01 Arch x86_64 cloud image
(SHA-256 `794eb4889b39e1f0ed7d5ae998b2a1ed2137db8cf8c3476cf2962c7437ecb003`).
The kernel was 7.2.11-arch1-1. A configfs vendor-specific USB HID gadget on
dummy_hcd provided the LCDV2 vendor/product IDs 0416:7395 and a kernel-created
hidraw node. The host also passed the same probe on the physical LCDV2.

Before installation, the guest node was root-only, mode 0600, and lacked the
uaccess tag. We installed the tracked rule under /usr/lib/udev/rules.d and
rebooted. The guest enumerated the gadget again. The tester account had a local
seat0 login on tty1, and the plugdev group did not exist. The node was owned by
root:root with an explicit rw ACL for tester. Opening the node O_RDWR from
tester succeeded and the nobody account received EACCES. No HID reports were sent.

The guest exercises USB enumeration, udev, logind, ACLs and access after reboot.
Package installation is tracked under #5.
