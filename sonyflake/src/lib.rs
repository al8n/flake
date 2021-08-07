/* Copyright 2021 Al Liu (https://github.com/al8n). Licensed under Apache-2.0.
 *
 * Copyright 2020 Arne Bahlo (https://github.com/bahlo/sonyflake-rs). Licensed under Apache-2.0.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
//! This file implements [SonyFlake](https://github.com/sony/sonyflake) distributed id generate algorithm and some extensions.
//! SonyFlake
//! =========
//!
//! SonyFlake is a distributed unique ID generator inspired by [Twitter's Snowflake](https://blog.twitter.com/2010/announcing-snowflake).
//!
//! SonyFlake focuses on lifetime and performance on many host/core environment.
//! So it has a different bit assignment from Snowflake.
//! A SonyFlake ID is composed of
//!
//! - 39 bits for time in units of 10 msec
//! - 8 bits for a sequence number
//! - 16 bits for a machine id
//!
//! As a result, SonyFlake has the following advantages and disadvantages:
//!
//! - The lifetime (174 years) is longer than that of Snowflake (69 years)
//! - It can work in more distributed machines (2^16) than Snowflake (2^10)
//! - It can generate 2^8 IDs per 10 msec at most in a single machine/thread (slower than Snowflake)
//!
//! However, if you want more generation rate in a single host,
//! you can easily run multiple SonyFlake ID generators concurrently using goroutines.
//!
//!
//! Usage
//! -----
//!
//! The function NewSonyFlake creates a new SonyFlake instance.
//!
//!
//! You can configure SonyFlake by the struct Settings:
//!
//! ## Install
//!
//! Add the following to your `Cargo.toml`:
//! ```toml
//! [dependencies]
//! infallible-sonyflake = "0.1"
//! ```
//!
//! ## Quickstart
//! 1. **Fallible SonyFlake**
//!    `Sonyflake` may fail to generate a unique ID when we call `next_id` if time overflows.
//!    ```rust
//!    use infallible_sonyflake::{SonyFlake, Settings};
//!    use chrono::Utc;
//!
//!    fn main() {
//!        let now = Utc::now();
//!        let mut sf = Settings::new().set_start_time(now).into_sonyflake().unwrap();
//!        let next_id = sf.next_id().unwrap();
//!        println!("{}", next_id);
//!    }
//!    ```
//! 2. **Infallible SonyFlake**
//!    `InfallibleSonyFlake` will always generate a unique ID when we call `next_id` if time overflow happens, it will refresh the `start_time` to the current time.
//!    ```rust
//!    use infallible_sonyflake::{InfallibleSonyFlake, Settings};
//!    use chrono::Utc;
//!
//!    fn main() {
//!        let now = Utc::now();
//!        let mut sf = Settings::new().set_start_time(now).into_infallible_sonyflake().unwrap();
//!        let next_id = sf.next_id();
//!        println!("{}", next_id);
//!    }
//!    ```
//! 3. **Custom machine ID and machine ID checker**
//!    ```rust
//!    use infallible_sonyflake::{InfallibleSonyFlake, Settings, MachineID, MachineIDChecker, IDParts, Error};
//!    use chrono::Utc;
//!
//!    struct CustomMachineID {
//!        counter: u64,
//!        id: u16,
//!    }
//!
//!    impl MachineID for CustomMachineID {
//!        fn machine_id(&mut self) -> Result<u16, Box<dyn std::error::Error + Send + Sync + 'static>> {
//!            self.counter += 1;
//!            if self.counter % 2 != 0 {
//!                Ok(self.id)
//!            } else {
//!                Err(Box::new("NaN".parse::<u32>().unwrap_err()))
//!            }
//!        }
//!    }
//!
//!    struct CustomMachineIDChecker;
//!
//!    impl MachineIDChecker for CustomMachineIDChecker {
//!        fn check_machine_id(&self, id: u16) -> bool {
//!            if id % 2 != 0 {
//!                true
//!            } else {
//!                false
//!            }
//!        }
//!    }
//!
//!    fn main() {
//!        let mut sf = Settings::new()
//!            .set_machine_id(Box::new(CustomMachineID { counter: 0, id: 1 }))
//!            .set_check_machine_id(Box::new(CustomMachineIDChecker {}))
//!            .into_infallible_sonyflake().unwrap();
//!        let id = sf.next_id();
//!        let parts = IDParts::decompose(id);
//!        assert_eq!(parts.get_machine_id(), 1);
//!
//!        let err = Settings::new()
//!            .set_machine_id(Box::new(CustomMachineID { counter: 0, id: 2 }))
//!            .set_check_machine_id(Box::new(CustomMachineIDChecker {}))
//!            .into_infallible_sonyflake().unwrap_err();
//!
//!        assert_eq!(format!("{}", err), Error::InvalidMachineID(2).to_string());
//!    }
//!    ```
//!
//!
//! - StartTime is the time since which the SonyFlake time is defined as the elapsed time.
//!   If StartTime is 0, the start time of the SonyFlake is set to "2021-08-06 00:00:00 +0000 UTC".
//!   If StartTime is ahead of the current time, SonyFlake is not created.
//!
//! - MachineID returns the unique ID of the SonyFlake instance.
//!   If MachineID returns an error, SonyFlake is not created.
//!   If MachineID is nil, default MachineID is used.
//!   Default MachineID returns the lower 16 bits of the private IP address.
//!
//! - CheckMachineID validates the uniqueness of the machine ID.
//!   If CheckMachineID returns false, SonyFlake is not created.
//!   If CheckMachineID is nil, no validation is done.
//!
//! In order to get a new unique ID, you just have to call the method NextID.
//!
//!
//! NextID can continue to generate IDs for about 174 years from StartTime.
//! But after the SonyFlake time is over the limit, NextID returns an error. Or, you can use `InfallibleSonyFlake`, `InfallibleSonyFlake` will always generate a unique ID when we call `next_id` if time overflow happens, it will refresh the `start_time` to the current time.

#[macro_use]
extern crate serde;

use chrono::{DateTime, TimeZone, Utc};
use pnet::datalink::interfaces;
use std::fmt::{Debug, Formatter};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;

/// bit length of time
const BIT_LEN_TIME: i64 = 39;

/// bit length of sequence number
const BIT_LEN_SEQUENCE: i64 = 8;

/// bit length of machine id
const BIT_LEN_MACHINE_ID: i64 = 63 - BIT_LEN_TIME - BIT_LEN_SEQUENCE;

/// 10 msec
const FLAKE_TIME_UNIT: i64 = 10_000_000;

/// The [`Error`] type for this crate.
///
/// [`Error`]: enum.Error.html
#[derive(Debug)]
pub enum Error {
    /// `Error::StartTimeAheadOfCurrentTime` means that start time is ahead of current time
    StartTimeAheadOfCurrentTime(DateTime<Utc>),

    /// `Error::MachineIdFailed` returned by `MachineID`
    MachineIdFailed(Box<dyn std::error::Error + 'static + Send + Sync>),

    /// `Error::InvalidMachineID` returned by `MachineIDChecker`
    InvalidMachineID(u16),

    /// `Error::TimeOverflow` means that we over the sonyflake time limit
    TimeOverflow,

    /// `Error::NoPrivateIPv4Address` means that there is no private ip address on this machine
    NoPrivateIPv4Address,
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::StartTimeAheadOfCurrentTime(time) => {
                write!(f, "start_time {} is ahead of current time", time)
            }
            Error::MachineIdFailed(e) => write!(f, "cannot get a machine id: {}", e),
            Error::InvalidMachineID(id) => write!(f, "invalid machine id: {}", id),
            Error::TimeOverflow => write!(f, "over the sonyflake time limit"),
            Error::NoPrivateIPv4Address => write!(f, "no private IPv4 address"),
        }
    }
}

impl std::error::Error for Error {}

/// `MachineID` is for custom machine id generator.
pub trait MachineID {
    /// `machine_id` returns the unique ID of the `Sonyflake` instance.
    /// If `machine_id` returns an error, `Sonyflake` is not created.
    /// If `machine_id` is nil, default `machine_id` is used.
    /// Default `machine_id` returns the lower 16 bits of the private IP address.
    fn machine_id(&mut self) -> Result<u16, Box<dyn std::error::Error + Send + Sync + 'static>>;
}

/// `MachineIDChecker` is for custom machine id checker.
pub trait MachineIDChecker {
    /// `check_machine_id` validates the uniqueness of the machine ID.
    /// If check_machine_id returns false, `Sonyflake` is not created.
    /// If check_machine_id is nil, no validation is done.
    fn check_machine_id(&self, id: u16) -> bool;
}

/// A builder to build a [`SonyFlake`] generator.
///
/// [`SonyFlake`]: struct.SonyFlake.html
pub struct Settings {
    start_time: Option<DateTime<Utc>>,
    machine_id: Option<Box<dyn MachineID>>,
    check_machine_id: Option<Box<dyn MachineIDChecker>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings::new()
    }
}

impl Settings {
    /// Construct a new builder to call methods on for the [`SonyFlake`] or [`InfallibleSonyFlake`] construction.
    ///
    /// [`SonyFlake`]: struct.SonyFlake.html
    /// [`InfallibleSonyFlake`]: struct.InfallibleSonyFlake.html
    pub fn new() -> Self {
        Self {
            start_time: None,
            machine_id: None,
            check_machine_id: None,
        }
    }

    fn get_start_time(&self) -> Result<i64, Error> {
        return if let Some(start_time) = self.start_time {
            if start_time > Utc::now() {
                return Err(Error::StartTimeAheadOfCurrentTime(start_time));
            }
            Ok(to_sonyflake_time(start_time))
        } else {
            Ok(to_sonyflake_time(default_start_time()))
        }
    }

    fn get_and_check_machine_id(self) -> Result<u16, Error> {
        return if let Some(mut machine_id) = self.machine_id {
            match machine_id.machine_id() {
                Ok(machine_id) => {
                    if let Some(checker) = self.check_machine_id {
                        if !checker.check_machine_id(machine_id) {
                            return Err(Error::InvalidMachineID(machine_id));
                        }
                    }
                    Ok(machine_id)
                },
                Err(e) => Err(Error::MachineIdFailed(e)),
            }
        } else {
            match lower_16_bit_private_ip() {
                Ok(machine_id) => {
                    if let Some(checker) = self.check_machine_id {
                        if !checker.check_machine_id(machine_id) {
                            return Err(Error::InvalidMachineID(machine_id));
                        }
                    }
                    Ok(machine_id)
                },
                Err(e) => Err(e),
            }
        };
    }

    /// Sets the start time.
    /// If the time is ahead of current time, finalize will fail.
    pub fn set_start_time(mut self, start_time: DateTime<Utc>) -> Self {
        self.start_time = Some(start_time);
        self
    }

    /// Sets the machine id.
    /// If the fn returns an error, finalize will fail.
    pub fn set_machine_id(mut self, machine_id: Box<dyn MachineID>) -> Self {
        self.machine_id = Some(machine_id);
        self
    }

    /// Set a function to check the machine id.
    /// If the fn returns false, finalize will fail.
    pub fn set_check_machine_id(mut self, check_machine_id: Box<dyn MachineIDChecker>) -> Self {
        self.check_machine_id = Some(check_machine_id);
        self
    }

    pub fn into_sonyflake(self) -> Result<SonyFlake, Error> {
        SonyFlake::new(self)
    }

    pub fn into_infallible_sonyflake(self) -> Result<InfallibleSonyFlake, Error> {
        InfallibleSonyFlake::new(self)
    }
}

/// SonyFlake is a distributed unique ID generator, may fail to generate unique id if time overflows.
#[derive(Debug)]
pub struct SonyFlake {
    start_time: i64,
    machine_id: u16,
    inner: Arc<Mutex<Inner>>,
}

impl SonyFlake {
    /// Create a new SonyFlake with the default configuration.
    /// For custom configuration see [`builder`].
    ///
    /// [`builder`]: struct.SonyFlake.html#method.builder
    pub fn new(st: Settings) -> Result<Self, Error> {
        let sequence = 1 << (BIT_LEN_SEQUENCE - 1);

        let start_time = st.get_start_time()?;

        let machine_id = st.get_and_check_machine_id()?;

        Ok(SonyFlake {
            start_time,
            machine_id,
            inner: Arc::new(Mutex::new(Inner {
                sequence,
                elapsed_time: 0,
            })),
        })
    }

    /// Generate the next unique id.
    /// After the SonyFlake time overflows, next_id returns an error.
    pub fn next_id(&mut self) -> Result<u64, Error> {
        let mask_sequence = (1 << BIT_LEN_SEQUENCE) - 1;
        
        let mut inner = self.inner.lock();

        let current = current_elapsed_time(self.start_time);

        if inner.elapsed_time < current {
            inner.elapsed_time = current;
            inner.sequence = 0;
        } else {
            // self.elapsed_time >= current
            inner.sequence = (inner.sequence + 1) & mask_sequence;
            if inner.sequence == 0 {
                inner.elapsed_time += 1;
                let overtime = inner.elapsed_time - current;
                std::thread::sleep(sleep_time(overtime));
            }
        }

        if inner.elapsed_time >= 1 << BIT_LEN_TIME {
            return Err(Error::TimeOverflow);
        }

        Ok(to_id(inner.elapsed_time, inner.sequence, self.machine_id))
    }
}

/// Returns a new `SonyFlake` referencing the same state as `self`.
impl Clone for SonyFlake {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            machine_id: self.machine_id,
            inner: self.inner.clone(),
        }
    }
}

/// InfallibleSonyFlake is a distributed unique ID generator, which will always generate a unique id.
/// If time overflows, it will refresh the start time to current time.
#[derive(Debug)]
pub struct InfallibleSonyFlake {
    start_time: i64,
    machine_id: u16,
    inner: Arc<Mutex<Inner>>,
}

impl InfallibleSonyFlake {
    /// Create a new SonyFlake with the default configuration.
    /// For custom configuration see [`builder`].
    ///
    /// [`builder`]: struct.SonyFlake.html#method.builder
    pub fn new(st: Settings) -> Result<Self, Error> {
        let sequence = 1 << (BIT_LEN_SEQUENCE - 1);

        let start_time = st.get_start_time()?;

        let machine_id = st.get_and_check_machine_id()?;

        Ok(Self {
            start_time,
            machine_id,
            inner: Arc::new(Mutex::new(Inner {
                sequence,
                elapsed_time: 0,
            })),
        })
    }

    /// Generate the next unique id.
    /// After the SonyFlake time overflows, next_id returns an error.
    pub fn next_id(&mut self) -> u64 {
        let mask_sequence = (1 << BIT_LEN_SEQUENCE) - 1;

        let mut inner = self.inner.lock();

        let current = current_elapsed_time(self.start_time);

        if inner.elapsed_time < current {
            inner.elapsed_time = current;
            inner.sequence = 0;
        } else {
            // self.elapsed_time >= current
            inner.sequence = (inner.sequence + 1) & mask_sequence;
            if inner.sequence == 0 {
                inner.elapsed_time += 1;
                let overtime = inner.elapsed_time - current;
                std::thread::sleep(sleep_time(overtime));
            }
        }

        if inner.elapsed_time >= 1 << BIT_LEN_TIME {
            let now = Utc::now();
            // let today = Utc::today().and_hms(now.hour(), now.minute(), now.second());
            self.start_time = to_sonyflake_time(now, );
            inner.elapsed_time = 0;
            inner.sequence = 0;
            return to_id(inner.elapsed_time, inner.sequence, self.machine_id);
        }

        to_id(inner.elapsed_time, inner.sequence, self.machine_id)
    }
}

/// Returns a new `InfallibleSonyFlake` referencing the same state as `self`.
impl Clone for InfallibleSonyFlake {
    fn clone(&self) -> Self {
        Self {
            start_time: self.start_time,
            machine_id: self.machine_id,
            inner: self.inner.clone(),
        }
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
        None => Err(Error::NoPrivateIPv4Address),
    }
}

#[derive(Debug)]
struct Inner {
    elapsed_time: i64,
    sequence: u16,
}

fn to_id(elapsed_time: i64, seq: u16, machine_id: u16) -> u64 {
    (elapsed_time as u64) << (BIT_LEN_SEQUENCE + BIT_LEN_MACHINE_ID)
        | (seq as u64) << BIT_LEN_MACHINE_ID
        | (machine_id as u64)
}

fn to_sonyflake_time(time: DateTime<Utc>) -> i64 {
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
    /// `decompose` returns a set of SonyFlake ID parts.
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

/// `decompose` returns a set of SonyFlake ID parts.
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
        machine_id,
    }
}

fn default_start_time() -> DateTime<Utc> {
    Utc.ymd(2021, 8, 6).and_hms_nano(0, 0, 0, 0)
}

#[cfg(test)]
mod tests {
    use crate::{Error as FlakeError, lower_16_bit_private_ip, to_sonyflake_time, IDParts, Settings, SonyFlake, InfallibleSonyFlake, BIT_LEN_SEQUENCE, MachineID, MachineIDChecker, BIT_LEN_TIME};
    use chrono::Utc;
    use std::time::Duration;
    use std::error::Error;
    use std::thread::JoinHandle;
    use std::collections::HashSet;

    #[test]
    fn test_sonyflake_once() {
        let now = Utc::now();
        let mut f = Settings::new().set_start_time(now).into_sonyflake().unwrap();

        let sleep_time = 500u64;
        std::thread::sleep(Duration::from_millis(sleep_time));
        let id = f.next_id().unwrap();

        let parts = IDParts::decompose(id);
        assert_eq!(parts.get_msb(), 0);
        assert_eq!(parts.get_sequence(), 0);
        assert!(parts.get_time() < sleep_time || parts.get_time() > sleep_time + 1);
        assert_eq!(parts.machine_id, lower_16_bit_private_ip().unwrap() as u64);
    }

    #[test]
    fn test_infallible_sonyflake_once() {
        let now = Utc::now();
        let mut f = Settings::new().set_start_time(now).into_infallible_sonyflake().unwrap();

        let sleep_time = 500u64;
        std::thread::sleep(Duration::from_millis(sleep_time));
        let id = f.next_id();

        let parts = IDParts::decompose(id);
        assert_eq!(parts.get_msb(), 0);
        assert_eq!(parts.get_sequence(), 0);
        assert!(parts.get_time() < sleep_time || parts.get_time() > sleep_time + 1);
        assert_eq!(parts.machine_id, lower_16_bit_private_ip().unwrap() as u64);
    }

    #[test]
    fn test_sonyflake_for_10_sec() {
        let now = Utc::now();
        let start_time = to_sonyflake_time(now);
        let mut f = SonyFlake::new(Settings::new().set_start_time(now)).unwrap();

        let mut num_id: u64 = 0;
        let mut last_id: u64 = 0;
        let mut max_seq: u64 = 0;

        let machine_id = lower_16_bit_private_ip().unwrap() as u64;

        let initial = to_sonyflake_time(Utc::now());
        let mut current = initial.clone();

        while current - initial < 1000 {
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

    #[test]
    fn test_infallible_sonyflake_for_10_sec() {
        let now = Utc::now();
        let start_time = to_sonyflake_time(now);
        let mut f = InfallibleSonyFlake::new(Settings::new().set_start_time(now)).unwrap();

        let mut num_id: u64 = 0;
        let mut last_id: u64 = 0;
        let mut max_seq: u64 = 0;

        let machine_id = lower_16_bit_private_ip().unwrap() as u64;

        let initial = to_sonyflake_time(Utc::now());
        let mut current = initial.clone();

        while current - initial < 1000 {
            let id = f.next_id();

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

    struct CustomMachineID {
        counter: u64,
        id: u16,
    }

    impl MachineID for CustomMachineID {
        fn machine_id(&mut self) -> Result<u16, Box<dyn Error + Send + Sync + 'static>> {
            self.counter += 1;
            if self.counter % 2 != 0 {
                Ok(self.id)
            } else {
                Err(Box::new("NaN".parse::<u32>().unwrap_err()))
            }
        }
    }

    struct CustomMachineIDChecker;

    impl MachineIDChecker for CustomMachineIDChecker {
        fn check_machine_id(&self, id: u16) -> bool {
            if id % 2 != 0 {
                true
            } else {
                false
            }
        }
    }

    #[test]
    fn test_sonyflake_custom_machine_id_and_checker() {
        let mut sf = Settings::new()
            .set_machine_id(Box::new(CustomMachineID { counter: 0, id: 1 }))
            .set_check_machine_id(Box::new(CustomMachineIDChecker {}))
            .into_sonyflake().unwrap();
        let id = sf.next_id().unwrap();
        let parts = IDParts::decompose(id);
        assert_eq!(parts.get_machine_id(), 1);

        let err = Settings::new()
            .set_machine_id(Box::new(CustomMachineID { counter: 0, id: 2 }))
            .set_check_machine_id(Box::new(CustomMachineIDChecker {}))
            .into_sonyflake().unwrap_err();

        assert_eq!(format!("{}", err), FlakeError::InvalidMachineID(2).to_string());
    }

    #[test]
    fn test_infallible_sonyflake_custom_machine_id_and_checker() {
        let mut sf = Settings::new()
            .set_machine_id(Box::new(CustomMachineID { counter: 0, id: 1 }))
            .set_check_machine_id(Box::new(CustomMachineIDChecker {}))
            .into_infallible_sonyflake().unwrap();
        let id = sf.next_id();
        let parts = IDParts::decompose(id);
        assert_eq!(parts.get_machine_id(), 1);

        let err = Settings::new()
            .set_machine_id(Box::new(CustomMachineID { counter: 0, id: 2 }))
            .set_check_machine_id(Box::new(CustomMachineIDChecker {}))
            .into_infallible_sonyflake().unwrap_err();

        assert_eq!(format!("{}", err), FlakeError::InvalidMachineID(2).to_string());
    }

    #[test]
    #[should_panic]
    fn test_fallible() {
        let now = Utc::now();
        let mut sf = Settings::new().set_start_time(now).into_sonyflake().unwrap();
        sf.inner.lock().elapsed_time = 1 << BIT_LEN_TIME;
        let _ = sf.next_id().unwrap();
    }

    #[test]
    fn test_infallible() {
        let now = Utc::now();
        let mut sf = Settings::new().set_start_time(now).into_infallible_sonyflake().unwrap();
        sf.inner.lock().elapsed_time = (1 << BIT_LEN_TIME) - 2;
        let _ = sf.next_id();
        let _ = sf.next_id();
        let _ = sf.next_id();
        let _ = sf.next_id();
    }

    #[test]
    fn test_sonyflake_concurrency() {
        let now = Utc::now();
        let sf = Settings::new().set_start_time(now).into_sonyflake().unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<u64>();

        let mut threads = Vec::<JoinHandle<()>>::with_capacity(1000);
        for _ in 0..100 {
            let mut thread_sf = sf.clone();
            let thread_tx = tx.clone();
            threads.push(std::thread::spawn(move || {
                for _ in 0..1000 {
                    thread_tx.send(thread_sf.next_id().unwrap()).unwrap();
                }
            }));
        }

        let mut ids = HashSet::new();
        for _ in 0..100000 {
            let id = rx.recv().unwrap();
            assert!(!ids.contains(&id), "duplicate id: {}", id);
            ids.insert(id);
        }

        for t in threads {
            t.join().expect("thread panicked");
        }
    }

    #[test]
    fn test_infallible_sonyflake_concurrency() {
        let now = Utc::now();
        let sf = Settings::new().set_start_time(now).into_infallible_sonyflake().unwrap();

        let (tx, rx) = std::sync::mpsc::channel::<u64>();

        let mut threads = Vec::<JoinHandle<()>>::with_capacity(1000);
        for _ in 0..100 {
            let mut thread_sf = sf.clone();
            let thread_tx = tx.clone();
            threads.push(std::thread::spawn(move || {
                for _ in 0..1000 {
                    thread_tx.send(thread_sf.next_id()).unwrap();
                }
            }));
        }

        let mut ids = HashSet::new();
        for _ in 0..100000 {
            let id = rx.recv().unwrap();
            assert!(!ids.contains(&id), "duplicate id: {}", id);
            ids.insert(id);
        }

        for t in threads {
            t.join().expect("thread panicked");
        }
    }

    #[test]
    fn test_error_send_sync() {
        let res = SonyFlake::new(Settings::new());
        std::thread::spawn(move || {
            let _ = res.is_ok();
        })
            .join()
            .unwrap();
    }
}
