//! Fiat: The Journal DSL
//! Let there be light — and let there be structure.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::{collections::BTreeMap, string::ToString};
use alloc::{fmt, format, vec};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags as Flags, PhysFrame, Size4KiB,
    },
};

use crate::thing::make_uuid_from_seed;

// Bootstrap paging declaration: early identity mapping for heap setup
pub const BOOTSTRAP_PAGING_DSL: &str = r#"
kind page
kind frame

fiat heap_page_0:page
fiat heap_page_1:page
fiat phys_0x9000:frame
fiat phys_0xA000:frame

heap_page_0 maps_to phys_0x9000
heap_page_1 maps_to phys_0xA000

heap_page_0.flags = PRESENT|WRITABLE
heap_page_1.flags = PRESENT|WRITABLE

fiat root_page_table:page_table
root_page_table includes heap_page_0
root_page_table includes heap_page_1

kernel uses root_page_table
"#;

/// Apply paging instructions from facts.
pub fn materialize_paging_from_facts(
    facts: &[Fact],
    mapper: &mut OffsetPageTable,
    allocator: &mut impl FrameAllocator<Size4KiB>,
    symbol_table: &SymbolTable,
) {
    for fact in facts.iter() {
        if fact.predicate == "maps_to" {
            if let (Some(_subject_id), Some(_object_id)) = (
                symbol_table.lookup(&fact.subject),
                symbol_table.lookup(&fact.object),
            ) {
                let virt_addr = parse_page_address(&fact.subject);
                let phys_addr = parse_frame_address(&fact.object);
                if let (Some(vaddr), Some(paddr)) = (virt_addr, phys_addr) {
                    let page: Page<Size4KiB> = Page::containing_address(vaddr);
                    let frame: PhysFrame<Size4KiB> = PhysFrame::containing_address(paddr);
                    unsafe {
                        mapper
                            .map_to(page, frame, Flags::PRESENT | Flags::WRITABLE, allocator)
                            .expect("map_to failed")
                            .flush();
                    }
                }
            }
        }
    }
}

pub fn parse_and_materialize_bootstrap(
    mapper: &mut OffsetPageTable,
    allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use crate::fiat::{SymbolTable, parse_fiat_line};
    let symbol_table = SymbolTable::default();
    let facts: alloc::vec::Vec<_> = BOOTSTRAP_PAGING_DSL
        .lines()
        .filter_map(|line| parse_fiat_line(line, 0, Some("bootstrap".into())))
        .collect();
    materialize_paging_from_facts(&facts, mapper, allocator, &symbol_table);
}

fn parse_page_address(name: &str) -> Option<VirtAddr> {
    const HEAP_START: u64 = 0x_4444_4444_0000;
    if name.starts_with("heap_page_0") {
        Some(VirtAddr::new(HEAP_START))
    } else if name.starts_with("heap_page_1") {
        Some(VirtAddr::new(HEAP_START + 0x1000))
    } else {
        None
    }
}

fn parse_frame_address(name: &str) -> Option<PhysAddr> {
    if let Some(hex) = name.strip_prefix("phys_0x") {
        u64::from_str_radix(hex, 16).ok().map(PhysAddr::new)
    } else {
        None
    }
}

/// Represents a single journaled instruction — a 'fact' of creation.
#[derive(Debug, Clone)]
pub struct Fact {
    pub timestamp: u64,
    pub origin: Option<String>,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub complements: BTreeMap<String, String>,
    pub tags: Vec<String>,
    pub comment: Option<String>,
    pub partial: bool,
}

impl fmt::Display for Fact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)?;
        for (k, v) in &self.complements {
            write!(f, " @ {}={}", k, v)?;
        }
        for tag in &self.tags {
            write!(f, " #{}", tag)?;
        }
        if let Some(comment) = &self.comment {
            write!(f, " // {}", comment)?;
        }
        if self.partial {
            write!(f, " [partial]")?;
        }
        Ok(())
    }
}

/// A symbol table to map user-visible names to UUIDs.
#[derive(Default)]
pub struct SymbolTable {
    names: BTreeMap<String, String>,
    reverse: BTreeMap<String, String>,
}

impl SymbolTable {
    pub fn resolve_or_create(&mut self, name: &str) -> String {
        if let Some(uuid) = self.names.get(name) {
            uuid.clone()
        } else {
            let uuid = make_uuid_from_seed(name.as_bytes());
            let uuid_str = uuid.to_string();
            self.names.insert(name.to_string(), uuid_str.clone());
            self.reverse.insert(uuid_str.clone(), name.to_string());
            uuid_str
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&str> {
        self.names.get(name).map(|s| s.as_str())
    }

    pub fn alias(&self, uuid: &str) -> Option<&str> {
        self.reverse.get(uuid).map(|s| s.as_str())
    }

    pub fn to_string_map(&self) -> String {
        self.names
            .iter()
            .map(|(k, v)| format!("{} = {}\n", k, v))
            .collect()
    }

    pub fn from_string_map(s: &str) -> Self {
        let mut names = BTreeMap::new();
        let mut reverse = BTreeMap::new();
        for line in s.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim().to_string();
                let val = v.trim().to_string();
                names.insert(key.clone(), val.clone());
                reverse.insert(val, key);
            }
        }
        Self { names, reverse }
    }
}

/// A parser that turns Fiat DSL lines into Fact structs.
pub fn parse_fiat_line(line: &str, timestamp: u64, origin: Option<String>) -> Option<Fact> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }

    if trimmed.starts_with('~') {
        let target = trimmed.trim_start_matches('~').trim();
        let (subject, predicate, object) = if let Some(dot) = target.find('.') {
            let (s, rest) = target.split_at(dot);
            let field = &rest[1..];
            (s.to_string(), field.to_string(), "__TOMBSTONE__".into())
        } else if target.starts_with("kind ") {
            (
                "kind".into(),
                target[5..].to_string(),
                "__TOMBSTONE__".into(),
            )
        } else {
            (
                target.to_string(),
                "__TOMBSTONE__".into(),
                "__TOMBSTONE__".into(),
            )
        };

        return Some(Fact {
            timestamp,
            origin,
            subject,
            predicate,
            object,
            complements: BTreeMap::new(),
            tags: vec!["tombstone".into()],
            comment: Some("tombstone".into()),
            partial: true,
        });
    }

    let mut parts = trimmed.split_whitespace();
    let subject = parts.next()?.to_string();
    let predicate = parts.next()?.to_string();
    let object = parts.next()?.to_string();

    let mut complements = BTreeMap::new();
    let mut tags = Vec::new();
    let mut comment = None;

    let mut in_comment = false;
    for token in parts {
        if in_comment {
            comment
                .get_or_insert_with(String::new)
                .push_str(&format!("{} ", token));
            continue;
        }
        if token.starts_with("@") {
            if let Some((k, v)) = token[1..].split_once('=') {
                complements.insert(k.to_string(), v.to_string());
            }
        } else if token.starts_with("#") {
            tags.push(token[1..].to_string());
        } else if token == "//" {
            in_comment = true;
        }
    }

    Some(Fact {
        timestamp,
        origin,
        subject,
        predicate,
        object,
        complements,
        tags,
        comment: comment.map(|s| s.trim().to_string()),
        partial: false,
    })
}

/// Save the current journal to disk (or memory-mapped file, etc)
pub fn write_journal_to_string(facts: &[Fact]) -> String {
    facts.iter().map(|f| format!("{}\n", f)).collect()
}
