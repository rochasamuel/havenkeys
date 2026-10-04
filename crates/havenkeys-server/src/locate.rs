//! Coarse location for a client address. A stub until a database is wired in.

pub struct Locator;

impl Locator {
    pub fn locate(&self, _ip: &str) -> Option<String> {
        None
    }
}
