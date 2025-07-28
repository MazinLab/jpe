use jpe::BaseContextBuilder;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = BaseContextBuilder::new()
        .with_serial("/dev/my_usb")
        .build()?;
    Ok(())
}
