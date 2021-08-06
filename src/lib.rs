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

/// 10 msec
const FLAKE_TIME_UNIT: i64 = 10_000_000;

/// Convenience type alias for usage within sonyflake.
pub(crate) type BoxDynError = Box<dyn std::error::Error + 'static + Send + Sync>;

/// The error type for this crate.
#[derive(Debug)]
pub enum Error {
    // #[error("start_time `{0}` is ahead of current time")]
    StartTimeAheadOfCurrentTime(DateTime<Utc>),
    // #[error("machine_id returned an error: {0}")]
    MachineIdFailed(BoxDynError),
    // #[error("check_machine_id returned false")]
    CheckMachineIdFailed,
    // #[error("over the time limit")]
    OverTimeLimit,
    // #[error("could not find any private ipv4 address")]
    NoPrivateIPv4,
    // #[error("mutex is poisoned (i.e. a panic happened while it was locked)")]
    MutexPoisoned,
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl std::error::Error for Error {}

/// A builder to build a [`Sonyflake`] generator.
///
/// [`Sonyflake`]: struct.Sonyflake.html
pub struct Builder<'a> {
    start_time: Option<DateTime<Utc>>,
    machine_id: Option<&'a dyn Fn() -> Result<u16, BoxDynError>>,
    check_machine_id: Option<&'a dyn Fn(u16) -> bool>,
}

impl<'a> Default for Builder<'a> {
    fn default() -> Self {
        Builder::new()
    }
}

impl<'a> Builder<'a> {
    /// Construct a new builder to call methods on for the [`Sonyflake`] construction.
    ///
    /// [`Sonyflake`]: struct.Sonyflake.html
    pub fn new() -> Self {
        Self {
            start_time: None,
            machine_id: None,
            check_machine_id: None,
        }
    }

    /// Sets the start time.
    /// If the time is ahead of current time, finalize will fail.
    pub fn start_time(mut self, start_time: DateTime<Utc>) -> Self {
        self.start_time = Some(start_time);
        self
    }

    /// Sets the machine id.
    /// If the fn returns an error, finalize will fail.
    pub fn machine_id(mut self, machine_id: &'a dyn Fn() -> Result<u16, BoxDynError>) -> Self {
        self.machine_id = Some(machine_id);
        self
    }

    /// Set a function to check the machine id.
    /// If the fn returns false, finalize will fail.
    pub fn check_machine_id(mut self, check_machine_id: &'a dyn Fn(u16) -> bool) -> Self {
        self.check_machine_id = Some(check_machine_id);
        self
    }

    /// Finalize the builder to create a Sonyflake.
    pub fn finalize(self) -> Result<Sonyflake, Error> {
        let sequence = 1 << (BIT_LEN_SEQUENCE - 1);

        let start_time = if let Some(start_time) = self.start_time {
            if start_time > Utc::now() {
                return Err(Error::StartTimeAheadOfCurrentTime(start_time));
            }

            to_sonyflake_time(start_time)
        } else {
            to_sonyflake_time(Utc.ymd(2014, 9, 1).and_hms(0, 0, 0))
        };

        let machine_id = if let Some(machine_id) = self.machine_id {
            match machine_id() {
                Ok(machine_id) => machine_id,
                Err(e) => return Err(Error::MachineIdFailed(e)),
            }
        } else {
            lower_16_bit_private_ip()?
        };

        if let Some(check_machine_id) = self.check_machine_id {
            if !check_machine_id(machine_id) {
                return Err(Error::CheckMachineIdFailed);
            }
        }

        let shared = Arc::new(SharedSonyflake {
            internals: Mutex::new(Internals {
                sequence,
                elapsed_time: 0,
            }),
            start_time,
            machine_id,
        });
        Ok(Sonyflake::new_inner(shared))
    }
}

fn private_ipv4() -> Option<Ipv4Addr> {
    interfaces()
        .iter()
        .filter(|interface| interface.is_up() && !interface.is_loopback())
        .map(|interface| {
            interface
                .ips
                .iter()
                .map(|ip_addr| ip_addr.ip()) // convert to std
                .find(|ip_addr| match ip_addr {
                    IpAddr::V4(ipv4) => is_private_ipv4(*ipv4),
                    IpAddr::V6(_) => false,
                })
                .and_then(|ip_addr| match ip_addr {
                    IpAddr::V4(ipv4) => Some(ipv4), // make sure the return type is Ipv4Addr
                    _ => None,
                })
        })
        .find(|ip| ip.is_some())
        .flatten()
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 10
        || octets[0] == 172 && (octets[1] >= 16 && octets[1] < 32)
        || octets[0] == 192 && octets[1] == 168
}

fn lower_16_bit_private_ip() -> Result<u16, Error> {
    match private_ipv4() {
        Some(ip) => {
            let octets = ip.octets();
            Ok(((octets[2] as u16) << 8) + (octets[3] as u16))
        }
        None => Err(Error::NoPrivateIPv4),
    }
}

#[derive(Debug)]
pub(crate) struct Internals {
    pub(crate) elapsed_time: i64,
    pub(crate) sequence: u16,
}

pub(crate) struct SharedSonyflake {
    pub(crate) start_time: i64,
    pub(crate) machine_id: u16,
    pub(crate) internals: Mutex<Internals>,
}

/// Sonyflake is a distributed unique ID generator.
pub struct Sonyflake(pub(crate) Arc<SharedSonyflake>);

impl Sonyflake {
    /// Create a new Sonyflake with the default configuration.
    /// For custom configuration see [`builder`].
    ///
    /// [`builder`]: struct.Sonyflake.html#method.builder
    pub fn new() -> Result<Self, Error> {
        Builder::new().finalize()
    }

    /// Create a new [`Builder`] to construct a Sonyflake.
    ///
    /// [`Builder`]: struct.Builder.html
    pub fn builder<'a>() -> Builder<'a> {
        Builder::new()
    }

    pub(crate) fn new_inner(shared: Arc<SharedSonyflake>) -> Self {
        Self(shared)
    }

    /// Generate the next unique id.
    /// After the Sonyflake time overflows, next_id returns an error.
    pub fn next_id(&mut self) -> Result<u64, Error> {
        let mask_sequence = (1 << BIT_LEN_SEQUENCE) - 1;

        let mut internals = self.0.internals.lock().map_err(|_| Error::MutexPoisoned)?;

        let current = current_elapsed_time(self.0.start_time);

        if internals.elapsed_time < current {
            internals.elapsed_time = current;
            internals.sequence = 0;
        } else {
            // self.elapsed_time >= current
            internals.sequence = (internals.sequence + 1) & mask_sequence;
            if internals.sequence == 0 {
                internals.elapsed_time += 1;
                let overtime = internals.elapsed_time - current;
                std::thread::sleep(sleep_time(overtime));
            }
        }

        if internals.elapsed_time >= 1 << BIT_LEN_TIME {
            return Err(Error::OverTimeLimit);
        }

        Ok(
            (internals.elapsed_time as u64) << (BIT_LEN_SEQUENCE + BIT_LEN_MACHINE_ID)
                | (internals.sequence as u64) << BIT_LEN_MACHINE_ID
                | (self.0.machine_id as u64),
        )
    }
}

/// Returns a new `Sonyflake` referencing the same state as `self`.
impl Clone for Sonyflake {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub(crate) fn to_sonyflake_time(time: DateTime<Utc>) -> i64 {
    time.timestamp_nanos() / FLAKE_TIME_UNIT
}

fn current_elapsed_time(start_time: i64) -> i64 {
    to_sonyflake_time(Utc::now()) - start_time
}

fn sleep_time(overtime: i64) -> Duration {
    Duration::from_millis(overtime as u64 * 10)
        - Duration::from_nanos((Utc::now().timestamp_nanos() % FLAKE_TIME_UNIT) as u64)
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
    let mask_seq = ((1 << BIT_LEN_SEQUENCE) - 1 as u64) << BIT_LEN_MACHINE_ID;
    let mask_machine_id = (1 << BIT_LEN_MACHINE_ID) - 1 as u64;

    let msb = id >> 63;
    let time = id >> (BIT_LEN_SEQUENCE + BIT_LEN_MACHINE_ID);

    let seq = (id & mask_seq) >> BIT_LEN_MACHINE_ID;
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
    Utc.ymd(2021, 8, 6).and_hms_nano(0,0,0,0)
}

#[cfg(test)]
mod tests {
    use crate::{decompose, lower_16_bit_private_ip, IDParts, BIT_LEN_SEQUENCE, to_sonyflake_time, Sonyflake};
    use std::time::Duration;
    use chrono::Utc;

    #[test]
    fn test_flake_once() {
        let now = Utc::now();
        let mut f = Sonyflake::builder().start_time(now).finalize().unwrap();

        let sleep_time = 500u64;
        std::thread::sleep(Duration::from_millis(sleep_time));
        #[cfg(feature = "default")]
        let id = f.next_id().unwrap();
        #[cfg(not(feature = "default"))]
        let id = f.next_id().unwrap();

        let parts = decompose(id);
        assert_eq!(parts.get_msb(), 0);
        assert_eq!(parts.get_sequence(), 0);
        assert!(parts.get_time() < sleep_time || parts.get_time() > sleep_time + 1);
        assert_eq!(parts.machine_id, lower_16_bit_private_ip().unwrap() as u64);
    }

    #[test]
    fn test_flake_for_10_sec() {
        let now = Utc::now();
        let start_time = to_sonyflake_time(now);
        let mut f = Sonyflake::builder().start_time(now).finalize().unwrap();

        let mut num_id: u64 = 0;
        let mut last_id: u64 = 0;
        let mut max_seq: u64 = 0;

        let machine_id = lower_16_bit_private_ip().unwrap() as u64;

        let initial = to_sonyflake_time(Utc::now());
        let mut current = initial.clone();

        while current - initial < 1000 {
            #[cfg(feature = "default")]
            let id = f.next_id().unwrap();
            #[cfg(not(feature = "default"))]
            let id = f.next_id().unwrap();

            let parts = IDParts::decompose(id);
            num_id += 1;

            assert!(id > last_id);
            last_id = id;

            current = to_sonyflake_time(Utc::now());

            assert_eq!(parts.get_msb(), 0);
            let overtime = start_time + (parts.get_time() as i64) - current;
            assert!(overtime <= 0);

            if max_seq < parts.get_sequence() {
                max_seq = parts.get_sequence();
            }

            assert_eq!(parts.get_machine_id(), machine_id);
        }

        assert_eq!(max_seq, (1 << BIT_LEN_SEQUENCE) - 1);
        println!("number of id: {}", num_id);
    }
}
