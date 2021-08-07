<div align="center">
<h1>Infallible-SonyFlake</h1>
</div>
<div align="center">

A distributed unique ID generator inspired by [Twitter's Snowflake](https://blog.twitter.com/2010/announcing-snowflake).

This is a Rust implementation of the original [sony/sonyflake](https://github.com/sony/sonyflake), which is written in Go.

[<img alt="github" src="https://img.shields.io/badge/GITHUB-infallible--sonyflake-8da0cb?style=for-the-badge&logo=Github" height="22">][Github-url]
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-infallible--sonyflake-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/infallible-sonyflake)
[<img alt="crates.io" src="https://img.shields.io/crates/v/infallible-sonyflake?logo=rust&style=for-the-badge" height="22">][crates-url]

[<img alt="Build" src="https://img.shields.io/badge/Build-passing-brightgreen?style=for-the-badge&logo=Github-Actions" height="22">][CI-url]
[<img alt="codecov" src="https://img.shields.io/codecov/c/gh/al8n/flake?style=for-the-badge&token=N7EPJLUZ0G&logo=codecov" height="22">][codecov-url]
[<img alt="license" src="https://img.shields.io/badge/License-Apache%202.0-blue.svg?style=for-the-badge&logo=Apache" height="22">][license-url]

English | [简体中文](README-zh_CN.md)

A Sonyflake ID is composed of

```
39 bits for time in units of 10 msec
 8 bits for a sequence number
16 bits for a machine id
```
</div>


<div align="center">
<h2>Install</h2>
</div>

Add the following to your `Cargo.toml`:
```toml
[dependencies]
infallible-sonyflake = "0.1"
```

<div align="center">
<h2>Quickstart</h2>
</div>

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

<div align="center">
<h4>License</h4>

<sup>
Licensed under <a href="LICENSE">Apache License, Version
2.0</a>.
</sup>
<br>
<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license.
</sub>
</div>


[Github-url]: https://github.com/al8n/flake/
[CI-url]: https://github.com/al8n/flake/actions/workflows/ci.yml
[codecov-url]: https://app.codecov.io/gh/al8n/flake/
[license-url]: https://opensource.org/licenses/Apache-2.0
[crates-url]: https://crates.io/crates/infallible-sonyflaked

