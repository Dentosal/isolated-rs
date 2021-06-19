use std::env::current_dir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let d = current_dir().unwrap();

    let rootfs = d.join("rootfs/");
    let writedir = d.join("write/");

    std::fs::create_dir_all(&writedir)?;

    let mut child =
        isolated::Process::spawn("/bin/sh", &["sh"], &[rootfs], Some(writedir), None, None)?;
    child.wait()?;
    Ok(())
}
