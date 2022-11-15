#![cfg_attr(not(feature = "std"), no_std)]
#![feature(type_alias_impl_trait)]
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use embedded_hal::digital::OutputPin;
use embedded_io::asynch::{Read, Write};

pub(crate) mod fmt;

#[doc(hidden)]
mod buffer;
#[doc(hidden)]
mod parser;
#[doc(hidden)]
mod protocol;
mod types;

use buffer::*;
use protocol::*;
pub use types::*;

const RECV_BUFFER_LEN: usize = 256;

/// An instance of the RAK811 AT command driver.
pub struct Rak811Driver<T, RESET>
where
    T: Read + Write + Unpin,
    RESET: OutputPin,
{
    transport: T,
    reset: RESET,
    parse_buffer: Buffer,
    config: LoraConfig,
}

impl<T, RESET> Rak811Driver<T, RESET>
where
    T: Read + Write + Unpin,
    RESET: OutputPin,
{
    /// Create a new instance of the RAK811 at command driver.
    pub fn new(transport: T, reset: RESET) -> Self {
        Self {
            transport,
            reset,
            config: LoraConfig::new(),
            parse_buffer: Buffer::new(),
        }
    }

    /// Initialize the driver, reading the existing configuration.
    pub async fn initialize(&mut self) -> Result<(), LoraError> {
        self.reset.set_high().ok();
        self.reset.set_low().ok();
        loop {
            // Run processing to increase likelyhood we have something to parse.
            self.process().await?;
            if let Some(response) = self.parse() {
                match response {
                    Response::Initialized(region) => {
                        info!("Got initialize response with region {:?}", region);
                        self.config.region.replace(region);
                        return Ok(());
                    }
                    e => {
                        error!("Got unexpected repsonse: {:?}", e);
                        return Err(LoraError::NotInitialized);
                    }
                }
            }
        }
    }

    async fn process(&mut self) -> Result<(), LoraError> {
        let mut buf = [0; 1];
        self.transport
            .read(&mut buf[..])
            .await
            .map_err(|_| LoraError::RecvError)?;
        self.parse_buffer
            .write(buf[0])
            .map_err(|_| LoraError::RecvError)?;
        Ok(())
    }

    fn parse(&mut self) -> Option<Response> {
        let result = self.parse_buffer.parse();
        if let Ok(response) = result {
            if !matches!(response, Response::None) {
                debug!("Got response: {:?}", response);
                return Some(response);
            }
        }
        None
    }

    async fn recv(&mut self) -> Result<Response, LoraError> {
        let mut buf = [0; 1];
        loop {
            match self.transport.read(&mut buf[..]).await {
                Ok(len) => {
                    for b in &buf[..len] {
                        self.parse_buffer.write(*b).unwrap();
                    }
                    if let Some(response) = self.parse() {
                        return Ok(response);
                    }
                }
                Err(_) => {
                    error!("Error reading from uart");
                }
            }
        }
    }

    async fn send_command<'m>(&mut self, command: Command<'m>) -> Result<Response, LoraError> {
        let mut s = Command::buffer();
        command.encode(&mut s);
        debug!("Sending command {}", s.as_str());
        s.push_str("\r\n").unwrap();
        self.transport
            .write(s.as_bytes())
            .await
            .map_err(|_| LoraError::SendError)?;

        self.recv().await
    }

    async fn send_command_ok<'m>(&mut self, command: Command<'m>) -> Result<(), LoraError> {
        match self.send_command(command).await? {
            Response::Ok => Ok(()),
            _ => Err(LoraError::OtherError),
        }
    }

    /// Configure the driver using the provided LoraConfig.
    pub async fn configure(&mut self, config: &LoraConfig) -> Result<(), LoraError> {
        info!("Applying config: {:?}", config);
        if let Some(region) = config.region {
            if self.config.region != config.region {
                self.send_command_ok(Command::SetBand(region)).await?;
                self.config.region.replace(region);
            }
        }
        if let Some(lora_mode) = config.lora_mode {
            if self.config.lora_mode != config.lora_mode {
                self.send_command_ok(Command::SetMode(lora_mode)).await?;
                self.config.lora_mode.replace(lora_mode);
            }
        }
        debug!("Config applied");
        Ok(())
    }
}

impl<T, RESET> Rak811Driver<T, RESET>
where
    T: Read + Write + Unpin,
    RESET: OutputPin,
{
    /// Join a LoRaWAN network in the provided mode.
    pub async fn join(&mut self, mode: JoinMode) -> Result<(), LoraError> {
        let mode = match mode {
            JoinMode::OTAA {
                dev_eui,
                app_eui,
                app_key,
            } => {
                self.send_command_ok(Command::SetConfig(ConfigOption::DevEui(&dev_eui)))
                    .await?;
                self.send_command_ok(Command::SetConfig(ConfigOption::AppEui(&app_eui)))
                    .await?;
                self.send_command_ok(Command::SetConfig(ConfigOption::AppKey(&app_key)))
                    .await?;
                ConnectMode::OTAA
            }
            JoinMode::ABP {
                news_key,
                apps_key,
                dev_addr,
            } => {
                self.send_command_ok(Command::SetConfig(ConfigOption::DevAddr(&dev_addr)))
                    .await?;
                self.send_command_ok(Command::SetConfig(ConfigOption::AppsKey(&apps_key)))
                    .await?;
                self.send_command_ok(Command::SetConfig(ConfigOption::NwksKey(&news_key)))
                    .await?;
                ConnectMode::ABP
            }
        };
        let response = self.send_command(Command::Join(mode)).await?;
        match response {
            Response::Ok => {
                let response = self.recv().await?;
                match response {
                    Response::Recv(EventCode::JoinedSuccess, _, _, _) => Ok(()),
                    r => log_unexpected(r),
                }
            }
            r => log_unexpected(r),
        }
    }

    /// Send a message with the provided QoS and data on the given port.
    pub async fn send(&mut self, qos: QoS, port: Port, data: &[u8]) -> Result<(), LoraError> {
        let response = self.send_command(Command::Send(qos, port, data)).await?;
        match response {
            Response::Ok => {
                let response = self.recv().await?;
                let expected_code = match qos {
                    QoS::Unconfirmed => EventCode::TxUnconfirmed,
                    QoS::Confirmed => EventCode::TxConfirmed,
                };
                match response {
                    Response::Recv(c, 0, _, _) if expected_code == c => Ok(()),
                    r => log_unexpected(r),
                }
            }
            r => log_unexpected(r),
        }
    }
}

fn log_unexpected(r: Response) -> Result<(), LoraError> {
    error!("Unexpected response: {:?}", r);
    Err(LoraError::OtherError)
}
