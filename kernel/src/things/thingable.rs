pub trait Thingable {
    fn kind() -> &'static str;
    fn serialize(&self) -> alloc::vec::Vec<u8>;
    fn deserialize(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized;
}
