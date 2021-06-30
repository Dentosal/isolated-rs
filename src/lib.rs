use std::ffi::CString;
use std::path::{Path, PathBuf};

use backtrace::Backtrace;

use nix::fcntl::OFlag;
use nix::sched::{clone, CloneFlags};
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{execv, mkdir, Pid};

use tempfile::{tempdir, TempDir};

/// Wrapper for automatically closing a raw file
/// when it goes out of scope
struct AutoCloseFd {
    /// Raw file descriptor
    fd: i32,
}
impl Drop for AutoCloseFd {
    fn drop(&mut self) {
        use nix::unistd::close;
        // Ignore errors
        let _ = close(self.fd);
    }
}

fn setup_rootfs(path: &Path) {
    use nix::fcntl::open;
    use nix::mount::{mount, umount2, MntFlags, MsFlags};
    use nix::sys::stat::Mode;
    use nix::unistd::{fchdir, pivot_root};

    let none: Option<&str> = None;
    let oflag = OFlag::O_DIRECTORY | OFlag::O_RDONLY;
    let mode = Mode::empty();

    // Hold both old and new root file descriptors
    AutoCloseFd {
        fd: open("/", oflag, mode).expect("Could not open old root directory"),
    };
    let newroot = AutoCloseFd {
        fd: open(path, oflag, mode).expect("Could not open new root directory"),
    };

    // Mark old and new roots as private
    mount(none, "/", none, MsFlags::MS_PRIVATE, none)
        .expect("Could not remount old root directory as private");
    mount(none, path, none, MsFlags::MS_PRIVATE, none)
        .expect("Could not remount new root directory as private");

    // Change root to point to the new root directory
    fchdir(newroot.fd).expect("Chould not change to new root directory");
    pivot_root(".", ".").expect("pivot_root failed");

    // Mount useful pseudo-filesystems
    let _ = mkdir("/proc", Mode::from_bits(0o700).unwrap());
    mount(none, "/proc", Some("proc"), MsFlags::empty(), none).expect("Could not mount proc");

    let _ = mkdir("/sys", Mode::from_bits(0o700).unwrap());
    mount(none, "/sys", Some("sysfs"), MsFlags::empty(), none).expect("Could not mount sysfs");

    // Detach from the old root so that it can not be used anymore
    umount2("/", MntFlags::MNT_DETACH).expect("Could not detach from old root directory");
}

fn overlayfs_escape_path<P: Into<String>>(path: P) -> String {
    path.into()
        .replace("\\", "\\\\")
        .replace(":", "\\:")
        .replace(",", "\\,")
}

fn create_overlayfs<L: AsRef<Path>>(
    mountpoint: &Path,
    workdir: &Path,
    layers: &[L],
    writedir: &Path,
) {
    use nix::mount::{mount, MsFlags};

    let options = format!(
        "workdir={},lowerdir={},upperdir={}",
        overlayfs_escape_path(workdir.to_str().expect("TODO: utf8 error")),
        layers
            .iter()
            .map(|p| overlayfs_escape_path(p.as_ref().to_str().expect("TODO: utf8 error")))
            .collect::<Vec<_>>()
            .join(":"),
        overlayfs_escape_path(writedir.to_str().expect("TODO: utf8 error"))
    );

    mount(
        Some("overlay"),
        mountpoint,
        Some("overlay"),
        MsFlags::empty(),
        Some(options.as_str()),
    )
    .expect("overlayfs mount");
}

/// Resources held by a process.
/// These require cleanup when the process has completed.
#[allow(dead_code)] // Fields are used for Drop, rustc isn't smart enough
struct HeldResources {
    /// Deleted on drop
    tmp: TempDir,
}

impl Drop for HeldResources {
    fn drop(&mut self) {
        let mountpoint = self.tmp.path().join("mount");
        nix::mount::umount(&mountpoint).expect("Failed to umount mountpoint");
    }
}

/// Offers an API similar to `std::process::Child`.
/// When dropping, attempts termination and cleanup.
pub struct Process {
    /// A Linux process id.
    /// Only guarantedd to point to the correct existing process
    /// before it has been waited for, so in case `self.status.is_some()`,
    /// this must not be used anymore.
    id: Pid,
    /// Stored after the first successful `wait` call
    status: Option<WaitStatus>,
    /// Resources, mostly stored for cleanup
    #[allow(dead_code)] // Fields is used for Drop, rustc isn't smart enough
    resources: HeldResources,
}

impl Process {
    /// Spawns a new process from `path` with `args`.
    /// `layers` specify overlayfs layers from outermost to innermost,
    /// usually `[rootfs, appdir]` where rootfs contains a linux root
    /// file system like Alpine minirootfs, and `appdir` is the directory
    /// where the application binary is located. All of the layers are
    /// overlayed on the root of the container file system.
    /// `writedir` is a directory containing modifications to the file system
    /// done by the application. If it is `None`, then a temporary directory
    /// is used instead.
    ///
    /// `pre_exec`, if given, is a closure to be execute after for
    ///
    /// TODO: Document restrictions
    pub fn spawn<L: AsRef<Path>, W: AsRef<Path>>(
        path: &str,
        args: &[&str],
        layers: &[L],
        writedir: Option<W>,
        pre_pivot: Option<fn() -> nix::Result<()>>,
        pre_exec: Option<fn() -> nix::Result<()>>,
    ) -> nix::Result<Process> {
        let tmp = tempdir().expect("tempdir creation failed");
        let mountpoint = tmp.path().join("mount");
        let workdir = tmp.path().join("work");

        let writedir: PathBuf = writedir.map(|d| d.as_ref().to_owned()).unwrap_or_else(|| {
            let d = tmp.path().join("write");
            std::fs::create_dir(&d).expect("Creating temp writedir failed");
            d
        });

        std::fs::create_dir(&mountpoint).expect("Creating temp mountpoint failed");
        std::fs::create_dir(&workdir).expect("Creating temp workdir failed");

        create_overlayfs(&mountpoint, &workdir, &layers, &writedir);

        let path = CString::new(path.as_bytes().to_vec()).expect("Nul byte in target");
        let args: Vec<CString> =
            std::iter::once(path.clone())
                .chain(args.iter().map(|arg| {
                    CString::new(arg.as_bytes().to_vec()).expect("Nul byte in an argument")
                }))
                .collect();

        // A more full-featured implementation might end up setting an anonymous pipe
        // between the parent and this child; however, we simply print the error and
        // return with an error code if anything nasty happens.
        let old_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|panic_info| {
            let bt = Backtrace::new();
            println!("BUG: panic in pre-exec environment!");
            println!("{}", panic_info);
            println!("\nBacktrace:\n{:?}", bt);
            std::process::exit(1);
        }));

        let mut stack = [0; 4096];
        let id = clone(
            Box::new(move || {
                // In post-clone, pre-exec environment.
                // Many rust features do not work properly here, for instance:
                // * If the code panics, it causes a segfault after printing the panic message

                // Argument callback
                if let Some(f) = &pre_pivot {
                    f().expect("pre_pivot failed");
                }

                // Do process setup before exec
                setup_rootfs(&mountpoint);

                // Argument callback
                if let Some(f) = &pre_exec {
                    f().expect("pre_exec failed");
                }

                // Change into the next process
                execv(path.as_c_str(), &args).expect("execv failed");
                unreachable!();
            }),
            &mut stack,
            CloneFlags::CLONE_VFORK
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWNET,
            Some(Signal::SIGCHLD as i32),
        )
        .expect("Clone failed");

        // Restore old panic hook
        std::panic::set_hook(old_hook);

        Ok(Process {
            id,
            status: None,
            resources: HeldResources { tmp },
        })
    }

    /// Wait until the process completes, and return it's status.
    pub fn wait(&mut self) -> nix::Result<WaitStatus> {
        if let Some(old_status) = self.status {
            Ok(old_status)
        } else {
            let status = waitpid(self.id, None)?;
            self.status = Some(status);
            Ok(status)
        }
    }

    /// Send a signal to the process.
    /// Panics if `wait` has returned succesfully before.
    pub fn signal(&mut self, signal: Signal) -> nix::Result<()> {
        use nix::sys::signal::kill;

        if self.status.is_some() {
            panic!("Attempting to send a signal to a known-dead process");
        }

        kill(self.id, signal)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if self.status.is_none() {
            panic!("Dropping a running process");
            // self.inner.cleanup();
        }
    }
}
