#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BValue(pub i64);

impl BValue {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

// Bit 62 set → local frame address
const LOCAL_TAG: i64 = 1_i64 << 62;
// Bit 61 set (bit 62 clear) → heap address
const HEAP_TAG: i64 = 1_i64 << 61;

#[derive(Clone, Copy, Debug)]
pub enum Address {
    Global(usize),
    Local(usize),
    Heap(usize),
}

pub fn encode_global(index: usize) -> i64 {
    index as i64
}

pub fn encode_local(index: usize) -> i64 {
    LOCAL_TAG | index as i64
}

pub fn encode_heap(index: usize) -> i64 {
    HEAP_TAG | index as i64
}

pub fn is_local(addr: i64) -> bool {
    (addr & LOCAL_TAG) != 0
}

pub fn is_heap(addr: i64) -> bool {
    (addr & HEAP_TAG) != 0 && (addr & LOCAL_TAG) == 0
}

pub fn decode_address(addr: i64) -> Address {
    if is_local(addr) {
        Address::Local((addr & !LOCAL_TAG) as usize)
    } else if is_heap(addr) {
        Address::Heap((addr & !HEAP_TAG) as usize)
    } else {
        Address::Global(addr as usize)
    }
}

pub fn add_offset(addr: i64, offset: i64) -> i64 {
    if is_local(addr) {
        let base = addr & !LOCAL_TAG;
        LOCAL_TAG | (base + offset)
    } else if is_heap(addr) {
        let base = addr & !HEAP_TAG;
        HEAP_TAG | (base + offset)
    } else {
        addr + offset
    }
}

/// Scalar global variables — one slot per external declaration.
#[derive(Clone, Debug)]
pub struct GlobalMemory {
    pub data: Vec<BValue>,
}

impl GlobalMemory {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn allocate_block(&mut self, slots: usize) -> usize {
        let base = self.data.len();
        self.data.resize(self.data.len() + slots, BValue(0));
        base
    }
}

/// Heap segment — all vectors and string literals live here.
/// Uses a simple bump allocator; no free for MVP.
#[derive(Clone, Debug)]
pub struct Heap {
    pub data: Vec<BValue>,
}

impl Heap {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Allocate `slots` contiguous slots, return base index.
    pub fn allocate(&mut self, slots: usize) -> usize {
        let base = self.data.len();
        self.data.resize(self.data.len() + slots, BValue(0));
        base
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub func: String,
    pub locals: Vec<BValue>,
    pub nargs: usize,
    /// Stack depth at the time this frame was pushed (for debugging).
    pub base_pointer: usize,
}
