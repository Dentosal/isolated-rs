use std::{
    ffi::CString,
    path::{Path, PathBuf},
};

use crate::Process;

#[derive(Debug, Clone)]
pub(crate) enum DiskWritePolicy {
    /// No write access to any filesystem parts
    ReadOnly,
    /// Write to temporary directory, automatically deleted when dropping child
    TempDir,
    /// Write modifications to the file system done by the application to this directory
    WriteDir(PathBuf),
}

type Hook = dyn FnOnce() -> nix::Result<()>;

/// Offers an API similar to `std::process::Command`.
#[must_use]
pub struct Command {
    /// Command path inside the isolated filesystem
    pub(crate) path: CString,
    /// Command arguments
    pub(crate) args: Vec<CString>,
    /// OverlayFS layers from outermost to innermost, usually `[rootfs, appdir]`
    /// where rootfs contains a linux root file system like Alpine minirootfs,
    /// and `appdir` is the directory where the application binary is located.
    /// All of the layers are overlayed on the root of the container file system.
    pub(crate) layers: Vec<PathBuf>,
    /// Disk write access
    pub(crate) disk_write: DiskWritePolicy,
    /// Called just before pivot_root, after fork
    pub(crate) pre_pivot: Vec<Box<Hook>>,
    /// Called just before exec'ing new process, after fork and pivot_root
    pub(crate) pre_exec: Vec<Box<Hook>>,
}
impl Command {
    /// Command path inside the isolated filesystem.
    /// Panics if path contains null bytes.
    pub fn new<P: AsRef<Path>>(root_fs: P, path: &str) -> Self {
        let path = CString::new(path.as_bytes().to_vec()).expect("Nul byte in target path");
        Self {
            path: path.clone(),
            args: vec![path],
            layers: vec![root_fs.as_ref().to_owned()],
            disk_write: DiskWritePolicy::ReadOnly,
            pre_pivot: Vec::new(),
            pre_exec: Vec::new(),
        }
    }

    /// Panics if any argument contains null bytes.
    pub fn args(mut self, args: &[&str]) -> Self {
        self.args =
            std::iter::once(self.path.clone())
                .chain(args.iter().map(|arg| {
                    CString::new(arg.as_bytes().to_vec()).expect("Nul byte in an argument")
                }))
                .collect();
        self
    }

    /// Adds new read-only OverlayFS layer
    pub fn layer<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.layers.push(path.as_ref().to_owned());
        self
    }

    /// Allows disk writes to a temporary directory
    pub fn disk_write_tempdir(mut self) -> Self {
        self.disk_write = DiskWritePolicy::TempDir;
        self
    }

    /// Allows disk writes to a temporary directory
    pub fn disk_write_to<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.disk_write = DiskWritePolicy::WriteDir(path.as_ref().to_owned());
        self
    }

    /// Hook is called just before pivot_root, after fork.
    /// If multiple hooks are registered, they will be called in order.
    /// If any hook returns an error, no more hooks will be called, and
    /// the process will not be started.
    pub fn hook_pre_pivot(mut self, hook: Box<Hook>) -> Self {
        self.pre_pivot.push(hook);
        self
    }

    /// Hook is called just before exec, after fork and pivot_root.
    /// If multiple hooks are registered, they will be called in order.
    /// If any hook returns an error, no more hooks will be called, and
    /// the process will not be started.
    pub fn hook_pre_exec(mut self, hook: Box<Hook>) -> Self {
        self.pre_exec.push(hook);
        self
    }

    pub fn spawn(self) -> nix::Result<Process> {
        Process::spawn(self)
    }
}
