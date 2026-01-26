use embedded_hal::i2c::I2c;

const STATUS_INT_AVAIL: u8 = 1 << 0;
const STATUS_INT_LOCKED: u8 = 1 << 1;

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

#[derive(Debug)]
pub struct Config {
    ctrl1: u8,
    ctrl7: u8,
}

mod registers {
    pub const WHO_AM_I: u8 = 0x00;
    pub const CTRL1: u8 = 0x02;
    pub const CTRL7: u8 = 0x08;
    pub const STATUSINT: u8 = 0x2d;
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

    pub fn initialize(&mut self, config: Config) -> Result<(), I2C::Error> {
        self.i2c
            .write(self.address, &[registers::CTRL1, config.ctrl1])?;
        self.i2c
            .write(self.address, &[registers::CTRL7, config.ctrl7])?;
        Ok(())
    }

    pub fn read_temperature(&mut self) -> Result<i16, I2C::Error> {
        let mut temperature = [0; 2];
        self.i2c
            .write_read(self.address, &[registers::TEMP_L], &mut temperature)?;
        Ok(i16::from_le_bytes(temperature))
    }

    pub fn read_status_int(&mut self) -> Result<u8, I2C::Error> {
        let mut status_int = [0; 1];
        self.i2c
            .write_read(self.address, &[registers::STATUSINT], &mut status_int)?;
        Ok(status_int[0])
    }

    pub fn read_imu_data(&mut self) -> Result<Option<ImuData>, I2C::Error> {
        let mut status_int = self.read_status_int()?;
        if (status_int & STATUS_INT_AVAIL) != STATUS_INT_AVAIL {
            // Data not available yet
            return Ok(None);
        }

        // Wait until data is locked
        while (status_int & STATUS_INT_LOCKED) != STATUS_INT_LOCKED {
            status_int = self.read_status_int()?;
        }

        let mut imu = [0; 12];
        self.i2c
            .write_read(self.address, &[registers::AX_L], &mut imu)?;

        Ok(Some(ImuData {
            accel_x: i16::from_le_bytes(imu[0..2].try_into().unwrap()),
            accel_y: i16::from_le_bytes(imu[2..4].try_into().unwrap()),
            accel_z: i16::from_le_bytes(imu[4..6].try_into().unwrap()),
            gyro_x: i16::from_le_bytes(imu[6..8].try_into().unwrap()),
            gyro_y: i16::from_le_bytes(imu[8..10].try_into().unwrap()),
            gyro_z: i16::from_le_bytes(imu[10..12].try_into().unwrap()),
        }))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ctrl1: 0b0010_0000,
            ctrl7: 0b0000_0000,
        }
    }
}

impl Config {
    pub fn with_internal_high_speed_oscillator_disabled(mut self) -> Config {
        self.ctrl1 |= 1 << 0;
        self
    }

    pub fn with_fifo_interrupt_mapped_to_int1(mut self) -> Config {
        self.ctrl1 |= 1 << 2;
        self
    }

    pub fn with_int1_pin_output_enabled(mut self) -> Config {
        self.ctrl1 |= 1 << 3;
        self
    }

    pub fn with_int2_pin_output_enabled(mut self) -> Config {
        self.ctrl1 |= 1 << 4;
        self
    }

    pub fn with_little_endian_data(mut self) -> Config {
        // Note: the bit is set by default, so we provide an API to clear it.
        self.ctrl1 &= !(1 << 5);
        self
    }

    pub fn with_address_auto_increment(mut self) -> Config {
        self.ctrl1 |= 1 << 6;
        self
    }

    pub fn with_3_wire_spi(mut self) -> Config {
        self.ctrl1 |= 1 << 7;
        self
    }

    pub fn with_accelerometer_enabled(mut self) -> Config {
        self.ctrl7 |= 1 << 0;
        self
    }

    pub fn with_gyroscope_enabled(mut self) -> Config {
        self.ctrl7 |= 1 << 1;
        self
    }

    pub fn with_gyro_in_snooze_mode(mut self) -> Config {
        self.ctrl7 |= 1 << 4;
        self
    }

    pub fn with_data_ready_disabled(mut self) -> Config {
        self.ctrl7 |= 1 << 5;
        self
    }

    pub fn with_sync_sample_enabled(mut self) -> Config {
        self.ctrl7 |= 1 << 7;
        self
    }
}
