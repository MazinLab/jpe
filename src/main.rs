use jpe::BaseContextBuilder;
#[cfg(feature = "sync")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Building context");
    //let mut ctx = BaseContextBuilder::new()
    //    .with_serial("/dev/cu.usbserial-D30IYJT2")
    //    .build()?;
    let mut ctx = BaseContextBuilder::new()
        .with_network("192.168.10.5")?
        .build()?;
    println!("Context built");
    println!("{:?}", ctx.get_fw_version()?);
    println!("{:?}", ctx.get_module_list()?);
    println!("{:?}", ctx.get_supported_stages()?);
    println!("{:?}", ctx.get_ip_config()?);
    println!("{:?}", ctx.get_mod_fw_version(jpe::Slot::One)?);
    println!("{:?}", ctx.get_fail_safe_state(jpe::Slot::One)?);
    //println!("{:?}", ctx.set_ip_config(jpe::IpAddrMode::Static, "192.168.10.5", "255.255.255.0", "192.168.10.1").await?);

    Ok(())
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Building async context");
    let mut ctx = BaseContextBuilder::new()
        .with_serial_async("/dev/cu.usbserial-D30IYJT2")
        .build()
        .await?;
    //let mut ctx = BaseContextBuilder::new()
    //    .with_network_async("192.168.10.5")?
    //    .build()
    //    .await?;
    println!("Async context built");
    println!("{:?}", ctx.get_fw_version().await?);
    println!("{:?}", ctx.get_module_list().await?);
    println!("{:?}", ctx.get_supported_stages().await?);
    println!("{:?}", ctx.get_ip_config().await?);
    println!(
      //"{:?}",
      //ctx.get_baud_rate(jpe::SerialInterface::Rs422).await?
    );
    println!("{:?}", ctx.get_mod_fw_version(jpe::Slot::One).await?);
    println!("{:?}", ctx.get_fail_safe_state(jpe::Slot::One).await?);
    //println!("{:?}", ctx.set_ip_config(jpe::IpAddrMode::Static, "192.168.10.5", "255.255.255.0", "192.168.10.1").await?);

    Ok(())
}
