#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uuid(pub u128);

impl Uuid {
    pub fn from_u128(u: u128) -> Self {
        Uuid(u)
    }

    pub fn as_u128(self) -> u128 {
        self.0
    }

    pub fn new_fake(name: &str) -> Self {
        // Very dumb hash for now
        let mut hash = 0u128;
        for byte in name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u128);
        }
        Uuid(hash)
    }
}
