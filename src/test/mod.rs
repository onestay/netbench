mod builder;
mod test;

pub use builder::*;
pub use test::*;
#[derive(Debug)]
enum ControlMessage {
    StopTest,
    GetInterval,
}

#[derive(Debug)]
enum IPPref {
    V4,
    V6,
}
