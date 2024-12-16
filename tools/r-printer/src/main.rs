use anyhow::Context;
use clap::Parser;
use escpos::driver::NativeUsbDriver;
use escpos::printer::Printer;
use escpos::utils::{JustifyMode, Protocol, QRCodeCorrectionLevel, QRCodeModel, QRCodeOption};

const MANUFACTURER: &str = "EPSON";
const PRODUCT_NAME: &str = "TM-T88V";

const URI_PREFIX: &str = "https://studyport.net/l/";

#[derive(Parser)]
struct Args {
    /// Print all usb devices
    #[clap(short, long, default_value_t = false)]
    debug: bool,

    /// The code of the location (will be appended to the URI_PREFIX)
    #[clap(short, long, required = true, value_parser = lowercase_non_empty_string_parser)]
    name: String,

    /// The text to print below the QR code
    #[clap(short, long, required = false)]
    subtext: Option<String>,

    /// The amount of copies to print
    #[clap(short, long, required = false, default_value_t = 1, value_parser = non_zero_i8_parser)]
    amount: i8,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let printer_usb_device = nusb::list_devices()?
        .inspect(|d| {
            if args.debug {
                println!("{:?}", d);
            }
        })
        .find(|d| {
            d.manufacturer_string().unwrap_or_default() == MANUFACTURER
                && d.product_string().unwrap_or_default() == PRODUCT_NAME
        })
        .context("printer not found")?;

    println!(
        "printer found with vendor id: {:#06x}, product id: {:#06x}",
        printer_usb_device.vendor_id(),
        printer_usb_device.product_id()
    );

    let mut printer = Printer::new(
        NativeUsbDriver::open(printer_usb_device.vendor_id(), printer_usb_device.product_id())?,
        Protocol::default(),
        None,
    );

    printer.init()?;

    let qr_code = format!("{URI_PREFIX}{}", args.name);
    println!("qr code link: {qr_code}");

    for i in 0..args.amount {
        println!("{}/{}", i + 1, args.amount);

        printer.justify(JustifyMode::CENTER)?.qrcode_option(
            &qr_code,
            QRCodeOption::new(QRCodeModel::Model2, 16, QRCodeCorrectionLevel::H),
        )?;

        if let Some(subtext) = &args.subtext {
            printer.feed()?.smoothing(true)?.bold(true)?.size(2, 2)?.write(subtext)?;
        }

        printer.feeds(3)?.print_cut()?;
    }

    Ok(())
}

fn lowercase_non_empty_string_parser(s: &str) -> Result<String, &'static str> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        Err("empty string")
    } else {
        Ok(s)
    }
}

fn non_zero_i8_parser(s: &str) -> Result<i8, &'static str> {
    match s.parse::<i8>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err("not a positive integer"),
    }
}
