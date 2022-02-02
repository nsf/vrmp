fn hex_digit(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => 10 + (b - b'a'),
        _ => 0,
    }
}

fn parse_color(b: &[u8]) -> f32 {
    let a = hex_digit(b[0].to_ascii_lowercase());
    let b = hex_digit(b[1].to_ascii_lowercase());
    (a * 16 + b) as f32 / 255.0
}

pub fn hex(s: &str) -> [f32; 4] {
    let mut b = s.as_bytes();
    if b[0] == b'#' {
        b = &b[1..];
    }
    let r = parse_color(&b[0..2]);
    let g = parse_color(&b[2..4]);
    let b = parse_color(&b[4..6]);

    [r, g, b, 1.0]
}

pub fn iter_bit_spans<F: FnMut(u8, u8)>(seen0: u64, seen1: u64, mut f: F) {
    let mut beg_x = 255;
    let mut end_x = 255;
    for i in 0..128 {
        let bit = {
            if i < 64 {
                ((seen0 >> i) & 1) == 1
            } else {
                ((seen1 >> (i - 64)) & 1) == 1
            }
        };
        if bit {
            if beg_x == 255 {
                beg_x = i;
                end_x = i;
            } else {
                end_x = i;
            }
        } else if beg_x != 255 {
            f(beg_x, end_x + 1);
            beg_x = 255;
        }
    }
    if beg_x != 255 {
        f(beg_x, end_x + 1);
    }
}
