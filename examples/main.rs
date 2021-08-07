use infallible_sonyflake::{InfallibleSonyFlake, SonyFlake, Settings, MachineID, MachineIDChecker, IDParts, Error};
use chrono::Utc;

fn fallible_sonyflake() {
    let now = Utc::now();
    let mut sf = Settings::new().set_start_time(now).into_sonyflake().unwrap();
    let next_id = sf.next_id().unwrap();
    println!("{}", next_id);

    let mut sf = SonyFlake::new(Settings::new().set_start_time(now)).unwrap();
    let next_id = sf.next_id().unwrap();
    println!("{}", next_id);
}

fn infallible_sonyflake() {
    let now = Utc::now();
    let mut sf = Settings::new().set_start_time(now).into_infallible_sonyflake().unwrap();
    let next_id = sf.next_id();
    println!("{}", next_id);

    let mut sf = InfallibleSonyFlake::new(Settings::new().set_start_time(now)).unwrap();
    let next_id = sf.next_id();
    println!("{}", next_id);
}

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

fn with_custom_machine_id_and_checker() {
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

fn main() {
    fallible_sonyflake();
    infallible_sonyflake();
    with_custom_machine_id_and_checker();
}