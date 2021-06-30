use isolated::{Command, WaitStatus};

#[test]
fn smoke_test() -> nix::Result<()> {
    let status = Command::new("rootfs", "/bin/pwd").spawn()?.wait()?;
    assert!(matches!(status, WaitStatus::Exited(_, 0)));
    Ok(())
}
