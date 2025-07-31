/* Defines the builder functionality for the BaseContext with serial and
network transport. */

use crate::{
    BaseResult,
    base::{BaseContext, BaseContextAsync},
    transport::{Connection, ConnectionAsync},
};
use serial2::SerialPort;
use serial2_tokio::SerialPort as SerialPortAsync;
use std::{
    marker::PhantomData,
    net::{SocketAddrV4, TcpStream},
    str::FromStr,
    time::Duration,
};
use tokio::net::TcpStream as TcpStreamAsync;

const DEFAULT_BAUD: u32 = 115_200;
pub(crate) const TCP_PORT: u16 = 2000;
const DEFAULT_CONN_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) type AsyncSerialConn = ConnectionAsync<SerialPortAsync>;
pub(crate) type AsyncNetConn = ConnectionAsync<TcpStreamAsync>;

// Type-state Builder states for the BaseContextBuilder
pub struct Init;
pub struct Serial;
pub struct Network;
pub struct SerialAsync;
pub struct NetworkAsync;

/// Type-State Builder for the Controller type based on connection mode.
pub struct BaseContextBuilder<T> {
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
            ip_addr: None,
            baud_rate: None,
            _marker: PhantomData,
        }
    }
    /// Continues in the path to build the controller using serial (USB or RS-422).
    pub fn with_serial(self, com_port: &str) -> BaseContextBuilder<Serial> {
        BaseContextBuilder {
            ip_addr: None,
            com_port: Some(com_port.into()),
            baud_rate: Some(DEFAULT_BAUD),
            _marker: PhantomData,
        }
    }
    /// Continues in the path to build the controller using serial (USB or RS-422) in
    /// an async runtime.
    pub fn with_serial_async(self, com_port: &str) -> BaseContextBuilder<SerialAsync> {
        BaseContextBuilder {
            ip_addr: None,
            com_port: Some(com_port.into()),
            baud_rate: Some(DEFAULT_BAUD),
            _marker: PhantomData,
        }
    }
    /// Continues in the path to build the controller using IP.
    pub fn with_network(self, v4_addr: &str) -> BaseResult<BaseContextBuilder<Network>> {
        let v4_addr = SocketAddrV4::from_str(&format!("{}:{}", v4_addr, TCP_PORT))?;
        Ok(BaseContextBuilder {
            ip_addr: Some(v4_addr),
            com_port: None,
            baud_rate: None,
            _marker: PhantomData,
        })
    }
    /// Continues in the path to build the controller using IP in an async runtime.
    pub fn with_network_async(self, v4_addr: &str) -> BaseResult<BaseContextBuilder<NetworkAsync>> {
        let v4_addr = SocketAddrV4::from_str(&format!("{}:{}", v4_addr, TCP_PORT))?;
        Ok(BaseContextBuilder {
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
        let io = SerialPort::open(
            self.com_port
                .as_ref()
                .expect("COM port required to get to serial build method."),
            self.baud_rate
                .expect("Baud rate required to get to serial build method."),
        )?;

        // Build connection
        let conn = Connection::new(io);

        // Try to init module list
        let mut ret = BaseContext::new(Box::new(conn));
        let _ = ret.get_module_list();
        Ok(ret)
    }
}
impl BaseContextBuilder<SerialAsync> {
    pub fn baud(mut self, baud: u32) -> Self {
        self.baud_rate = Some(baud);
        self
    }
    /// Builds the controller type and tries to connect over serial in an async runtime.
    pub fn build(self) -> BaseResult<BaseContextAsync<AsyncSerialConn>> {
        // Try to bind to a serial port handle and return newly built instance
        let io = SerialPortAsync::open(
            self.com_port
                .as_ref()
                .expect("COM port required to get to serial build method."),
            self.baud_rate
                .expect("Baud rate required to get to serial build method."),
        )?;

        // Build connection
        let conn = ConnectionAsync::new(io);

        // Try to init module list
        let mut ret = BaseContextAsync::new(conn);
        let _ = ret.get_module_list();
        Ok(ret)
    }
}
impl BaseContextBuilder<Network> {
    pub fn build(self) -> BaseResult<BaseContext> {
        // Try to connect to TCP socket and return newly built instance. TcpStream
        // automatically set in non-blocking mode with `connect_timeout()`
        let tcp_con = TcpStream::connect_timeout(
            &self
                .ip_addr
                .expect("IP address required to get to network build method.")
                .into(),
            DEFAULT_CONN_TIMEOUT,
        )?;
        tcp_con.set_nonblocking(true)?;
        // Build connection
        let conn = Connection::new(tcp_con);

        // Try to init module list
        let mut ret = BaseContext::new(Box::new(conn));
        let _ = ret.get_module_list();
        Ok(ret)
    }
}
impl BaseContextBuilder<NetworkAsync> {
    pub fn build(self) -> BaseResult<BaseContextAsync<AsyncNetConn>> {
        // Try to connect to TCP socket and return newly built instance. TcpStream
        // automatically set in non-blocking mode with `connect_timeout()`
        let tcp_con = TcpStream::connect_timeout(
            &self
                .ip_addr
                .expect("IP address required to get to network build method.")
                .into(),
            DEFAULT_CONN_TIMEOUT,
        )?;
        tcp_con.set_nonblocking(true)?;

        // Try to consume the connection and turn into async
        let tcp_con = TcpStreamAsync::from_std(tcp_con)?;

        // Build connection
        let conn = ConnectionAsync::new(tcp_con);

        // Try to init module list
        let mut ret = BaseContextAsync::new(conn);
        let _ = ret.get_module_list();
        Ok(ret)
    }
}
