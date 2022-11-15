#![macro_use]
#![feature(type_alias_impl_trait)]

mod serial;

use async_io::Async;
use embassy_time::{Duration, Timer};
use embedded_hal::digital::{ErrorType, OutputPin};
use embedded_io::adapters::FromFutures;
use nix::sys::termios;
use rak811_at_driver::*;
use serial::*;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    let baudrate = termios::BaudRate::B115200;
    let port = SerialPort::new("/dev/ttyUSB0", baudrate).unwrap();
    let port = Async::new(port).unwrap();
    let port = futures::io::BufReader::new(port);
    let port = FromFutures::new(port);

    let reset_pin = DummyPin {};

    // TODO: Modify with your credentials.
    let join_mode = JoinMode::OTAA {
        dev_eui: [0, 0, 0, 0, 0, 0, 0, 0].into(),
        app_eui: [0, 0, 0, 0, 0, 0, 0, 0].into(),
        app_key: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].into(),
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN);

    let mut modem = Rak811Driver::new(port, reset_pin);
    modem.initialize().await.unwrap();
    modem.configure(&config).await.unwrap();

    modem.join(join_mode).await.unwrap();

    loop {
        modem.send(QoS::Confirmed, 1, b"PING").await.unwrap();
        Timer::after(Duration::from_secs(60)).await;
    }
}

pub struct DummyPin;
impl ErrorType for DummyPin {
    type Error = ();
}

impl OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), ()> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), ()> {
        Ok(())
    }
}
