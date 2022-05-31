use udev;

pub fn print_device_information(path: &str) -> std::io::Result<()> {
    println!("device: {path}");
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_property("DEVNAME", path)?;
    if let Some(device) = enumerator.scan_devices().unwrap().next() {
        println!(" [properties]");
        for property in device.properties() {
            println!("  - {:?} {:?}", property.name(), property.value());
        }

        println!(" [attributes]");
        for attribute in device.attributes() {
            println!("  - {:?} {:?}", attribute.name(), attribute.value());
        }
    }

    return Ok(());
}

pub fn get_device_links(path: &str) -> std::io::Result<Vec<String>> {
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_property("DEVNAME", path)?;
    if let Some(device) = enumerator.scan_devices().unwrap().next() {
        for property in device.properties() {
            if "DEVLINKS" == property.name() {
                return Ok(property
                    .value()
                    .to_str()
                    .ok_or(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Failed to convert property name.",
                    ))?
                    .split_whitespace()
                    .map(str::to_string)
                    .collect());
            }
        }
    }

    return Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Failed to find device or property.",
    ));
}
