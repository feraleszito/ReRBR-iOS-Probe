//! Probe host storage, not physics. Stable 4 KiB allocations replace a 4 GiB mmap.
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};

const COUNT: usize = 1 << 20;
const WORDS: usize = 1024;
const MAX_PAGES: usize = 256 * 1024 * 1024 / 4096;

pub(super) struct SparseMap {
    pointers: Box<[AtomicPtr<AtomicU32>]>,
    marks: Box<[AtomicU64]>,
    allocated: AtomicUsize,
}
impl Default for SparseMap {
    fn default() -> Self {
        Self {
            pointers: (0..COUNT).map(|_| AtomicPtr::new(std::ptr::null_mut())).collect(),
            marks: (0..COUNT).map(|_| AtomicU64::new(0)).collect(),
            allocated: AtomicUsize::new(0),
        }
    }
}
impl SparseMap {
    pub fn ensure(&self, at: u32) {
        let pointer = &self.pointers[(at >> 12) as usize];
        if !pointer.load(Ordering::Acquire).is_null() {
            return;
        }
        let previous = self.allocated.fetch_add(1, Ordering::Relaxed);
        if previous >= MAX_PAGES {
            self.allocated.fetch_sub(1, Ordering::Relaxed);
            panic!("physics-probe: sparse page payload exceeds 256 MiB host budget");
        }
        let page: Box<[AtomicU32]> = (0..WORDS).map(|_| AtomicU32::new(0)).collect();
        let raw = Box::into_raw(page).cast::<AtomicU32>();
        if pointer.compare_exchange(std::ptr::null_mut(), raw, Ordering::Release, Ordering::Acquire).is_err() {
            // SAFETY: this allocation lost publication and has no readers.
            unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(raw, WORDS))) };
            self.allocated.fetch_sub(1, Ordering::Relaxed);
        }
    }
    pub fn word_ptr(&self, at: u32) -> *const AtomicU32 {
        let page = self.pointers[(at >> 12) as usize].load(Ordering::Acquire);
        assert!(!page.is_null(), "physics-probe: internal access to unallocated sparse page");
        // SAFETY: stable allocation until the last Arc is dropped; offset is in this page.
        unsafe { page.add(((at & 4095) / 4) as usize) }
    }
    pub fn mark(&self, at: u32) -> &AtomicU64 {
        &self.marks[(at >> 12) as usize]
    }
    pub fn allocated_bytes(&self) -> usize {
        self.allocated.load(Ordering::Relaxed) * 4096
    }
    pub fn metadata_bytes(&self) -> usize {
        COUNT * (size_of::<AtomicPtr<AtomicU32>>() + size_of::<AtomicU64>())
    }
}
impl Drop for SparseMap {
    fn drop(&mut self) {
        for slot in &mut self.pointers {
            let raw = *slot.get_mut();
            if !raw.is_null() {
                // SAFETY: last Arc owner, each published page is owned exactly once.
                unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(raw, WORDS))) };
            }
        }
    }
}
