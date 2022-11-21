mod builder;
mod test;

pub use test::*;
pub use builder::*;
#[derive(Debug)]
enum ControlMessage {
    StopTest,
    GetInterval,
}

#[derive(Debug)]
enum IPPref {
    V4,
    V6
}
