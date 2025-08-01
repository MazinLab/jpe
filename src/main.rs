use jpe::BaseContextBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = BaseContextBuilder::new()
        .with_serial_async("/dev/cu.usbserial-D30IYJT2")
        .build()?;
    //let mut ctx = BaseContextBuilder::new()
    //    .with_network_async("192.168.10.100")?
    //    .build()?;
    println!("{:?}", ctx.get_fw_version().await?);
    println!("{:?}", ctx.get_module_list().await?);
    println!("{:?}", ctx.get_supported_stages().await?);
    println!("{:?}", ctx.get_ip_config().await?);
    println!(
        "{:?}",
        ctx.get_baud_rate(jpe::SerialInterface::Rs422).await?
    );
    println!("{:?}", ctx.get_mod_fw_version(jpe::Slot::One).await?);
    println!("{:?}", ctx.get_fail_safe_state(jpe::Slot::One).await?);

    Ok(())
}
