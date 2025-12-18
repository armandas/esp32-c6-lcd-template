#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

extern crate alloc;
use alloc::format;

use core::cell::RefCell;
use embedded_hal_bus::i2c::RefCellDevice;

use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    primitives::Rectangle,
    text::Text,
};
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_graphics_framebuf::FrameBuf;
use esp_hal::delay::Delay;
use esp_hal::i2c::master::I2c;
use esp_hal::main;
use esp_hal::spi::master::Spi;
use esp_hal::time::Instant;
use esp_hal::time::Rate;
use esp_hal::{
    clock::CpuClock,
    dma::{DmaRxBuf, DmaTxBuf},
    dma_buffers,
};
use esp_hal::{gpio, spi};
use log::{error, info};
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation, Rotation};
use static_cell::StaticCell;

use hello_display::qmi8658a::Qmi8658a;

#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    error!("{panic_info}");
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
    esp_alloc::heap_allocator!(size: 1024);

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = Delay::new();

    // Configure I2C
    let i2c_config = esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(50));
    let i2c = I2c::new(peripherals.I2C0, i2c_config)
        .expect("could not create I2C instance")
        .with_sda(peripherals.GPIO18)
        .with_scl(peripherals.GPIO19);
    let i2c = RefCell::new(i2c);

    // Configure touch driver
    let touch_driver_reset_pin = gpio::Output::new(
        peripherals.GPIO20,
        gpio::Level::Low,
        gpio::OutputConfig::default(),
    );
    let mut touch_driver = axs5106l::Axs5106l::new(
        RefCellDevice::new(&i2c),
        touch_driver_reset_pin,
        DISPLAY_SIZE_W,
        DISPLAY_SIZE_H,
        // Note: touch panel and LCD rotations are different.
        axs5106l::Rotation::Rotate270,
    );
    touch_driver
        .init(&mut delay)
        .expect("failed to initialize the touch driver");

    // Configure IMU
    let mut imu = Qmi8658a::new(RefCellDevice::new(&i2c), 0x6b);

    // Configure DMA
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(10 * 1024);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    // Configure SPI
    let spi = Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(spi::Mode::_0),
    )
    .expect("could not create SPI instance")
    .with_sck(peripherals.GPIO1)
    .with_mosi(peripherals.GPIO2)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf);
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

    let mut data = [Rgb565::BLACK; DISPLAY_SIZE_W as usize * DISPLAY_SIZE_H as usize];
    let mut frame_buffer =
        FrameBuf::new(&mut data, DISPLAY_SIZE_W as usize, DISPLAY_SIZE_H as usize);

    let character_style = MonoTextStyle::new(&FONT_10X20, Rgb565::BLACK);
    let mut text = Text::new("Hello, World!", Point::new(90, 0), character_style);
    let mut y = 0;

    imu.initialize().expect("failed to initialize IMU");
    match imu.read_chip_id() {
        Ok(id) => info!("IMU ID: {id}"),
        Err(err) => error!("Error reading chip id: {err}"),
    }

    loop {
        if let Ok(temperature) = imu.read_temperature() {
            info!("Temperature: {temperature:#06X} {}", temperature as f32 / 256f32);
        }

        frame_buffer.clear(Rgb565::WHITE).ok();

        let message = format!(
            "Current timestamp: {} ms",
            Instant::now().duration_since_epoch().as_millis()
        );
        let text2 = Text::new(&message, Point::new(30, 20 + y), character_style);
        text2.draw(&mut frame_buffer).ok();

        text.position.y = y;
        text.draw(&mut frame_buffer).ok();

        if y == DISPLAY_SIZE_H as i32 + 20 {
            y = 0;
        } else {
            y += 1;
        }

        // let start = Instant::now();
        let area = Rectangle::new(Point::zero(), frame_buffer.size());
        display
            .fill_contiguous(&area, frame_buffer.data.iter().copied())
            .ok();
        // info!("{}", start.elapsed());
    }
}
