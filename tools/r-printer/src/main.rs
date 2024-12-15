use anyhow::Context;
use escpos::driver::NativeUsbDriver;
use escpos::printer::Printer;
use escpos::utils::{JustifyMode, Protocol, QRCodeCorrectionLevel, QRCodeModel, QRCodeOption};
use inquire::validator::Validation;
use inquire::{Confirm, CustomType, Text};

const MANUFACTURER: &str = "EPSON";
const PRODUCT_NAME: &str = "TM-T88V";

const URI_PREFIX: &str = "https://studyport.net/l/";

fn main() -> anyhow::Result<()> {
    let verbose = std::env::args().any(|a| &a == "-v" || &a == "--verbose");

    let (vendor_id, product_id) = nusb::list_devices()?
        .inspect(|d| {
            if verbose {
                println!("{:?}", d);
            }
        })
        .find(|d| {
            d.manufacturer_string().unwrap_or_default() == MANUFACTURER
                && d.product_string().unwrap_or_default() == PRODUCT_NAME
        })
        .map(|d| (d.vendor_id(), d.product_id()))
        .context("printer not found")?;

    let name = Text::new("location code?")
        .with_formatter(&str::to_lowercase)
        .with_validator(|n: &str| {
            Ok(if !n.is_empty() {
                Validation::Valid
            } else {
                Validation::Invalid("not enough".into())
            })
        })
        .prompt()?;

    let subtext = Text::new("subtext?").with_default(&name).prompt().unwrap_or_default();

    let amount = CustomType::<i8>::new("how many to print?")
        .with_default(1)
        .with_validator(|&a: &i8| {
            Ok(if a > 0 { Validation::Valid } else { Validation::Invalid("not enough".into()) })
        })
        .prompt()?;

    let mut printer =
        Printer::new(NativeUsbDriver::open(vendor_id, product_id)?, Protocol::default(), None);

    if !Confirm::new("print?").prompt()? {
        return Ok(());
    }

    printer.init()?;

    let qr_code = format!("{URI_PREFIX}{name}");
    println!("qr code link: {qr_code}");

    for i in 0..amount {
        println!("{}/{amount}", i + 1);

        printer.justify(JustifyMode::CENTER)?.qrcode_option(
            &qr_code,
            QRCodeOption::new(QRCodeModel::Model2, 16, QRCodeCorrectionLevel::H),
        )?;

        if !subtext.is_empty() {
            printer.feed()?.smoothing(true)?.bold(true)?.size(2, 2)?.write(&subtext)?;
        }

        printer.feeds(3)?.print_cut()?;
    }

    Ok(())
}
