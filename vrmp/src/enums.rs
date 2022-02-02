use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Projection {
    Er360,
    Er180,
    Fisheye,
    Eac,
    Flat,
}

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Mono,
    LeftRight,
    RightLeft,
    TopBottom,
    BottomTop,
}

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AspectRatio {
    Half,
    One,
    Two,
}
