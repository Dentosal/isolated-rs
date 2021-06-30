use std::env::current_dir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let d = current_dir().unwrap();

    let rootfs = d.join("rootfs/");
    let writedir = d.join("write/");

    std::fs::create_dir_all(&writedir)?;

    let mut child = isolated::Command::new(rootfs, "/bin/sh")
        .disk_write_to(writedir)
        .spawn()?;

    child.wait()?;
    Ok(())
}
