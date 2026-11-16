use super::{pages::Pages, pages_reference::Pages as Flat, Backend};

fn exercise(p: &impl Backend) -> Vec<u8> {
    let mut out = Vec::new();
    let mut seed = 73u32;
    for i in 0..1000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let at = [0x1000, 0x1000_0000, 0x791a_0000, 0xffff_f000][i % 4] + (seed % 4093);
        p.write_u32(at, seed);
        out.extend(p.read_u32(at).unwrap().to_le_bytes());
    }
    p.write(0xffff_fffe, &[1, 2, 3, 4]);
    let mut wrap = [0; 4];
    assert!(p.read(0xffff_fffe, &mut wrap));
    out.extend(wrap);
    let mut missing = [0; 1];
    assert!(!p.read(0x0800_0000, &mut missing));
    out
}
#[test]
fn sparse_matches_original_flat_word_byte_page_boundary_and_wrap_semantics() {
    let sparse = Pages::default();
    let flat = Flat::default();
    assert_eq!(exercise(&sparse), exercise(&flat));
    let epoch = sparse.end_epoch();
    let old = flat.end_epoch();
    let s = sparse.snapshot();
    let f = flat.snapshot();
    sparse.set_word(0x8800, 99);
    flat.set_word(0x8800, 99);
    assert!(sparse.written_after(0x8800, epoch));
    assert!(flat.written_after(0x8800, old));
    sparse.restore(&s);
    flat.restore(&f);
    assert_eq!(sparse.regions(), flat.regions());
    assert_eq!(sparse.word(0x8800), None);
    sparse.set_word(0x8804, 123);
    flat.set_word(0x8804, 123);
    // Restore intentionally retains old bytes on unmapped pages, exactly as upstream.
    assert_eq!(sparse.word(0x8800), flat.word(0x8800));
    assert!(sparse.allocated_bytes() < 1024 * 1024);
    assert_eq!(sparse.metadata_bytes(), 16 * 1024 * 1024);
}
