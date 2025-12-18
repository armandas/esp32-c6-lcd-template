use core::ops::Shl;

use embedded_hal::i2c::I2c;

#[derive(Debug)]
pub struct Qmi8658a<I2C: I2c> {
    i2c: I2C,
    address: u8,
}

#[derive(Debug)]
pub struct ImuData {
    pub accel_x: i16,
    pub accel_y: i16,
    pub accel_z: i16,
    pub gyro_x: i16, // Pitch
    pub gyro_y: i16, // Roll
    pub gyro_z: i16, // Yaw
}

mod registers {
    pub const WHO_AM_I: u8 = 0x00;
    pub const CTRL1: u8 = 0x02;
    pub const CTRL7: u8 = 0x08;
    pub const TEMP_L: u8 = 0x33;
    pub const AX_L: u8 = 0x35;
}

impl<I2C: I2c> Qmi8658a<I2C> {
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    pub fn read_chip_id(&mut self) -> Result<u8, I2C::Error> {
        let mut id = [0];
        self.i2c
            .write_read(self.address, &[registers::WHO_AM_I], &mut id)?;
        Ok(id[0])
    }

    pub fn initialize(&mut self) -> Result<(), I2C::Error> {
        let control1: u8 = 0b0110_0000;
        // CTRL7 gSN=0, aEN=1, gEN=1
        let control7: u8 = 0b0000_0011;
        self.i2c
            .write(self.address, &[registers::CTRL1, control1])?;
        self.i2c
            .write(self.address, &[registers::CTRL7, control7])?;
        Ok(())
    }

    pub fn read_temperature(&mut self) -> Result<i16, I2C::Error> {
        let mut temperature = [0; 2];
        self.i2c
            .write_read(self.address, &[registers::TEMP_L], &mut temperature)?;
        Ok(i16::from_le_bytes(temperature))
    }

    pub fn read_imu_data(&mut self) -> Result<ImuData, I2C::Error> {
        let mut imu = [0; 12];
        self.i2c
            .write_read(self.address, &[registers::AX_L], &mut imu)?;

        Ok(ImuData {
            accel_x: i16::from_le_bytes(imu[0..2].try_into().unwrap()),
            accel_y: i16::from_le_bytes(imu[2..4].try_into().unwrap()),
            accel_z: i16::from_le_bytes(imu[4..6].try_into().unwrap()),
            gyro_x: i16::from_le_bytes(imu[6..8].try_into().unwrap()),
            gyro_y: i16::from_le_bytes(imu[8..10].try_into().unwrap()),
            gyro_z: i16::from_le_bytes(imu[10..12].try_into().unwrap()),
        })
    }
}
