#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::i2c::master::I2c;
use esp_hal::main;
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use esp_hal::{gpio, spi};
use log::info;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

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
        172,
        320,
        axs5106l::Rotation::Rotate0,
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

    loop {}
}
