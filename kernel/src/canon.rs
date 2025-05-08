use uuid::Uuid;

// -------------------------------
// Canonical UUIDs
// -------------------------------

// Special UUIDs
pub const NOTHING: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000000000);
pub const SYSTEM: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000000001);
pub const KERNEL: Uuid = SYSTEM;

// Core Kinds
pub const KIND_KIND: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000000007); // KindKind (meta-kind)
pub const VERB_KIND: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000000008); // Verb
pub const KERNEL_KIND: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000001002);
pub const DEVICE_KIND: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000001003);
pub const JOURNAL_KIND: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000001001);

// Core Verbs
pub const IS_A: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_00000090000a);
pub const OWNS: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_00000090030a);
pub const READS: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_00000090200a);
pub const WRITES: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_00000097000a);

// Core Things
pub const JOURNAL: Uuid = Uuid::from_u128(0x22222222_2222_2222_2222_222222220002);
pub const KEYBOARD1: Uuid = Uuid::from_u128(0x22222222_2222_2222_2222_222222220003);
pub const FRAMEBUFFER1: Uuid = Uuid::from_u128(0x22222222_2222_2222_2222_222222220004);

// Other well-known Things (optional add-ons)
pub const PROCESS_KIND: Uuid = Uuid::from_u128(0x00000000_0000_0000_0000_0000_000000000009); // Process kind
