/* Defines the builder functionality for the BaseContext with serial and
network transport. */

use crate::{BaseResult, base::BaseContext, config::ConnMode};
use serialport::{DataBits, FlowControl, Parity, StopBits};
use std::{marker::PhantomData, net::SocketAddrV4, net::TcpStream, str::FromStr};

const PARITY: Parity = Parity::None;
const DATABITS: DataBits = DataBits::Eight;
const FLOWCONTROL: FlowControl = FlowControl::None;
const STOPBITS: StopBits = StopBits::One;
const DEFAULT_BAUD: u32 = 115_200;
pub(crate) const TCP_PORT: u16 = 2000;

// Type-state Builder states for the BaseContextBuilder
pub struct Init;
pub struct Serial;
pub struct Network;

/// Type-State Builder for the Controller type based on connection mode.
pub struct BaseContextBuilder<T> {
    conn_mode: ConnMode,
    ip_addr: Option<SocketAddrV4>,
    com_port: Option<String>,
    baud_rate: Option<u32>,
    _marker: PhantomData<T>,
}
impl BaseContextBuilder<Init> {
    /// Starts the type-state builder pattern
    pub fn new() -> BaseContextBuilder<Init> {
        Self {
            com_port: None,
            conn_mode: ConnMode::Serial,
            ip_addr: None,
            baud_rate: None,
            _marker: PhantomData,
        }
    }
    /// Continues in the path to build the controller using serial (USB or RS-422).
    pub fn with_serial(self, com_port: &str) -> BaseContextBuilder<Serial> {
        BaseContextBuilder {
            conn_mode: ConnMode::Serial,
            ip_addr: None,
            com_port: Some(com_port.into()),
            baud_rate: Some(DEFAULT_BAUD),
            _marker: PhantomData,
        }
    }
    /// Continies in the path to build the controller using IP.
    pub fn with_network(self, v4_addr: &str) -> BaseResult<BaseContextBuilder<Network>> {
        let v4_addr = SocketAddrV4::from_str(&format!("{}:{}", v4_addr, TCP_PORT))?;
        Ok(BaseContextBuilder {
            conn_mode: ConnMode::Network,
            ip_addr: Some(v4_addr),
            com_port: None,
            baud_rate: None,
            _marker: PhantomData,
        })
    }
}
impl BaseContextBuilder<Serial> {
    pub fn baud(mut self, baud: u32) -> Self {
        self.baud_rate = Some(baud);
        self
    }
    /// Builds the controller type and tries to connect over serial.
    pub fn build(self) -> BaseResult<BaseContext> {
        // Try to bind to a serial port handle and return newly built instance
        let io = serialport::new(
            self.com_port
                .as_ref()
                .expect("COM port required to get to serial build method."),
            self.baud_rate
                .expect("Baud rate required to get to serial build method."),
        )
        .data_bits(DATABITS)
        .parity(PARITY)
        .flow_control(FLOWCONTROL)
        .stop_bits(STOPBITS)
        .open()?;

        let mut ret = BaseContext::new(
            self.conn_mode,
            self.ip_addr,
            self.com_port,
            Some(io),
            None,
            self.baud_rate,
        );
        let _ = ret.get_module_list();
        Ok(ret)
    }
}
impl BaseContextBuilder<Network> {
    pub fn build(self) -> BaseResult<BaseContext> {
        // Try to connect to TCP socket and return newly built instance
        let tcp_con = TcpStream::connect(
            self.ip_addr
                .as_ref()
                .expect("Some required to get to this method."),
        )?;
        tcp_con.set_nonblocking(true)?;

        let mut ret = BaseContext::new(
            self.conn_mode,
            self.ip_addr,
            self.com_port,
            None,
            Some(tcp_con),
            self.baud_rate,
        );
        // Attempt to fill module list. If unable, fallback to default of Empty
        let _ = ret.get_module_list();
        Ok(ret)
    }
}
