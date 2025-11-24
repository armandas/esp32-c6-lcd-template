#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::i2c::master::I2c;
use esp_hal::main;
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::{gpio, spi};
use log::info;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation, Rotation};
use static_cell::StaticCell;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

//     ┌─────────────────────────┐
//     │ +──────────────────── X │
//    ┌┐ │                       │
// ====│ │  Display Orientation  │
//    └┘ │                       │
//     │ Y                       │
//     └─────────────────────────┘
const DISPLAY_OFFSET_X: u16 = 34;
const DISPLAY_OFFSET_Y: u16 = 0;
const DISPLAY_SIZE_H: u16 = 172; // Y-axis
const DISPLAY_SIZE_W: u16 = 320; // X-axis

static SPI_BUFFER: StaticCell<[u8; 4096]> = StaticCell::new();

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();

    // Configure I2C
    let i2c_config = esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(50));
    let i2c = I2c::new(peripherals.I2C0, i2c_config)
        .expect("could not create I2C instance")
        .with_sda(peripherals.GPIO18)
        .with_scl(peripherals.GPIO19);

    // Configure touch driver
    let touch_driver_reset_pin = gpio::Output::new(
        peripherals.GPIO20,
        gpio::Level::Low,
        gpio::OutputConfig::default(),
    );
    let mut touch_driver = axs5106l::Axs5106l::new(
        i2c,
        touch_driver_reset_pin,
        DISPLAY_SIZE_W,
        DISPLAY_SIZE_H,
        // Note: touch panel and LCD rotations are different.
        axs5106l::Rotation::Rotate270,
    );
    touch_driver
        .init(&mut delay)
        .expect("failed to initialize the touch driver");

    // Configure SPI
    let spi = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(spi::Mode::_0),
    )
    .expect("could not create SPI instance")
    .with_sck(peripherals.GPIO1)
    .with_mosi(peripherals.GPIO2);
    let spi = embedded_hal_bus::spi::ExclusiveDevice::new(
        spi,
        gpio::Output::new(
            peripherals.GPIO14,
            gpio::Level::High,
            gpio::OutputConfig::default(),
        ),
        delay.clone(),
    )
    .expect("could not create SPI bus device");

    // Configure SPI interface for display
    let buffer = SPI_BUFFER.init([0; 4096]);
    let spi_interface = SpiInterface::new(
        spi,
        gpio::Output::new(
            peripherals.GPIO15,
            gpio::Level::High,
            gpio::OutputConfig::default(),
        ),
        buffer,
    );

    // Configure display driver
    let mut display = mipidsi::Builder::new(ST7789, spi_interface)
        .invert_colors(ColorInversion::Normal)
        .color_order(mipidsi::options::ColorOrder::Bgr)
        .reset_pin(gpio::Output::new(
            peripherals.GPIO22,
            gpio::Level::High,
            gpio::OutputConfig::default(),
        ))
        .display_offset(DISPLAY_OFFSET_X, DISPLAY_OFFSET_Y)
        .display_size(DISPLAY_SIZE_H, DISPLAY_SIZE_W) // H/W swapped because of rotation
        .orientation(Orientation::new().rotate(Rotation::Deg90).flip_horizontal())
        .init(&mut delay)
        .expect("Failed to init display");

    // Initialize display
    display
        .clear(Rgb565::WHITE)
        .expect("Failed to clear display");

    // Turn on display backlight
    let _backlight = gpio::Output::new(
        peripherals.GPIO23,
        gpio::Level::High,
        gpio::OutputConfig::default(),
    );

    loop {}
}
