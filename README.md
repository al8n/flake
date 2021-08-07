# flake 

[![Build](https://github.com/bahlo/sonyflake-rs/workflows/Build/badge.svg)](https://github.com/bahlo/sonyflake-rs/actions?query=workflow%3ABuild)
[![crates.io](https://img.shields.io/crates/v/sonyflake.svg)](https://crates.io/crates/sonyflake)
[![docs.rs](https://docs.rs/sonyflake/badge.svg)](https://docs.rs/sonyflake/)
[![License](https://img.shields.io/crates/l/sonyflake)](LICENSE-APACHE)

A distributed unique ID generator inspired by [Twitter's Snowflake](https://blog.twitter.com/2010/announcing-snowflake).

This is a Rust implementation of the original [sony/sonyflake](https://github.com/sony/sonyflake), which is written in Go.

A Sonyflake ID is composed of

```
39 bits for time in units of 10 msec
 8 bits for a sequence number
16 bits for a machine id
```

## Install

Add the following to your `Cargo.toml`:
```toml
[dependencies]
infallible-sonyflake = "0.1"
```

## Quickstart
1. **Fallible SonyFlake**  
   `Sonyflake` may fail to generate a unique ID when we call `next_id` if time overflows.
   ```rust
   use infallible_sonyflake::{SonyFlake, Settings};
   use chrono::Utc;
   
   fn main() {
       let now = Utc::now();
       let mut sf = Settings::new().set_start_time(now).into_sonyflake().unwrap();
       let next_id = sf.next_id().unwrap();
       println!("{}", next_id); 
   }
   ```
2. **Infallible SonyFlake**   
   `InfaillibleSonyFlake` will always generate a unique ID when we call `next_id` if time overflow happens, it will refresh the `start_time` to the current time.
   ```rust
   use infallible_sonyflake::{InfallibleSonyFlake, Settings};
   use chrono::Utc;
   
   fn main() {
       let now = Utc::now();
       let mut sf = Settings::new().set_start_time(now).into_infallible_sonyflake().unwrap();
       let next_id = sf.next_id();
       println!("{}", next_id); 
   } 
   ```
3. **Custom machine ID and machine ID checker**
   ```rust
   use infallible_sonyflake::{InfallibleSonyFlake, Settings, MachineID, MachineIDChecker, IDParts, Error};
   use chrono::Utc;
   
   struct CustomMachineID {
       counter: u64,
       id: u16,
   }
   
   impl MachineID for CustomMachineID {
       fn machine_id(&mut self) -> Result<u16, Box<dyn std::error::Error + Send + Sync + 'static>> {
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
   
   fn main() {
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
   
       assert_eq!(format!("{}", err), Error::InvalidMachineID(2).to_string());
   }
   ```

#### License

<sup>
Licensed under <a href="LICENSE">Apache License, Version
2.0</a>.
</sup>
<br>
<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license.
</sub>
