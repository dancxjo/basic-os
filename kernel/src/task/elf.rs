use core::mem::size_of;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

pub struct Elf<'a> {
    pub header: &'a Elf64Header,
    pub program_headers: &'a [ProgramHeader],
}

impl<'a> Elf<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, &'static str> {
        if data.len() < size_of::<Elf64Header>() {
            return Err("Data too short for ELF header");
        }

        let header_ptr = data.as_ptr() as *const Elf64Header;
        let header = unsafe { &*header_ptr };

        // Verify magic
        if header.e_ident[0] != 0x7f
            || header.e_ident[1] != b'E'
            || header.e_ident[2] != b'L'
            || header.e_ident[3] != b'F'
        {
            return Err("Invalid ELF magic");
        }

        // Verify 64-bit
        if header.e_ident[4] != 2 {
            return Err("Not a 64-bit ELF");
        }

        let ph_off = header.e_phoff as usize;
        let ph_num = header.e_phnum as usize;
        let ph_size = header.e_phentsize as usize;

        if ph_size != size_of::<ProgramHeader>() {
            // It might be different if extensions are used, but for standard x86_64 ELF it should match.
            // Or we can just use the size from the header to iterate.
            // But for simplicity, let's enforce it for now or handle it.
            // Actually, let's just cast the slice if it matches.
        }

        if data.len() < ph_off + ph_num * ph_size {
            return Err("Data too short for program headers");
        }

        let ph_ptr = unsafe { data.as_ptr().add(ph_off) } as *const ProgramHeader;
        let program_headers = unsafe { core::slice::from_raw_parts(ph_ptr, ph_num) };

        Ok(Elf {
            header,
            program_headers,
        })
    }
}
pub const ET_DYN: u16 = 3;
