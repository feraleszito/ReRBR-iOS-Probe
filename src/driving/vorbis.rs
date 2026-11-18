//! Pure-Rust Xiph.org Vorbis I decoder for FSB5 headerless Vorbis car sound banks.
//!
//! Implements the open Vorbis I specification (MSB/LSB bitreader, Huffman codebooks,
//! Floor 1 piecewise curves, Residue 0/1/2, channel polar coupling, and O(N log N)
//! inverse MDCT with Vorbis overlap-add windowing) without external dependencies.

use std::f64::consts::{FRAC_PI_2, PI};

/// Standard Vorbis I Section 7.2 inverse dB lookup table (0..=255).
#[rustfmt::skip]
const FLOOR1_INVERSE_DB_TABLE: [f64; 256] = [
    1.0649863e-07, 1.1341951e-07, 1.2079015e-07, 1.2863978e-07,
    1.3699951e-07, 1.4590251e-07, 1.5538408e-07, 1.6548181e-07,
    1.7623575e-07, 1.8768855e-07, 1.9988561e-07, 2.1287530e-07,
    2.2670913e-07, 2.4144197e-07, 2.5713223e-07, 2.7384213e-07,
    2.9163793e-07, 3.1059021e-07, 3.3077411e-07, 3.5226968e-07,
    3.7516214e-07, 3.9954229e-07, 4.2550680e-07, 4.5315863e-07,
    4.8260743e-07, 5.1396998e-07, 5.4737065e-07, 5.8294187e-07,
    6.2082472e-07, 6.6116941e-07, 7.0413592e-07, 7.4989464e-07,
    7.9862701e-07, 8.5052630e-07, 9.0579828e-07, 9.6466216e-07,
    1.0273513e-06, 1.0941144e-06, 1.1652161e-06, 1.2409384e-06,
    1.3215816e-06, 1.4074654e-06, 1.4989305e-06, 1.5963394e-06,
    1.7000785e-06, 1.8105592e-06, 1.9282195e-06, 2.0535261e-06,
    2.1869758e-06, 2.3290978e-06, 2.4804557e-06, 2.6416497e-06,
    2.8133190e-06, 2.9961443e-06, 3.1908506e-06, 3.3982101e-06,
    3.6190449e-06, 3.8542308e-06, 4.1047004e-06, 4.3714470e-06,
    4.6555282e-06, 4.9580707e-06, 5.2802740e-06, 5.6234160e-06,
    5.9888572e-06, 6.3780469e-06, 6.7925283e-06, 7.2339451e-06,
    7.7040476e-06, 8.2047000e-06, 8.7378876e-06, 9.3057248e-06,
    9.9104632e-06, 1.0554501e-05, 1.1240392e-05, 1.1970856e-05,
    1.2748789e-05, 1.3577278e-05, 1.4459606e-05, 1.5399272e-05,
    1.6400004e-05, 1.7465768e-05, 1.8600792e-05, 1.9809576e-05,
    2.1096914e-05, 2.2467911e-05, 2.3928002e-05, 2.5482978e-05,
    2.7139006e-05, 2.8902651e-05, 3.0780908e-05, 3.2781225e-05,
    3.4911534e-05, 3.7180282e-05, 3.9596466e-05, 4.2169667e-05,
    4.4910090e-05, 4.7828601e-05, 5.0936773e-05, 5.4246931e-05,
    5.7772202e-05, 6.1526565e-05, 6.5524908e-05, 6.9783085e-05,
    7.4317983e-05, 7.9147585e-05, 8.4291040e-05, 8.9768747e-05,
    9.5602426e-05, 0.00010181521, 0.00010843174, 0.00011547824,
    0.00012298267, 0.00013097477, 0.00013948625, 0.00014855085,
    0.00015820453, 0.00016848555, 0.00017943469, 0.00019109536,
    0.00020351382, 0.00021673929, 0.00023082423, 0.00024582449,
    0.00026179955, 0.00027881276, 0.00029693158, 0.00031622787,
    0.00033677814, 0.00035866388, 0.00038197188, 0.00040679456,
    0.00043323036, 0.00046138411, 0.00049136745, 0.00052329927,
    0.00055730621, 0.00059352311, 0.00063209358, 0.00067317058,
    0.00071691700, 0.00076350630, 0.00081312324, 0.00086596457,
    0.00092223983, 0.00098217216, 0.0010459992, 0.0011139742,
    0.0011863665, 0.0012634633, 0.0013455702, 0.0014330129,
    0.0015261382, 0.0016253153, 0.0017309374, 0.0018434235,
    0.0019632195, 0.0020908006, 0.0022266726, 0.0023713743,
    0.0025254795, 0.0026895994, 0.0028643847, 0.0030505286,
    0.0032487691, 0.0034598925, 0.0036847358, 0.0039241906,
    0.0041792066, 0.0044507950, 0.0047400328, 0.0050480668,
    0.0053761186, 0.0057254891, 0.0060975636, 0.0064938176,
    0.0069158225, 0.0073652516, 0.0078438871, 0.0083536271,
    0.0088964928, 0.009474637, 0.010090352, 0.010746080,
    0.011444421, 0.012188144, 0.012980198, 0.013823725,
    0.014722068, 0.015678791, 0.016697687, 0.017782797,
    0.018938423, 0.020169149, 0.021479854, 0.022875735,
    0.024362330, 0.025945531, 0.027631618, 0.029427276,
    0.031339626, 0.033376252, 0.035545228, 0.037855157,
    0.040315199, 0.042935108, 0.045725273, 0.048696758,
    0.051861348, 0.055231591, 0.058820850, 0.062643361,
    0.066714279, 0.071049749, 0.075666962, 0.080584227,
    0.085821044, 0.091398179, 0.097337747, 0.10366330,
    0.11039993, 0.11757434, 0.12521498, 0.13335215,
    0.14201813, 0.15124727, 0.16107617, 0.17154380,
    0.18269168, 0.19456402, 0.20720788, 0.22067342,
    0.23501402, 0.25028656, 0.26655159, 0.28387361,
    0.30232132, 0.32196786, 0.34289114, 0.36517414,
    0.38890521, 0.41417847, 0.44109412, 0.46975890,
    0.50028648, 0.53279791, 0.56742212, 0.60429640,
    0.64356699, 0.68538959, 0.72993007, 0.77736504,
    0.82788260, 0.88168307, 0.9389798, 1.0,
];

struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn read_bits(&mut self, n: u8) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        if n > 32 {
            return Err("bit read > 32".into());
        }
        let mut out = 0u32;
        let mut got = 0u8;
        while got < n {
            let b = *self
                .data
                .get(self.byte_pos)
                .ok_or_else(|| "vorbis: unexpected end of packet".to_string())?;
            let avail = 8 - self.bit_pos;
            let take = (n - got).min(avail);
            let mask = ((1u16 << take) - 1) as u8;
            let bits = (b >> self.bit_pos) & mask;
            out |= u32::from(bits) << got;
            got += take;
            self.bit_pos += take;
            if self.bit_pos == 8 {
                self.bit_pos = 0;
                self.byte_pos += 1;
            }
        }
        Ok(out)
    }

    #[inline]
    fn read_bit_fast(&mut self) -> Option<u32> {
        let b = *self.data.get(self.byte_pos)?;
        let bit = u32::from((b >> self.bit_pos) & 1);
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(bit)
    }

    fn read_flag(&mut self) -> Result<bool, String> {
        Ok(self.read_bits(1)? == 1)
    }
}

#[inline]
fn ilog(x: u32) -> u8 {
    32 - x.leading_zeros() as u8
}

fn float32_unpack(val: u32) -> f32 {
    let mantissa = (val & 0x1f_ffff) as i32;
    let sign = val & 0x8000_0000;
    let exp = ((val & 0x7fe0_0000) >> 21) as i32;
    let signed_m = if sign != 0 { -mantissa } else { mantissa };
    (signed_m as f32) * (2.0f32).powi(exp - 788)
}

fn lookup1_values(entries: usize, dimensions: usize) -> usize {
    if dimensions == 0 || entries == 0 {
        return 0;
    }
    let mut r = (entries as f64).powf(1.0 / dimensions as f64).floor() as usize;
    loop {
        let mut prod = 1usize;
        let mut prod1 = 1usize;
        for _ in 0..dimensions {
            prod = prod.saturating_mul(r);
            prod1 = prod1.saturating_mul(r + 1);
        }
        if prod1 <= entries {
            r += 1;
        } else if prod > entries && r > 0 {
            r -= 1;
        } else {
            return r;
        }
    }
}

#[derive(Clone, Copy)]
struct HuffNode {
    children: [i32; 2], // >= 0: child node index, < 0: -(entry_index + 1), i32::MIN: empty
}

impl HuffNode {
    const EMPTY: i32 = i32::MIN;
}

struct Codebook {
    dimensions: usize,
    nodes: Vec<HuffNode>,
    vq: Vec<f32>,
}

impl Codebook {
    fn parse(br: &mut BitReader<'_>) -> Result<Self, String> {
        let sync = br.read_bits(24)?;
        if sync != 0x564342 {
            return Err(format!("vorbis: bad codebook sync {sync:#x}"));
        }
        let dimensions = br.read_bits(16)? as usize;
        let entries = br.read_bits(24)? as usize;
        if entries > 1_048_576 {
            return Err("vorbis: codebook entries overflow".into());
        }
        let ordered = br.read_flag()?;
        let mut lengths = vec![0u8; entries];
        if !ordered {
            let sparse = br.read_flag()?;
            for len in &mut lengths {
                if sparse {
                    if br.read_flag()? {
                        *len = br.read_bits(5)? as u8 + 1;
                    }
                } else {
                    *len = br.read_bits(5)? as u8 + 1;
                }
            }
        } else {
            let mut cur_len = br.read_bits(5)? as u8 + 1;
            let mut i = 0usize;
            while i < entries {
                let bits = ilog((entries - i) as u32);
                let num = br.read_bits(bits)? as usize;
                if i + num > entries {
                    return Err("vorbis: ordered codebook overflow".into());
                }
                for item in &mut lengths[i..i + num] {
                    *item = cur_len;
                }
                i += num;
                cur_len += 1;
            }
        }

        let nodes = build_huffman_tree(&lengths)?;

        let lookup_type = br.read_bits(4)?;
        let mut vq = Vec::new();
        match lookup_type {
            0 => {}
            1 | 2 => {
                if dimensions == 0 {
                    return Err("vorbis: VQ codebook with 0 dimensions".into());
                }
                let min_v = float32_unpack(br.read_bits(32)?);
                let delta_v = float32_unpack(br.read_bits(32)?);
                let val_bits = br.read_bits(4)? as u8 + 1;
                let seq_p = br.read_flag()?;
                let lookup_values = if lookup_type == 1 {
                    lookup1_values(entries, dimensions)
                } else {
                    entries
                        .checked_mul(dimensions)
                        .ok_or("vorbis: lookup size overflow")?
                };
                let mut mults = Vec::with_capacity(lookup_values);
                for _ in 0..lookup_values {
                    mults.push(br.read_bits(val_bits)?);
                }
                vq.resize(entries * dimensions, 0.0);
                if lookup_type == 1 {
                    for (e, len) in lengths.iter().enumerate() {
                        if *len == 0 {
                            continue;
                        }
                        let mut index_div = 1usize;
                        let mut last = 0.0f32;
                        for j in 0..dimensions {
                            let off = (e / index_div) % lookup_values;
                            let val = mults[off] as f32 * delta_v + min_v + last;
                            vq[e * dimensions + j] = val;
                            if seq_p {
                                last = val;
                            }
                            index_div *= lookup_values;
                        }
                    }
                } else {
                    for (e, len) in lengths.iter().enumerate() {
                        if *len == 0 {
                            continue;
                        }
                        let mut last = 0.0f32;
                        let off = e * dimensions;
                        for j in 0..dimensions {
                            let val = mults[off + j] as f32 * delta_v + min_v + last;
                            vq[off + j] = val;
                            if seq_p {
                                last = val;
                            }
                        }
                    }
                }
            }
            other => return Err(format!("vorbis: unsupported lookup_type {other}")),
        }

        Ok(Self {
            dimensions,
            nodes,
            vq,
        })
    }

    #[inline]
    fn decode_scalar(&self, br: &mut BitReader<'_>) -> Result<usize, String> {
        if self.nodes.is_empty() {
            return Err("vorbis: empty Huffman tree".into());
        }
        let mut node_idx = 0usize;
        loop {
            let bit = br
                .read_bit_fast()
                .ok_or_else(|| "vorbis: unexpected EOF in huffman".to_string())?
                as usize;
            let next = self.nodes[node_idx].children[bit];
            if next < 0 {
                if next == HuffNode::EMPTY {
                    return Err("vorbis: hit empty Huffman branch".into());
                }
                return Ok((-(next + 1)) as usize);
            }
            node_idx = next as usize;
        }
    }

    #[inline]
    fn vq_entry(&self, idx: usize) -> &[f32] {
        let start = idx * self.dimensions;
        &self.vq[start..start + self.dimensions]
    }
}

fn build_huffman_tree(lengths: &[u8]) -> Result<Vec<HuffNode>, String> {
    let used_count = lengths.iter().filter(|&&l| l > 0).count();
    let mut nodes = vec![HuffNode {
        children: [HuffNode::EMPTY, HuffNode::EMPTY],
    }];
    if used_count == 0 {
        return Ok(nodes);
    }
    if used_count == 1 {
        let (idx, &len) = lengths.iter().enumerate().find(|(_, &l)| l > 0).unwrap();
        if len != 1 {
            return Err("vorbis: single-entry codebook must have length 1".into());
        }
        nodes[0].children[0] = -(idx as i32 + 1);
        return Ok(nodes);
    }

    let mut root_nodes = vec![HuffNode {
        children: [HuffNode::EMPTY, HuffNode::EMPTY],
    }];
    fn insert(nodes: &mut Vec<HuffNode>, node_idx: usize, depth_left: u8, entry: i32) -> bool {
        if depth_left == 1 {
            for side in 0..2 {
                if nodes[node_idx].children[side] == HuffNode::EMPTY {
                    nodes[node_idx].children[side] = -(entry + 1);
                    return true;
                }
            }
            return false;
        }
        for side in 0..2 {
            let child = nodes[node_idx].children[side];
            if child == HuffNode::EMPTY {
                let new_idx = nodes.len();
                nodes.push(HuffNode {
                    children: [HuffNode::EMPTY, HuffNode::EMPTY],
                });
                nodes[node_idx].children[side] = new_idx as i32;
                if insert(nodes, new_idx, depth_left - 1, entry) {
                    return true;
                }
            } else if child >= 0 && insert(nodes, child as usize, depth_left - 1, entry) {
                return true;
            }
        }
        false
    }
    for (entry, &len) in lengths.iter().enumerate() {
        if len == 0 {
            continue;
        }
        if len > 32 || !insert(&mut root_nodes, 0, len, entry as i32) {
            return Err("vorbis: invalid or overspecified Huffman tree".into());
        }
    }
    Ok(root_nodes)
}

#[derive(Clone)]
struct Floor1Class {
    dimensions: usize,
    subclasses: u8,
    masterbook: Option<usize>,
    subclass_books: Vec<Option<usize>>,
}

struct Floor1 {
    partition_class_list: Vec<usize>,
    classes: Vec<Floor1Class>,
    multiplier: i32,
    rangebits: u8,
    x_list: Vec<i32>,
    sort_order: Vec<usize>,
    neighbors: Vec<(usize, usize)>,
}

struct Floor1Curve {
    final_y: Vec<i32>,
    step2_flag: Vec<bool>,
}

impl Floor1 {
    fn parse(br: &mut BitReader<'_>, codebook_count: usize) -> Result<Self, String> {
        let partitions = br.read_bits(5)? as usize;
        let mut partition_class_list = Vec::with_capacity(partitions);
        let mut max_class = 0usize;
        for _ in 0..partitions {
            let c = br.read_bits(4)? as usize;
            max_class = max_class.max(c);
            partition_class_list.push(c);
        }
        let mut classes = Vec::with_capacity(max_class + 1);
        for _ in 0..=max_class {
            let dimensions = br.read_bits(3)? as usize + 1;
            let subclasses = br.read_bits(2)? as u8;
            let masterbook = if subclasses > 0 {
                let mb = br.read_bits(8)? as usize;
                if mb >= codebook_count {
                    return Err("vorbis: floor1 masterbook OOB".into());
                }
                Some(mb)
            } else {
                None
            };
            let sc_count = 1usize << subclasses;
            let mut subclass_books = Vec::with_capacity(sc_count);
            for _ in 0..sc_count {
                let raw = br.read_bits(8)? as i32 - 1;
                if raw >= codebook_count as i32 {
                    return Err("vorbis: floor1 subclass book OOB".into());
                }
                subclass_books.push((raw >= 0).then_some(raw as usize));
            }
            classes.push(Floor1Class {
                dimensions,
                subclasses,
                masterbook,
                subclass_books,
            });
        }
        let multiplier = br.read_bits(2)? as i32 + 1;
        let rangebits = br.read_bits(4)? as u8;
        let mut x_list = vec![0i32, 1i32 << rangebits];
        for &c_idx in &partition_class_list {
            for _ in 0..classes[c_idx].dimensions {
                x_list.push(br.read_bits(rangebits)? as i32);
            }
        }
        if x_list.len() > 65 {
            return Err("vorbis: floor1 > 65 X points".into());
        }
        let mut sort_order: Vec<usize> = (0..x_list.len()).collect();
        sort_order.sort_by_key(|&i| x_list[i]);
        let mut neighbors = vec![(0usize, 0usize); x_list.len()];
        for i in 2..x_list.len() {
            let xi = x_list[i];
            let mut lo = 0usize;
            let mut hi = 1usize;
            for j in 0..i {
                let xj = x_list[j];
                if xj < xi && xj > x_list[lo] {
                    lo = j;
                }
                if xj > xi && xj < x_list[hi] {
                    hi = j;
                }
            }
            neighbors[i] = (lo, hi);
        }
        Ok(Self {
            partition_class_list,
            classes,
            multiplier,
            rangebits,
            x_list,
            sort_order,
            neighbors,
        })
    }

    fn decode_curve(
        &self,
        br: &mut BitReader<'_>,
        codebooks: &[Codebook],
    ) -> Result<Option<Floor1Curve>, String> {
        if !br.read_flag()? {
            return Ok(None);
        }
        let range = match self.multiplier {
            1 => 256i32,
            2 => 128,
            3 => 86,
            4 => 64,
            _ => return Err("vorbis: bad floor1 multiplier".into()),
        };
        let mut final_y = vec![0i32; self.x_list.len()];
        let mut step2_flag = vec![false; self.x_list.len()];
        let q_bits = ilog((range - 1) as u32);
        final_y[0] = br.read_bits(q_bits)? as i32;
        final_y[1] = br.read_bits(q_bits)? as i32;
        step2_flag[0] = true;
        step2_flag[1] = true;

        let mut offset = 2usize;
        for &c_idx in &self.partition_class_list {
            let class = &self.classes[c_idx];
            let cdim = class.dimensions;
            let cbits = class.subclasses;
            let csub = (1u32 << cbits) - 1;
            let mut cval = 0u32;
            if cbits > 0 {
                let book = class.masterbook.ok_or("vorbis: missing masterbook")?;
                cval = codebooks[book].decode_scalar(br)? as u32;
            }
            for _ in 0..cdim {
                let sub = (cval & csub) as usize;
                cval >>= cbits;
                if let Some(book) = class.subclass_books[sub] {
                    final_y[offset] = codebooks[book].decode_scalar(br)? as i32;
                } else {
                    final_y[offset] = 0;
                }
                offset += 1;
            }
        }

        for i in 2..self.x_list.len() {
            let (lo, hi) = self.neighbors[i];
            let predicted = render_point(
                self.x_list[lo],
                self.x_list[hi],
                final_y[lo],
                final_y[hi],
                self.x_list[i],
            );
            let val = final_y[i];
            let highroom = range - predicted;
            let lowroom = predicted;
            let room = if highroom < lowroom {
                highroom * 2
            } else {
                lowroom * 2
            };
            if val != 0 {
                step2_flag[lo] = true;
                step2_flag[hi] = true;
                step2_flag[i] = true;
                final_y[i] = if val >= room {
                    if highroom > lowroom {
                        val - lowroom + predicted
                    } else {
                        predicted - val + highroom - 1
                    }
                } else if val & 1 != 0 {
                    predicted - ((val + 1) >> 1)
                } else {
                    predicted + (val >> 1)
                };
            } else {
                step2_flag[i] = false;
                final_y[i] = predicted;
            }
        }
        let _ = self.rangebits;
        Ok(Some(Floor1Curve {
            final_y,
            step2_flag,
        }))
    }

    fn synthesize(&self, curve: &Floor1Curve, out: &mut [f32]) {
        let Floor1Curve {
            final_y,
            step2_flag,
        } = curve;
        let n = out.len();
        let mut hx = 0i32;
        let mut lx = 0i32;
        let mut hy = 0i32;
        let mut ly = final_y[0] * self.multiplier;
        for &idx in &self.sort_order[1..] {
            if !step2_flag[idx] {
                continue;
            }
            hx = self.x_list[idx];
            hy = final_y[idx] * self.multiplier;
            if hx > lx {
                render_line(lx, ly, hx, hy, out);
                lx = hx;
                ly = hy;
            }
        }
        if (hx as usize) < n {
            let gain = FLOOR1_INVERSE_DB_TABLE[hy.clamp(0, 255) as usize] as f32;
            for s in &mut out[hx as usize..n] {
                *s *= gain;
            }
        }
    }
}

#[inline]
fn render_point(x0: i32, x1: i32, y0: i32, y1: i32, x: i32) -> i32 {
    let dy = y1 - y0;
    let adx = x1 - x0;
    let ady = dy.abs();
    let err = ady * (x - x0);
    let off = err / adx;
    if dy < 0 {
        y0 - off
    } else {
        y0 + off
    }
}

fn render_line(x0: i32, y0: i32, x1: i32, y1: i32, out: &mut [f32]) {
    let dy = y1 - y0;
    let adx = x1 - x0;
    if adx <= 0 {
        return;
    }
    let ady = dy.abs();
    let base = dy / adx;
    let sy = if dy < 0 { base - 1 } else { base + 1 };
    let ady_rem = ady - base.abs() * adx;
    let mut err = 0i32;
    let mut y = y0;
    let end = (x1 as usize).min(out.len());
    for (x, sample) in out.iter_mut().enumerate().take(end).skip(x0 as usize) {
        let _ = x;
        *sample *= FLOOR1_INVERSE_DB_TABLE[y.clamp(0, 255) as usize] as f32;
        err += ady_rem;
        if err >= adx {
            err -= adx;
            y += sy;
        } else {
            y += base;
        }
    }
}

struct Residue {
    kind: u16,
    begin: usize,
    end: usize,
    partition_size: usize,
    classifications: usize,
    classbook: usize,
    books: Vec<[Option<usize>; 8]>,
}

impl Residue {
    fn parse(br: &mut BitReader<'_>, codebook_count: usize) -> Result<Self, String> {
        let kind = br.read_bits(16)? as u16;
        if kind > 2 {
            return Err(format!("vorbis: bad residue kind {kind}"));
        }
        let begin = br.read_bits(24)? as usize;
        let end = br.read_bits(24)? as usize;
        let partition_size = br.read_bits(24)? as usize + 1;
        let classifications = br.read_bits(6)? as usize + 1;
        let classbook = br.read_bits(8)? as usize;
        if classbook >= codebook_count {
            return Err("vorbis: residue classbook OOB".into());
        }
        let mut cascades = Vec::with_capacity(classifications);
        for _ in 0..classifications {
            let low = br.read_bits(3)?;
            let high = if br.read_flag()? {
                br.read_bits(5)? << 3
            } else {
                0
            };
            cascades.push(high | low);
        }
        let mut books = Vec::with_capacity(classifications);
        for c in cascades {
            let mut row = [None; 8];
            for (b, slot) in row.iter_mut().enumerate() {
                if (c & (1 << b)) != 0 {
                    let book = br.read_bits(8)? as usize;
                    if book >= codebook_count {
                        return Err("vorbis: residue pass book OOB".into());
                    }
                    *slot = Some(book);
                }
            }
            books.push(row);
        }
        Ok(Self {
            kind,
            begin,
            end,
            partition_size,
            classifications,
            classbook,
            books,
        })
    }

    fn decode(
        &self,
        br: &mut BitReader<'_>,
        codebooks: &[Codebook],
        vectors: &mut [Vec<f32>],
        do_not_decode: &[bool],
        half_n: usize,
    ) -> Result<(), String> {
        let ch = vectors.len();
        if ch == 0 {
            return Ok(());
        }
        if self.kind == 2 {
            if do_not_decode.iter().all(|&skip| skip) {
                return Ok(());
            }
            let total = half_n * ch;
            let limit_begin = self.begin.min(total);
            let limit_end = self.end.min(total);
            if limit_end <= limit_begin {
                return Ok(());
            }
            let mut flat = vec![0.0f32; total];
            self.decode_01(
                br,
                codebooks,
                std::slice::from_mut(&mut flat),
                &[false],
                1,
                limit_begin,
                limit_end,
            )?;
            for (i, val) in flat.into_iter().enumerate() {
                vectors[i % ch][i / ch] += val;
            }
            return Ok(());
        }
        let limit_begin = self.begin.min(half_n);
        let limit_end = self.end.min(half_n);
        if limit_end <= limit_begin {
            return Ok(());
        }
        self.decode_01(
            br,
            codebooks,
            vectors,
            do_not_decode,
            self.kind,
            limit_begin,
            limit_end,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_01(
        &self,
        br: &mut BitReader<'_>,
        codebooks: &[Codebook],
        vectors: &mut [Vec<f32>],
        do_not_decode: &[bool],
        effective_kind: u16,
        limit_begin: usize,
        limit_end: usize,
    ) -> Result<(), String> {
        let ch = vectors.len();
        let n_to_read = limit_end - limit_begin;
        let parts_to_read = n_to_read / self.partition_size;
        let class_dim = codebooks[self.classbook].dimensions.max(1);
        let mut classwords = vec![vec![0usize; parts_to_read + class_dim]; ch];

        for pass in 0..8 {
            let mut part = 0usize;
            while part < parts_to_read {
                if pass == 0 {
                    for (c_idx, skip) in do_not_decode.iter().enumerate() {
                        if *skip {
                            continue;
                        }
                        let mut temp = codebooks[self.classbook].decode_scalar(br)?;
                        for k in (0..class_dim).rev() {
                            if part + k < parts_to_read {
                                classwords[c_idx][part + k] = temp % self.classifications;
                            }
                            temp /= self.classifications;
                        }
                    }
                }
                for _ in 0..class_dim {
                    if part >= parts_to_read {
                        break;
                    }
                    for c_idx in 0..ch {
                        if do_not_decode[c_idx] {
                            continue;
                        }
                        let vqclass = classwords[c_idx][part];
                        if let Some(book_idx) = self.books[vqclass][pass] {
                            let book = &codebooks[book_idx];
                            let offset = limit_begin + part * self.partition_size;
                            let dim = book.dimensions.max(1);
                            if effective_kind == 0 {
                                let step = self.partition_size / dim;
                                for i in 0..step {
                                    let idx = book.decode_scalar(br)?;
                                    let entry = book.vq_entry(idx);
                                    for (d, &v) in entry.iter().enumerate() {
                                        let pos = offset + i + d * step;
                                        if pos < limit_end {
                                            vectors[c_idx][pos] += v;
                                        }
                                    }
                                }
                            } else {
                                let mut i = 0usize;
                                while i < self.partition_size {
                                    let idx = book.decode_scalar(br)?;
                                    let entry = book.vq_entry(idx);
                                    for &v in entry {
                                        let pos = offset + i;
                                        if pos < limit_end {
                                            vectors[c_idx][pos] += v;
                                        }
                                        i += 1;
                                    }
                                }
                            }
                        }
                    }
                    part += 1;
                }
            }
        }
        Ok(())
    }
}

struct Submap {
    floor: usize,
    residue: usize,
}

struct Mapping {
    coupling: Vec<(usize, usize)>,
    mux: Vec<usize>,
    submaps: Vec<Submap>,
}

impl Mapping {
    fn parse(
        br: &mut BitReader<'_>,
        channels: usize,
        floor_count: usize,
        residue_count: usize,
    ) -> Result<Self, String> {
        let kind = br.read_bits(16)?;
        if kind != 0 {
            return Err(format!("vorbis: unsupported mapping kind {kind}"));
        }
        let submaps_count = if br.read_flag()? {
            br.read_bits(4)? as usize + 1
        } else {
            1
        };
        let coupling_steps = if br.read_flag()? {
            br.read_bits(8)? as usize + 1
        } else {
            0
        };
        let ch_bits = ilog((channels.saturating_sub(1)) as u32);
        let mut coupling = Vec::with_capacity(coupling_steps);
        for _ in 0..coupling_steps {
            let mag = br.read_bits(ch_bits)? as usize;
            let ang = br.read_bits(ch_bits)? as usize;
            if mag >= channels || ang >= channels || mag == ang {
                return Err("vorbis: invalid channel coupling pair".into());
            }
            coupling.push((mag, ang));
        }
        if br.read_bits(2)? != 0 {
            return Err("vorbis: bad mapping reserved bits".into());
        }
        let mut mux = vec![0usize; channels];
        if submaps_count > 1 {
            for m in &mut mux {
                *m = br.read_bits(4)? as usize;
                if *m >= submaps_count {
                    return Err("vorbis: mapping mux OOB".into());
                }
            }
        }
        let mut submaps = Vec::with_capacity(submaps_count);
        for _ in 0..submaps_count {
            let _ = br.read_bits(8)?;
            let floor = br.read_bits(8)? as usize;
            let residue = br.read_bits(8)? as usize;
            if floor >= floor_count || residue >= residue_count {
                return Err("vorbis: mapping submap OOB".into());
            }
            submaps.push(Submap { floor, residue });
        }
        Ok(Self {
            coupling,
            mux,
            submaps,
        })
    }
}

struct Mode {
    blockflag: bool,
    mapping: usize,
}

/// Decodes an FMOD FSB5 headerless Vorbis sample into interleaved `[L, R]` or mono-upmixed stereo `f32` PCM (`[-1.0, 1.0]`).
pub fn decode_fsb5_vorbis_to_stereo(
    data: &[u8],
    channels: usize,
    sample_rate: u32,
    expected_frames: usize,
    setup_header: &[u8],
) -> Result<Vec<[f32; 2]>, String> {
    if channels == 0 || channels > 2 {
        return Err(format!("vorbis: unsupported channel count {channels}"));
    }
    let _ = sample_rate;
    let mut dec = VorbisDecoder::from_setup_header(channels, 256, 2048, setup_header)?;
    let mut out = Vec::with_capacity(expected_frames.max(1024));
    let mut pos = 0usize;
    while pos + 2 <= data.len() {
        let pkt_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pkt_len == 0 {
            break;
        }
        let end = pos + pkt_len;
        if end > data.len() {
            break;
        }
        dec.decode_packet(&data[pos..end], &mut out)?;
        pos = end;
    }
    if expected_frames > 0 && out.len() > expected_frames {
        out.truncate(expected_frames);
    }
    Ok(out)
}

pub struct VorbisDecoder {
    channels: usize,
    blocksize_0: usize,
    blocksize_1: usize,
    codebooks: Vec<Codebook>,
    floors: Vec<Floor1>,
    residues: Vec<Residue>,
    mappings: Vec<Mapping>,
    modes: Vec<Mode>,
    prev_window: Option<Vec<Vec<f32>>>,
    win_short: Vec<f32>,
    win_long: Vec<f32>,
    imdct_short: ImdctPlan,
    imdct_long: ImdctPlan,
}

impl VorbisDecoder {
    pub fn from_setup_header(
        channels: usize,
        blocksize_0: usize,
        blocksize_1: usize,
        setup_packet: &[u8],
    ) -> Result<Self, String> {
        if setup_packet.get(..7) != Some(b"\x05vorbis") {
            return Err("vorbis: missing 0x05vorbis setup header".into());
        }
        let mut br = BitReader::new(&setup_packet[7..]);
        let codebook_count = br.read_bits(8)? as usize + 1;
        let mut codebooks = Vec::with_capacity(codebook_count);
        for _ in 0..codebook_count {
            codebooks.push(Codebook::parse(&mut br)?);
        }
        let time_count = br.read_bits(6)? as usize + 1;
        for _ in 0..time_count {
            if br.read_bits(16)? != 0 {
                return Err("vorbis: non-zero time transform".into());
            }
        }
        let floor_count = br.read_bits(6)? as usize + 1;
        let mut floors = Vec::with_capacity(floor_count);
        for _ in 0..floor_count {
            let ftype = br.read_bits(16)?;
            if ftype != 1 {
                return Err(format!("vorbis: unsupported floor type {ftype}"));
            }
            floors.push(Floor1::parse(&mut br, codebook_count)?);
        }
        let residue_count = br.read_bits(6)? as usize + 1;
        let mut residues = Vec::with_capacity(residue_count);
        for _ in 0..residue_count {
            residues.push(Residue::parse(&mut br, codebook_count)?);
        }
        let mapping_count = br.read_bits(6)? as usize + 1;
        let mut mappings = Vec::with_capacity(mapping_count);
        for _ in 0..mapping_count {
            mappings.push(Mapping::parse(
                &mut br,
                channels,
                floor_count,
                residue_count,
            )?);
        }
        let mode_count = br.read_bits(6)? as usize + 1;
        let mut modes = Vec::with_capacity(mode_count);
        for _ in 0..mode_count {
            let blockflag = br.read_flag()?;
            let _windowtype = br.read_bits(16)?;
            let _transformtype = br.read_bits(16)?;
            let mapping = br.read_bits(8)? as usize;
            if mapping >= mapping_count {
                return Err("vorbis: mode mapping OOB".into());
            }
            modes.push(Mode { blockflag, mapping });
        }
        if !br.read_flag()? {
            return Err("vorbis: setup framing bit missing".into());
        }

        Ok(Self {
            channels,
            blocksize_0,
            blocksize_1,
            codebooks,
            floors,
            residues,
            mappings,
            modes,
            prev_window: None,
            win_short: vorbis_half_window(blocksize_0 / 2),
            win_long: vorbis_half_window(blocksize_1 / 2),
            imdct_short: ImdctPlan::new(blocksize_0),
            imdct_long: ImdctPlan::new(blocksize_1),
        })
    }

    pub fn decode_packet(
        &mut self,
        packet: &[u8],
        out_stereo: &mut Vec<[f32; 2]>,
    ) -> Result<(), String> {
        let mut br = BitReader::new(packet);
        if br.read_flag()? {
            return Err("vorbis: audio packet type bit nonzero".into());
        }
        let mode_bits = ilog((self.modes.len().saturating_sub(1)) as u32);
        let mode_idx = br.read_bits(mode_bits)? as usize;
        let mode = self.modes.get(mode_idx).ok_or("vorbis: bad mode index")?;
        let n = if mode.blockflag {
            self.blocksize_1
        } else {
            self.blocksize_0
        };
        let half_n = n / 2;
        let (prev_win_flag, next_win_flag) = if mode.blockflag {
            (br.read_flag()?, br.read_flag()?)
        } else {
            (false, false)
        };
        let mapping = &self.mappings[mode.mapping];

        let mut floor_curves: Vec<Option<Floor1Curve>> = Vec::with_capacity(self.channels);
        let mut no_residue = vec![false; self.channels];
        for (ch, no_res) in no_residue.iter_mut().enumerate() {
            let submap_idx = mapping.mux[ch];
            let floor_idx = mapping.submaps[submap_idx].floor;
            let curve = self.floors[floor_idx].decode_curve(&mut br, &self.codebooks)?;
            *no_res = curve.is_none();
            floor_curves.push(curve);
        }
        for &(mag, ang) in &mapping.coupling {
            if !no_residue[mag] || !no_residue[ang] {
                no_residue[mag] = false;
                no_residue[ang] = false;
            }
        }

        let mut spec = vec![vec![0.0f32; half_n]; self.channels];
        for (submap_idx, submap) in mapping.submaps.iter().enumerate() {
            let mut ch_map = Vec::new();
            let mut do_not_decode = Vec::new();
            let mut sub_vecs = Vec::new();
            for ch in 0..self.channels {
                if mapping.mux[ch] == submap_idx {
                    ch_map.push(ch);
                    do_not_decode.push(no_residue[ch]);
                    sub_vecs.push(std::mem::take(&mut spec[ch]));
                }
            }
            self.residues[submap.residue].decode(
                &mut br,
                &self.codebooks,
                &mut sub_vecs,
                &do_not_decode,
                half_n,
            )?;
            for (k, ch) in ch_map.into_iter().enumerate() {
                spec[ch] = std::mem::take(&mut sub_vecs[k]);
            }
        }

        for &(mag, ang) in mapping.coupling.iter().rev() {
            let (left, right) = if mag < ang {
                let (a, b) = spec.split_at_mut(ang);
                (&mut a[mag], &mut b[0])
            } else {
                let (a, b) = spec.split_at_mut(mag);
                (&mut b[0], &mut a[ang])
            };
            for (m, a) in left.iter_mut().zip(right.iter_mut()) {
                let mv = *m;
                let av = *a;
                let (nm, na) = if mv > 0.0 {
                    if av > 0.0 {
                        (mv, mv - av)
                    } else {
                        (mv + av, mv)
                    }
                } else if av > 0.0 {
                    (mv, mv + av)
                } else {
                    (mv - av, mv)
                };
                *m = nm;
                *a = na;
            }
        }

        let mut td_blocks = vec![vec![0.0f32; n]; self.channels];
        let plan = if mode.blockflag {
            &self.imdct_long
        } else {
            &self.imdct_short
        };
        for ch in 0..self.channels {
            if let Some(curve) = &floor_curves[ch] {
                let submap_idx = mapping.mux[ch];
                let floor_idx = mapping.submaps[submap_idx].floor;
                self.floors[floor_idx].synthesize(curve, &mut spec[ch]);
                plan.inverse(&spec[ch], &mut td_blocks[ch]);
            }
        }

        let left_win_size = if mode.blockflag && !prev_win_flag {
            self.blocksize_0 / 2
        } else {
            n / 2
        };
        let left_win_start = n / 4 - left_win_size / 2;
        let left_win_end = left_win_start + left_win_size;

        let right_win_size = if mode.blockflag && !next_win_flag {
            self.blocksize_0 / 2
        } else {
            n / 2
        };
        let right_win_start = (n * 3) / 4 - right_win_size / 2;
        let right_win_end = right_win_start + right_win_size;

        let left_slope = if left_win_size == self.blocksize_0 / 2 {
            &self.win_short
        } else {
            &self.win_long
        };
        let right_slope = if right_win_size == self.blocksize_0 / 2 {
            &self.win_short
        } else {
            &self.win_long
        };

        for ch_block in &mut td_blocks {
            for (k, i) in (left_win_start..left_win_end).enumerate() {
                ch_block[i] *= left_slope[k];
            }
            for s in &mut ch_block[..left_win_start] {
                *s = 0.0;
            }
            for (k, i) in (right_win_start..right_win_end).enumerate() {
                ch_block[i] *= right_slope[right_win_size - 1 - k];
            }
            for s in &mut ch_block[right_win_end..] {
                *s = 0.0;
            }
        }

        let center = n / 2;
        if let Some(prev) = self.prev_window.take() {
            let prev_n = prev[0].len();
            let prev_center = prev_n / 2;
            let overlap_len = left_win_size.min(prev_n / 2);
            // Vorbis overlap-add between `prev[prev_center..]` and `cur[left_win_start..center]`.
            if self.channels == 1 {
                for s in &prev[0][prev_center..prev_center + (prev_n / 4 - overlap_len / 2)] {
                    out_stereo.push([*s, *s]);
                }
                let prev_ov_start = prev_center + (prev_n / 4 - overlap_len / 2);
                for k in 0..overlap_len {
                    let s = (prev[0][prev_ov_start + k] + td_blocks[0][left_win_start + k])
                        .clamp(-1.0, 1.0);
                    out_stereo.push([s, s]);
                }
                for s in &td_blocks[0][left_win_end..center] {
                    let v = s.clamp(-1.0, 1.0);
                    out_stereo.push([v, v]);
                }
            } else {
                let flat_tail = prev_n / 4 - overlap_len / 2;
                let prev_ov_start = prev_center + flat_tail;
                for k in 0..flat_tail {
                    out_stereo.push([
                        prev[0][prev_center + k].clamp(-1.0, 1.0),
                        prev[1][prev_center + k].clamp(-1.0, 1.0),
                    ]);
                }
                for k in 0..overlap_len {
                    out_stereo.push([
                        (prev[0][prev_ov_start + k] + td_blocks[0][left_win_start + k])
                            .clamp(-1.0, 1.0),
                        (prev[1][prev_ov_start + k] + td_blocks[1][left_win_start + k])
                            .clamp(-1.0, 1.0),
                    ]);
                }
                for (&l, &r) in td_blocks[0][left_win_end..center]
                    .iter()
                    .zip(&td_blocks[1][left_win_end..center])
                {
                    out_stereo.push([l.clamp(-1.0, 1.0), r.clamp(-1.0, 1.0)]);
                }
            }
        }
        self.prev_window = Some(td_blocks);
        Ok(())
    }
}

fn vorbis_half_window(half: usize) -> Vec<f32> {
    (0..half)
        .map(|i| {
            let s = (PI * (i as f64 + 0.5) / (2.0 * half as f64)).sin();
            (FRAC_PI_2 * s * s).sin() as f32
        })
        .collect()
}

struct ImdctPlan {
    n: usize,
    m: usize, // n / 4
    twiddles_pre: Vec<(f32, f32)>,
    twiddles_post: Vec<(f32, f32)>,
    fft_twiddles: Vec<(f32, f32)>,
    bit_rev: Vec<usize>,
}

impl ImdctPlan {
    fn new(n: usize) -> Self {
        let m = n / 4;
        let two_n = (2 * n) as f64;
        let twiddles_pre = (0..m)
            .map(|k| {
                let ang = PI * (4.0 * k as f64 + 1.0) / two_n;
                (ang.cos() as f32, ang.sin() as f32)
            })
            .collect();
        let twiddles_post = (0..m)
            .map(|k| {
                let ang = (2.0 * PI * k as f64) / (n as f64);
                (ang.cos() as f32, ang.sin() as f32)
            })
            .collect();
        let mut fft_twiddles = Vec::with_capacity(m / 2);
        for k in 0..m / 2 {
            let ang = 2.0 * PI * k as f64 / m as f64;
            fft_twiddles.push((ang.cos() as f32, ang.sin() as f32));
        }
        let bits = m.trailing_zeros();
        let bit_rev = (0..m)
            .map(|i| (i.reverse_bits()) >> (usize::BITS - bits))
            .collect();
        Self {
            n,
            m,
            twiddles_pre,
            twiddles_post,
            fft_twiddles,
            bit_rev,
        }
    }

    fn inverse(&self, spec: &[f32], out: &mut [f32]) {
        let n = self.n;
        let m = self.m;
        let half_n = n / 2;
        let q1 = n / 4;
        let q3 = (3 * n) / 4;
        let mut buf = vec![(0.0f32, 0.0f32); m];
        let half_m = m / 2;
        for (k, slot) in buf.iter_mut().enumerate() {
            let xr = spec[2 * k];
            let xi = -spec[half_n - 1 - 2 * k];
            let (cr, ci) = self.twiddles_pre[k];
            slot.0 = xr * cr - xi * ci;
            slot.1 = xr * ci + xi * cr;
        }
        for i in 0..m {
            let j = self.bit_rev[i];
            if i < j {
                buf.swap(i, j);
            }
        }
        let mut half_size = 1usize;
        let mut step = m / 2;
        while half_size < m {
            let size = half_size * 2;
            for group in (0..m).step_by(size) {
                for k in 0..half_size {
                    let (wr, wi) = self.fft_twiddles[k * step];
                    let (tr, ti) = buf[group + k + half_size];
                    let pr = tr * wr - ti * wi;
                    let pi_v = tr * wi + ti * wr;
                    let (ur, ui) = buf[group + k];
                    buf[group + k] = (ur + pr, ui + pi_v);
                    buf[group + k + half_size] = (ur - pr, ui - pi_v);
                }
            }
            half_size = size;
            step /= 2;
        }
        for (q, slot) in buf.iter().enumerate() {
            let (cr, ci) = self.twiddles_post[q];
            let ur = slot.0 * cr - slot.1 * ci;
            let ui = slot.0 * ci + slot.1 * cr;
            let y_even = -ui;
            let y_odd = -ur;
            out[q1 + 2 * q] = y_even;
            out[q3 - 1 - 2 * q] = y_odd;
            if q < half_m {
                out[q1 - 1 - 2 * q] = -y_even;
                out[q3 + 2 * q] = y_odd;
            } else {
                out[5 * q1 - 1 - 2 * q] = y_even;
                out[2 * q - q1] = -y_odd;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driving::fsb5_vorbis_setup::setup_header_for_crc;
    use rerbr_sim::rbr_formats::fmod;

    #[test]
    fn imdct_matches_exact_vorbis_reference_formula() {
        for n in [256usize, 2048usize] {
            let plan = ImdctPlan::new(n);
            let spec: Vec<f32> = (0..n / 2)
                .map(|k| ((k as f32 * 0.37).sin()) * 0.5 + if k == 3 { 1.0 } else { 0.0 })
                .collect();
            let mut fast = vec![0.0f32; n];
            plan.inverse(&spec, &mut fast);
            let two_n = (2 * n) as f64;
            for &sample_idx in &[
                0usize,
                1,
                7,
                63,
                64,
                n / 4,
                n / 2 - 1,
                n / 2,
                (3 * n) / 4,
                n - 1,
            ] {
                let mut ref_val = 0.0f64;
                for (k, &x) in spec.iter().enumerate() {
                    let ang = std::f64::consts::PI / two_n
                        * (2.0 * sample_idx as f64 + 1.0 + (n / 2) as f64)
                        * (2.0 * k as f64 + 1.0);
                    ref_val += f64::from(x) * ang.cos();
                }
                assert!(
                    (f64::from(fast[sample_idx]) - ref_val).abs() < 1e-3,
                    "n={n} idx={sample_idx}: fast={} ref={ref_val}",
                    fast[sample_idx]
                );
            }
        }
    }

    #[test]
    fn decodes_all_44_focus_2006_bank_samples_if_pc_present() {
        let rbr = std::path::Path::new(r"C:\Richard Burns Rally");
        if !rbr.join("AudioFMOD/Focus_Mk2_WRC.bank").is_file() {
            return;
        }
        let bank_bytes = std::fs::read(rbr.join("AudioFMOD/Focus_Mk2_WRC.bank")).unwrap();
        let str_bytes = std::fs::read(rbr.join("AudioFMOD/Focus_Mk2_WRC.strings.bank")).unwrap();
        let bank = fmod::parse(bank_bytes, &str_bytes).unwrap();
        assert_eq!(bank.samples.len(), 44);
        for (idx, s) in bank.samples.iter().enumerate() {
            let crc = s.vorbis_crc.expect("vorbis crc");
            let setup = setup_header_for_crc(crc).expect("known focus setup crc");
            let pcm = decode_fsb5_vorbis_to_stereo(
                bank.sample_data(s),
                s.channels as usize,
                s.rate,
                s.frames as usize,
                setup,
            )
            .unwrap_or_else(|e| panic!("sample {idx} ({}) failed: {e}", s.name));
            let peak = pcm
                .iter()
                .map(|f| f[0].abs().max(f[1].abs()))
                .fold(0.0f32, f32::max);
            assert!(!pcm.is_empty(), "empty decoded pcm for {}", s.name);
            assert!(
                pcm.iter().all(|f| f[0].is_finite() && f[1].is_finite()),
                "nonfinite sample in {}",
                s.name
            );
            assert!(peak > 0.001 && peak <= 1.0, "bad peak {peak} in {}", s.name);
            if idx < 8 {
                eprintln!(
                    "DECODED FOCUS SAMPLE[{idx:02}] {:<24} ch={} rate={} frames={}/{} peak={:.3}",
                    s.name,
                    s.channels,
                    s.rate,
                    pcm.len(),
                    s.frames,
                    peak
                );
            }
        }
    }
}
