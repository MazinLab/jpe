use jpe::BaseContextBuilder;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = BaseContextBuilder::new()
        .with_serial_async("/dev/my_usb")
        .build()?;
    let mut ctx = BaseContextBuilder::new()
        .with_network_async("192.168.10.1")?
        .build()?;
    Ok(())
}
