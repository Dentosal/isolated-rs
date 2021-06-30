# `isolated` - a child-process container for Rust on Linux

[![crates.io badge](https://img.shields.io/crates/v/isolated.svg)](https://crates.io/crates/isolated)
[![docs.rs badge](https://docs.rs/isolated/badge.svg)](https://docs.rs/isolated/)

Sets up following limits:
* Limits filesystem access with `pivot_root` and `overlayfs`, making it possible to only read a fabricated read-only root filesystem (usually from Alpine minirootfs) and a single directory (`writedir`) that is shared between the host and the container.
* Limits network access using a network namespace. Currently access to other networks is simply disabled. In the future it should be interesting to implement a proper access control using VETH interfaces.
* Disables access to host pids and mounts using namespaces.

## API stability

Not yet, although I will not be making major breaking changes without incrementing `0.x` version.

## Running an example

Note that running this requires root privileges, as setting up namespaces cannot be done otherwise. This repository contains a `.cargo/config` that uses `sudo -E` with all cargo runners.

Firstly, download alpine minirootfs and extract that (using [`./download-rootfs.sh`](download-rootfs.sh) works).

Then `cargo run --example shell` gives you an isolated interactive shell. See [the source code for the example](examples/shell.rs).

## License

MIT
