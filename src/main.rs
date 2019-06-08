use std::time::Duration;
use std::thread;

use libusb::{Context, Error as UsbError};
use failure::Error;
use failure::{format_err, bail};

fn main() {
    match run() {
        Ok(_) => { }
        Err(e) => println!("{}", e),
    }

    // keep open on windows
    if cfg!(target_os = "windows") {
        thread::sleep(Duration::from_secs(9999999999999));
    }
}

fn run() -> Result<(), Error> {
    let context = Context::new().map_err(|e| format_err!("Failed to create libusb context: {}", e))?;
    let devices = context.devices().map_err(|e| format_err!("Failed to create libusb devices: {}", e))?;

    let mut device = None;
    for the_device in devices.iter() {
        if let Ok(device_desc) = the_device.device_descriptor() {
            if device_desc.vendor_id() == 0x057E && device_desc.product_id() == 0x0337 {
                device = Some(the_device);
            }
            else {
                println!("Unknown usb device with vendor_id 0x{:x} and product_id 0x{:x}", device_desc.vendor_id(), device_desc.product_id());
            }
        }
    }

    let device = if let Some(device) = device {
        device
    } else {
        bail!("No GC adapter found");
    };

    let mut handle = match device.open() {
        Ok(handle) => handle,
        Err(e) => {
            let access_solution = if cfg!(target_os = "linux") { r#":
You need to set a udev rule so that the adapter can be accessed.
To fix this on most Linux distributions, run the following command and then restart your computer.
echo 'SUBSYSTEM=="usb", ENV{DEVTYPE}=="usb_device", ATTRS{idVendor}=="057e", ATTRS{idProduct}=="0337", TAG+="uaccess"' | sudo tee /etc/udev/rules.d/51-gcadapter.rules"#
            } else { "" };

            let driver_solution = if cfg!(target_os = "windows") { r#":
To use your GC adapter you must:
1. Download and run Zadig: http://zadig.akeo.ie/
2. Options -> List all devices
3. In the pulldown menu, Select WUP-028
4. On the right ensure WinUSB is selected
5. Select Replace Driver
6. Select yes in the dialog box
7. Restart your computer"# // You dont really need to restart your computer, but dolphin etc probably needs to be, safest to just tell them to restart the computer :P
            } else { "" };

            match e {
                UsbError::Access       => bail!("Permissions error{}", access_solution),
                UsbError::NotSupported => bail!("Not supported error{}", driver_solution),
                _                      => bail!("Failed to open handle: {:?}", e),
            }
        }
    };

    match handle.kernel_driver_active(0) {
        Ok(true) => {
            if let Err(e) = handle.detach_kernel_driver(0) {
                bail!("Failed to detach kernel driver: {}", e);
            }
        }
        Ok(false) => { /* All good */ }
        Err(_) => { /* TODO: I think this is fine because PF Sandbox code wasnt unwrapping it, should verify though */ }
    }

    handle.claim_interface(0).map_err(|e| format_err!("Failed to claim interface on GC adapter: {}", e))?;

    // Tell adapter to start reading
    let payload = [0x13];
    handle.write_interrupt(0x2, &payload, Duration::new(1, 0)).map_err(|e| format_err!("Failed to initialize GC adapter: {}", e))?;

    // Old data can be kept around so throw away the first 99 results
    let mut data: [u8; 37] = [0; 37];
    for _ in 0..100 {
        handle.read_interrupt(0x81, &mut data, Duration::new(1, 0)).map_err(|e| format_err!("Failed to read data from GC adapter: {}", e))?;
    }

    // process data
    for port in 0..4 {
        let plugged_in = data[9*port+1] == 20 || data[9*port+1] == 16;

        if plugged_in {
            println!("GC Adapter port {}: plugged in", port + 1);
        } else {
            println!("GC Adapter port {}: unplugged", port + 1);
        }
    }

    Ok(())
}
