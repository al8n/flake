//! This file implements [SonyFlake](https://github.com/sony/sonyflake) distributed id generate algorithm.
//! Sonyflake
//! =========
//!
//! Sonyflake is a distributed unique ID generator inspired by [Twitter's Snowflake](https://blog.twitter.com/2010/announcing-snowflake).
//!
//! Sonyflake focuses on lifetime and performance on many host/core environment.
//! So it has a different bit assignment from Snowflake.
//! A Sonyflake ID is composed of
//!
//! - 39 bits for time in units of 10 msec
//! - 8 bits for a sequence number
//! - 16 bits for a machine id
//!
//! As a result, Sonyflake has the following advantages and disadvantages:
//!
//! - The lifetime (174 years) is longer than that of Snowflake (69 years)
//! - It can work in more distributed machines (2^16) than Snowflake (2^10)
//! - It can generate 2^8 IDs per 10 msec at most in a single machine/thread (slower than Snowflake)
//!
//! However, if you want more generation rate in a single host,
//! you can easily run multiple Sonyflake ID generators concurrently using goroutines.
//!
//!
//! Usage
//! -----
//!
//! The function NewSonyflake creates a new Sonyflake instance.
//!
//!
//! You can configure Sonyflake by the struct Settings:
//!
//!
//! - StartTime is the time since which the Sonyflake time is defined as the elapsed time.
//!   If StartTime is 0, the start time of the Sonyflake is set to "2021-08-05 00:00:00 +0000 UTC".
//!   If StartTime is ahead of the current time, Sonyflake is not created.
//!
//! - MachineID returns the unique ID of the Sonyflake instance.
//!   If MachineID returns an error, Sonyflake is not created.
//!   If MachineID is nil, default MachineID is used.
//!   Default MachineID returns the lower 16 bits of the private IP address.
//!
//! - CheckMachineID validates the uniqueness of the machine ID.
//!   If CheckMachineID returns false, Sonyflake is not created.
//!   If CheckMachineID is nil, no validation is done.
//!
//! In order to get a new unique ID, you just have to call the method NextID.
//!
//!
//! NextID can continue to generate IDs for about 174 years from StartTime.
//! But after the Sonyflake time is over the limit, NextID returns an error.
//!
//! AWS VPC and Docker
//! ------------------
//!
//! The [awsutil](https://github.com/sony/sonyflake/blob/master/awsutil) package provides
//! the function AmazonEC2MachineID that returns the lower 16-bit private IP address of the Amazon EC2 instance.
//! It also works correctly on Docker
//! by retrieving [instance metadata](http://docs.aws.amazon.com/en_us/AWSEC2/latest/UserGuide/ec2-instance-metadata.html).
//!
//! [AWS VPC](http://docs.aws.amazon.com/en_us/AmazonVPC/latest/UserGuide/VPC_Subnets.html)
//! is assigned a single CIDR with a netmask between /28 and /16.
//! So if each EC2 instance has a unique private IP address in AWS VPC,
//! the lower 16 bits of the address is also unique.
//! In this common case, you can use AmazonEC2MachineID as Settings.MachineID.
//!

#[macro_use]
extern crate serde;

use chrono::{Utc, DateTime, TimeZone};
#[cfg(feature = "default")]
use std::sync::{Mutex};
#[cfg(not(feature = "default"))]
use parking_lot::Mutex;
use std::sync::{Arc};
use std::fmt::{Formatter, Debug};
use std::net::{Ipv4Addr, IpAddr};
use std::time::Duration;
use pnet::datalink::{interfaces};

/// bit length of time
const BIT_LEN_TIME: i64 = 39;

/// bit length of sequence number
const BIT_LEN_SEQUENCE: i64 = 8;

/// bit length of machine id
const BIT_LEN_MACHINE_ID: i64 = 63 - BIT_LEN_TIME - BIT_LEN_SEQUENCE;

const FLAKE_TIME_UNIT: i64 = 1e7 as i64;

/// FlakeError
#[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum FlakeError {
    /// `FlakeError::InvalidStartTime` means that start time larger than current time
    InvalidStartTime,

    /// `FlakeError::NoPrivateIPAddress` means that there is no private ip address on this machine
    NoPrivateIPAddress,

    /// `FlakeError::FlakeTimeOverflow` means that we over the time limit
    FlakeTimeOverflow,

    /// `FlakeError::InvalidMachineID` returned by `check_machine_id`
    InvalidMachineID,
}

unsafe impl Send for FlakeError {}
unsafe impl Sync for FlakeError {}

impl std::fmt::Display for FlakeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FlakeError::InvalidStartTime => write!(f, "start time larger than current time"),
            FlakeError::NoPrivateIPAddress => write!(f, "no private ip address"),
            FlakeError::FlakeTimeOverflow => write!(f, "over the flake time limit"),
            FlakeError::InvalidMachineID => write!(f, "invalid machine id")
        }
    }
}

impl std::error::Error for FlakeError {}

/// `MachineIDGenerator` is for custom machine id generator.
pub trait MachineIDGenerator {
    /// `machine_id` returns the unique ID of the `Sonyflake` instance.
    /// If `machine_id` returns an error, `Sonyflake` is not created.
    /// If `machine_id` is nil, default `machine_id` is used.
    /// Default `machine_id` returns the lower 16 bits of the private IP address.
    fn machine_id(&self) -> Result<u16, FlakeError>;
}

/// `MachineIDChecker` is for custom machine id checker.
pub trait MachineIDChecker {
    /// `check_machine_id` validates the uniqueness of the machine ID.
    /// If check_machine_id returns false, `Sonyflake` is not created.
    /// If check_machine_id is nil, no validation is done.
    fn check_machine_id(&self, id: u16) -> bool;
}

/// [`Settings`](crate::Settings) is the Flake settings.
pub struct Settings {
    /// `start_time` is the time since which the `Sonyflake` time is defined as the elapsed time.
    /// If StartTime is 0, the start time of the `Sonyflake` is set to "2021-08-05 00:00:00 +0000 UTC".
    /// If StartTime is ahead of the current time, `Sonyflake` is not created.
    start_time: DateTime<Utc>,

    generator: Option<Box<dyn MachineIDGenerator>>,

    checker: Option<Box<dyn MachineIDChecker>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_time: default_start_time(),
            generator: None,
            checker: None,
        }
    }
}

impl Settings {
    pub fn new(start_time: DateTime<Utc>, generator: Box<dyn MachineIDGenerator>, checker: Box<dyn MachineIDChecker>) -> Self {
        Self {
            start_time,
            generator: Some(generator),
            checker: Some(checker),
        }
    }

    pub fn set_start_time(&mut self, start_time: DateTime<Utc>) {
        self.start_time = start_time;
    }

    pub fn set_generator(&mut self, generator: Box<dyn MachineIDGenerator>) {
        self.generator = Some(generator);
    }

    pub fn set_checker(&mut self, checker: Box<dyn MachineIDChecker>) {
        self.checker = Some(checker);
    }

    /// `into_flake` returns `Flake`
    pub fn into_flake(self) -> Result<Arc<Mutex<Flake>>, FlakeError> {
        Flake::new(self)
    }

    /// `into_infallible_flake` returns `InfallibleFlake`
    pub fn into_infallible_flake(self) -> Result<Arc<Mutex<InfallibleFlake>>, FlakeError> {
        InfallibleFlake::new(self)
    }
}

/// `Flake` is a distributed unique ID generator.
#[derive(Copy, Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Flake(Inner);


impl Flake {
    /// `default` returns a `Flake` with default settings.
    pub fn default() -> Result<Arc<Mutex<Self>>, FlakeError> {
        match default_flake() {
            Ok(inner) => Ok(Arc::new(Mutex::new(Self(inner)))),
            Err(e) => Err(e),
        }
    }

    /// `new` returns a `Flake` according to the provided `Settings`.
    pub fn new(st: Settings) -> Result<Arc<Mutex<Self>>, FlakeError> {
        match new_flake(st) {
            Ok(inner) => Ok(Arc::new(Mutex::new(Self(inner)))),
            Err(e) => Err(e)
        }
    }

    /// `next_id` generates a next unique ID.
    /// After the `Flake` time overflows, `next_id` returns an error.
    pub fn next_id(&mut self) -> Result<u64, FlakeError> {
        let mask_seq = (1 << BIT_LEN_SEQUENCE) - 1 as u16;
        let current = current_elapsed_time(self.0.start_time);

        if self.0.elapsed_time < current {
            self.0.elapsed_time = current;
            self.0.sequence = 0;
        } else {
            self.0.sequence = (self.0.sequence + 1) & mask_seq;
            if self.0.sequence == 0 {
                self.0.elapsed_time += 1;
                let overtime = self.0.elapsed_time - current;
                std::thread::sleep(sleep_time(overtime));
            }
        }

        to_id(self.0.elapsed_time, self.0.sequence, self.0.machine_id)
    }
}

/// `InfallibleFlake` is a distributed unique ID generator,
/// the `next_id` method will never failed to generate a unique ID
/// by refresh the inner `start_time`.
#[derive(Copy, Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct InfallibleFlake(Inner);

impl InfallibleFlake {
    /// `default` returns a `InfallibleFlake` with default settings
    pub fn default() -> Result<Arc<Mutex<Self>>, FlakeError> {
        match default_flake() {
            Ok(inner) => Ok(Arc::new(Mutex::new(Self(inner)))),
            Err(e) => Err(e),
        }
    }

    /// `new` returns a `InfallibleFlake` according to the provided `Settings`.
    pub fn new(st: Settings) -> Result<Arc<Mutex<Self>>, FlakeError> {
        match new_flake(st) {
            Ok(inner) => Ok(Arc::new(Mutex::new(Self(inner)))),
            Err(e) => Err(e)
        }
    }

    /// `next_id` generates a next unique ID.
    /// After the `Flake` time overflows, `next_id` will refresh
    /// the `start_time` field of `InfallibleFlake` to the current time to avoid time overflows.
    pub fn next_id(&mut self) -> u64 {
        let mask_seq = (1 << BIT_LEN_SEQUENCE) - 1 as u16;
        let current = current_elapsed_time(self.0.start_time);

        if self.0.elapsed_time < current {
            self.0.elapsed_time = current;
            self.0.sequence = 0;
        } else {
            self.0.sequence = (self.0.sequence + 1) & mask_seq;
            if self.0.sequence == 0 {
                self.0.elapsed_time += 1;
                let overtime = self.0.elapsed_time - current;
                std::thread::sleep(sleep_time(overtime));
            }
        }

        match to_id(self.0.elapsed_time, self.0.sequence, self.0.machine_id) {
            Ok(v) => v,
            Err(_) => {
                self.0.start_time = to_flake_time(Utc::now());
                self.next_id()
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Inner {
    machine_id: u16,
    start_time: i64,
    elapsed_time: i64,
    sequence: u16,
}

fn default_flake() -> Result<Inner, FlakeError> {
    let seq = (1 << BIT_LEN_SEQUENCE) - 1 as u16;
    let start_time_utc = default_start_time();
    let start_time = to_flake_time(start_time_utc);
    let machine_id = lower_16_bit_private_ip()?;

    let instance = Inner {
        machine_id,
        start_time,
        elapsed_time: 0,
        sequence: seq,
    };

    Ok(instance)
}

fn new_flake(st: Settings) -> Result<Inner, FlakeError> {
    let seq = (1 << BIT_LEN_SEQUENCE) - 1 as u16;
    let start_time;

    if st.start_time > Utc::now() {
        return Err(FlakeError::InvalidStartTime);
    }

    if st.start_time.timestamp() == 0 {
        start_time = to_flake_time(default_start_time());
    } else {
        start_time = to_flake_time(st.start_time);
    }

    let machine_id;
    match st.generator {
        None => {
            machine_id = lower_16_bit_private_ip()?;
        }
        Some(g) => {
            machine_id = g.machine_id()?;
        }
    }

    if let Some(c) = st.checker {
        if !c.check_machine_id(machine_id) {
            return Err(FlakeError::InvalidMachineID);
        }
    }

    Ok(Inner {
        machine_id,
        start_time,
        elapsed_time: 0,
        sequence: seq,
    })
}

fn to_id(elapsed_time: i64, seq: u16, machine_id: u16) -> Result<u64, FlakeError> {
    if elapsed_time >= (1 << BIT_LEN_TIME) {
        return Err(FlakeError::FlakeTimeOverflow);
    }

    let id = (elapsed_time as u64) << (BIT_LEN_SEQUENCE + BIT_LEN_MACHINE_ID) | (seq as u64) << BIT_LEN_MACHINE_ID | (machine_id as u64);
    Ok(id)
}

fn to_flake_time(datetime: DateTime<Utc>) -> i64 {
    datetime.timestamp_nanos() / FLAKE_TIME_UNIT
}

fn current_elapsed_time(start_time: i64) -> i64 {
    to_flake_time(Utc::now()) - start_time
}

fn sleep_time(overtime: i64) -> Duration {
    Duration::from_millis((overtime * 10) as u64) - Duration::from_nanos((Utc::now().timestamp_nanos() % FLAKE_TIME_UNIT) as u64)
}

fn private_ipv4() -> Result<Ipv4Addr, FlakeError> {
    let network_interfaces = interfaces();
    for interface in network_interfaces {
        if interface.is_loopback() {
            continue;
        }

        for ip in interface.ips {
            if let IpAddr::V4(v4) = ip.ip() {
                if is_private_ipv4(v4) {
                    return Ok(v4);
                }
            }
        }
    }

    Err(FlakeError::NoPrivateIPAddress)
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets().to_vec();
    octets[0] == 10 || octets[0] == 172 && (octets[1] >= 16 && octets[1] < 32) || octets[0] == 192 && octets[1] == 168
}

fn lower_16_bit_private_ip() -> Result<u16, FlakeError> {
    let p_ipv4 = private_ipv4();
    match p_ipv4 {
        Ok(addr) => {
            let octets_vec = addr.octets().to_vec();
            Ok(((octets_vec[2] as u16) << 8) + (octets_vec[3] as u16))
        }
        Err(e) => Err(e),
    }
}

/// `IDParts` contains the bit parts for an ID.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct IDParts {
    id: u64,
    msb: u64,
    time: u64,
    sequence: u64,
    machine_id: u64,
}

impl IDParts {
    /// `decompose` returns a set of Sonyflake ID parts.
    pub fn decompose(id: u64) -> Self {
        decompose(id)
    }

    /// `get_id` returns the original ID
    pub fn get_id(&self) -> u64 {
        self.id
    }

    /// `get_msb` returns msb for the id
    pub fn get_msb(&self) -> u64 {
        self.msb
    }

    /// `get_time` returns a timestamp
    pub fn get_time(&self) -> u64 {
        self.time
    }

    /// `get_sequence` returns sequence
    pub fn get_sequence(&self) -> u64 {
        self.sequence
    }

    /// `get_machine_id` returns the machine id
    pub fn get_machine_id(&self) -> u64 {
        self.machine_id
    }
}

/// `decompose` returns a set of Sonyflake ID parts.
pub fn decompose(id: u64) -> IDParts {
    let first_part = (1 << BIT_LEN_SEQUENCE - 1) as u64;
    let mask_seq = first_part << BIT_LEN_MACHINE_ID;
    let mask_machine_id = (1 << BIT_LEN_MACHINE_ID - 1) as u64;

    let msb = id >> 63;
    let time = id >> (BIT_LEN_SEQUENCE + BIT_LEN_MACHINE_ID);
    let seq = id & mask_seq >> BIT_LEN_MACHINE_ID;
    let machine_id = id & mask_machine_id;

    IDParts {
        id,
        msb,
        time,
        sequence: seq,
        machine_id
    }
}

fn default_start_time() -> DateTime<Utc> {
    Utc.ymd(2021, 8, 4).and_hms_nano(0,0,0,0)
}

#[cfg(test)]
mod tests {
    use crate::{Settings, decompose};
    use std::time::Duration;
    use chrono::Utc;

    #[test]

    fn test_flake() {
        let mut st = Settings::default();
        st.set_start_time(Utc::now());
        let f = st.into_flake().unwrap();

        let sleep_time = 500u64;
        std::thread::sleep(Duration::from_millis(500));
        #[cfg(feature = "default")]
        let id = f.lock().unwrap().next_id().unwrap();
        #[cfg(not(feature = "default"))]
        let id = f.lock().next_id().unwrap();
        println!("{}", id);

        let parts = decompose(id);
        println!("{:?}", parts);
        assert_ne!(parts.get_msb(), 0);
        assert_ne!(parts.get_sequence(), 0);
        assert!(parts.get_time() < sleep_time || parts.get_time() > sleep_time + 1);
    }
}
