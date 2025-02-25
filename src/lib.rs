#![allow(clippy::get_first)]
#![allow(clippy::int_plus_one)]
#![allow(clippy::len_zero)]
#![cfg_attr(feature = "bench", feature(test))]

use std::f32;
use std::str::{self, FromStr};

const NONE: f32 = 0_f32;

#[doc(hidden)]
pub type Rgba = Srgb;

/// A color in the sRGB color space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Srgb {
    /// The red component.
    pub red: f32,
    /// The green component.
    pub green: f32,
    /// The blue component.
    pub blue: f32,
    /// The alpha component.
    pub alpha: f32,
}

impl Srgb {
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Srgb {
        Srgb {
            red,
            green,
            blue,
            alpha,
        }
    }

    fn from_rgb8(red: u8, green: u8, blue: u8) -> Srgb {
        Srgb::from_rgba8(red, green, blue, 255)
    }

    fn from_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Srgb {
        Srgb {
            red: red as f32 / 255.,
            green: green as f32 / 255.,
            blue: blue as f32 / 255.,
            alpha: alpha as f32 / 255.,
        }
    }
}

#[derive(Debug, Error)]
#[cfg_attr(feature = "miette", derive(miette::Diagnostic))]
#[error("CSS color parsing error")]
pub struct ParseColorError {
    #[cfg_attr(feature = "miette", label("{inner_error}{}", if *expected_none {
        " or `none`"
    } else {
        ""
    }))]
    pub span: std::ops::Range<usize>,
    pub inner_error: ParseColorErrorInner,
    pub expected_none: bool,
}

#[derive(Debug, Error)]
pub enum ParseColorErrorInner {
    #[error("Expected `{expected}`")]
    UnexpectedByte { expected: u8 },
    #[error("Expected `{}`", String::from_utf8_lossy(expected))]
    UnexpectedName { expected: &'static [u8] },
    #[error("Expected one of: {expected:?}")]
    UnexpectedOneOfName { expected: &'static [&'static str] },
    #[error("Expected {expected}")]
    UnexpectedElement { expected: &'static str },
}

impl FromStr for Srgb {
    type Err = ParseColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_css_color(s.as_bytes().into())
    }
}

// https://www.w3.org/TR/css-color-4/
fn parse_css_color(mut input: CssInput<'_>) -> Result<Srgb, ParseColorError> {
    if input.consume_byte(b'#').is_ok() {
        input.parse_hex()
    } else if input.consume_function(b"rgb").is_ok() {
        input.parse_rgb()
    } else if input.consume_function(b"rgba").is_ok() {
        input.parse_rgb()
    } else if input.consume_function(b"hsl").is_ok() {
        input.parse_hsl()
    } else if input.consume_function(b"hsla").is_ok() {
        input.parse_hsl()
    } else if input.consume_function(b"hwb").is_ok() {
        input.parse_hwb()
    } else if let Ok(color) = input.parse_named() {
        Ok(color)
    } else {
        Err(ParseColorError {
            span: input.span(0..input.len()),
            inner_error: ParseColorErrorInner::UnexpectedOneOfName {
                expected: &["rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "a CSS color name"],
            },
            expected_none: false,
        })
    }
}

fn clamp_unit_f32(value: f32) -> f32 {
    value.max(0.).min(1.)
}

fn normalize_hue(value: f32) -> f32 {
    value - value.floor()
}

struct Hsla {
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
    pub alpha: f32,
}

// https://www.w3.org/TR/css-color-4/#hsl-to-rgb
impl From<Hsla> for Srgb {
    fn from(hsla: Hsla) -> Self {
        let t2 = if hsla.lightness <= 0.5 {
            hsla.lightness * (hsla.saturation + 1.)
        } else {
            hsla.lightness + hsla.saturation - hsla.lightness * hsla.saturation
        };
        let t1 = hsla.lightness * 2. - t2;

        let hue_to_rgb = |h6: f32| -> f32 {
            if h6 < 1. {
                (t2 - t1) * h6 + t1
            } else if h6 < 3. {
                t2
            } else if h6 < 4. {
                (t2 - t1) * (4. - h6) + t1
            } else {
                t1
            }
        };
        let h6 = hsla.hue * 6.;
        let h6_red = if h6 + 2. < 6. { h6 + 2. } else { h6 - 4. };
        let h6_blue = if h6 - 2. >= 0. { h6 - 2. } else { h6 + 4. };
        Srgb {
            red: hue_to_rgb(h6_red),
            green: hue_to_rgb(h6),
            blue: hue_to_rgb(h6_blue),
            alpha: hsla.alpha,
        }
    }
}

struct Hwba {
    pub hue: f32,
    pub whiteness: f32,
    pub blackness: f32,
    pub alpha: f32,
}

// https://www.w3.org/TR/css-color-4/#hwb-to-rgb
impl From<Hwba> for Srgb {
    fn from(hwba: Hwba) -> Self {
        // If the sum of these two arguments is greater than 100%, then at computed-value time they
        // are further normalized to add up to 100%, with the same relative ratio.
        if hwba.whiteness + hwba.blackness >= 1. {
            let gray = hwba.whiteness / (hwba.whiteness + hwba.blackness);
            Srgb {
                red: gray,
                green: gray,
                blue: gray,
                alpha: hwba.alpha,
            }
        } else {
            fn hue_to_rgb(h6: f32) -> f32 {
                if h6 < 1. {
                    h6
                } else if h6 < 3. {
                    1.
                } else if h6 < 4. {
                    4. - h6
                } else {
                    0.
                }
            }
            let h6 = hwba.hue * 6.;
            let h6_red = if h6 + 2. < 6. { h6 + 2. } else { h6 - 4. };
            let h6_blue = if h6 - 2. >= 0. { h6 - 2. } else { h6 + 4. };
            let x = 1. - hwba.whiteness - hwba.blackness;
            Srgb {
                red: hue_to_rgb(h6_red) * x + hwba.whiteness,
                green: hue_to_rgb(h6) * x + hwba.whiteness,
                blue: hue_to_rgb(h6_blue) * x + hwba.whiteness,
                alpha: hwba.alpha,
            }
        }
    }
}

macro_rules! rgb {
    ($red: expr, $green: expr, $blue: expr) => {
        Srgb::from_rgb8($red, $green, $blue)
    };
}

pub struct CssInput<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> From<&'a [u8]> for CssInput<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self {
            input: value,
            offset: 0,
        }
    }
}

impl CssInput<'_> {
    fn consume_byte(&mut self, b: u8) -> Result<(), ParseColorError> {
        match self.get(0) {
            Some(c) if c == b => {
                self.offset += 1;
                Ok(())
            }
            _ => Err(ParseColorError {
                span: self.span(0..1),
                inner_error: ParseColorErrorInner::UnexpectedByte { expected: b },
                expected_none: false,
            }),
        }
    }

    /// Consumes a function-token matching the given identifier.
    ///
    /// Any whitespace following the function-token is also consumed.
    fn consume_function(&mut self, name: &'static [u8]) -> Result<(), ParseColorError> {
        debug_assert!(is_ident_start(name));

        let n = name.len();
        if self.len() >= n + 1
            && self.offset_input()[..n].eq_ignore_ascii_case(name)
            && self.offset_input()[n] == b'('
        {
            self.offset += n + 1;
            self.skip_ws();
            Ok(())
        } else {
            Err(ParseColorError {
                span: self.span(0..name.len()),
                inner_error: ParseColorErrorInner::UnexpectedName { expected: name },
                expected_none: false,
            })
        }
    }

    #[inline]
    fn consume_name(&mut self, name: &'static [u8]) -> Result<(), ParseColorError> {
        debug_assert!(is_ident_start(name));

        let n = name.len();
        if self.len() >= n
            && self.offset_input()[..n].eq_ignore_ascii_case(name)
            && self.get(n).filter(|c| is_name(*c)).is_none()
        {
            self.offset += n;
            Ok(())
        } else {
            Err(ParseColorError {
                span: self.span(0..name.len()),
                inner_error: ParseColorErrorInner::UnexpectedName { expected: name },
                expected_none: false,
            })
        }
    }

    #[inline]
    fn consume_none(
        &mut self,
        mut err_if_not_none: ParseColorError,
    ) -> Result<(), ParseColorError> {
        self.consume_name(b"none").map_err(|_| {
            err_if_not_none.expected_none = true;
            err_if_not_none
        })
    }

    fn consume_number(&mut self) -> Result<(), ParseColorError> {
        self.skip_sign();
        match self.get(0) {
            Some(b'.') => {}
            _ => {
                self.consume_digits()?;
            }
        }
        if let Some(b'.') = self.get(0) {
            self.offset += 1;
            self.consume_digits()?;
        }
        match self.get(0) {
            Some(b'E') | Some(b'e') => {
                self.offset += 1;
                self.skip_sign();
                self.consume_digits()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn consume_digits(&mut self) -> Result<(), ParseColorError> {
        match self.get(0).map(|c| digit(c)) {
            Some(Ok(_)) => {
                while let Some(Ok(_)) = self.get(0).map(|c| digit(c)) {
                    self.offset += 1;
                }
                Ok(())
            }
            _ => Err(ParseColorError {
                span: self.span(0..1),
                inner_error: ParseColorErrorInner::UnexpectedElement {
                    expected: "a digit",
                },
                expected_none: false,
            }),
        }
    }

    fn parse_number(&mut self) -> Result<f32, ParseColorError> {
        let start = self.offset;
        self.consume_number()?;
        Ok(str::from_utf8(&self.input[start..self.offset])
            .unwrap()
            .parse()
            .or(Err(ParseColorError {
                span: start..self.offset,
                inner_error: ParseColorErrorInner::UnexpectedElement {
                    expected: "a valid floating point number",
                },
                expected_none: false,
            }))?)
    }

    // <percentage> = <number> %
    fn parse_percentage(&mut self) -> Result<f32, ParseColorError> {
        let value = self.parse_number()?;
        self.consume_byte(b'%')?;
        Ok(value / 100.)
    }

    fn parse_number_or_percentage(&mut self) -> Result<(NumberOrPercentage, f32), ParseColorError> {
        let value = self.parse_number()?;

        if self.consume_byte(b'%').is_ok() {
            Ok((Percentage, value / 100.))
        } else {
            Ok((Number, value))
        }
    }

    // <alpha-value> = <number> | <percentage>
    fn parse_alpha_value(&mut self) -> Result<f32, ParseColorError> {
        let (_, alpha) = self.parse_number_or_percentage()?;
        Ok(clamp_unit_f32(alpha))
    }

    // <hue> = <number> | <angle>
    fn parse_hue(&mut self) -> Result<f32, ParseColorError> {
        let value = self.parse_number()?;

        if !is_ident_start(self.offset_input()) {
            Ok(value / 360.)
        } else if self.consume_name(b"deg").is_ok() {
            Ok(value / 360.)
        } else if self.consume_name(b"grad").is_ok() {
            Ok(value / 400.)
        } else if self.consume_name(b"rad").is_ok() {
            Ok(value / (2. * f32::consts::PI))
        } else if self.consume_name(b"turn").is_ok() {
            Ok(value)
        } else {
            Err(ParseColorError {
                span: self.span(0..4),
                inner_error: ParseColorErrorInner::UnexpectedOneOfName {
                    expected: &["deg", "grad", "rad", "turn"],
                },
                expected_none: false,
            })
        }
    }

    /// Parse sRGB hex colors.
    fn parse_hex(&self) -> Result<Srgb, ParseColorError> {
        match self.len() {
            8 => Ok(Srgb::from_rgba8(
                hex_digit(self.offset_input()[0]).or(Err(ParseColorError {
                    span: self.span(0..1),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 16
                    + hex_digit(self.offset_input()[1]).or(Err(ParseColorError {
                        span: self.span(1..2),
                        inner_error: ParseColorErrorInner::UnexpectedElement {
                            expected: "a hexadecimal digit ([0-9A-Fa-f])",
                        },
                        expected_none: false,
                    }))?,
                hex_digit(self.offset_input()[2]).or(Err(ParseColorError {
                    span: self.span(2..3),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 16
                    + hex_digit(self.offset_input()[3]).or(Err(ParseColorError {
                        span: self.span(3..4),
                        inner_error: ParseColorErrorInner::UnexpectedElement {
                            expected: "a hexadecimal digit ([0-9A-Fa-f])",
                        },
                        expected_none: false,
                    }))?,
                hex_digit(self.offset_input()[4]).or(Err(ParseColorError {
                    span: self.span(4..5),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 16
                    + hex_digit(self.offset_input()[5]).or(Err(ParseColorError {
                        span: self.span(5..6),
                        inner_error: ParseColorErrorInner::UnexpectedElement {
                            expected: "a hexadecimal digit ([0-9A-Fa-f])",
                        },
                        expected_none: false,
                    }))?,
                hex_digit(self.offset_input()[6]).or(Err(ParseColorError {
                    span: self.span(6..7),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 16
                    + hex_digit(self.offset_input()[7]).or(Err(ParseColorError {
                        span: self.span(7..8),
                        inner_error: ParseColorErrorInner::UnexpectedElement {
                            expected: "a hexadecimal digit ([0-9A-Fa-f])",
                        },
                        expected_none: false,
                    }))?,
            )),
            6 => Ok(Srgb::from_rgb8(
                hex_digit(self.offset_input()[0]).or(Err(ParseColorError {
                    span: self.span(0..1),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 16
                    + hex_digit(self.offset_input()[1]).or(Err(ParseColorError {
                        span: self.span(1..2),
                        inner_error: ParseColorErrorInner::UnexpectedElement {
                            expected: "a hexadecimal digit ([0-9A-Fa-f])",
                        },
                        expected_none: false,
                    }))?,
                hex_digit(self.offset_input()[2]).or(Err(ParseColorError {
                    span: self.span(2..3),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 16
                    + hex_digit(self.offset_input()[3]).or(Err(ParseColorError {
                        span: self.span(3..4),
                        inner_error: ParseColorErrorInner::UnexpectedElement {
                            expected: "a hexadecimal digit ([0-9A-Fa-f])",
                        },
                        expected_none: false,
                    }))?,
                hex_digit(self.offset_input()[4]).or(Err(ParseColorError {
                    span: self.span(4..5),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 16
                    + hex_digit(self.offset_input()[5]).or(Err(ParseColorError {
                        span: self.span(5..6),
                        inner_error: ParseColorErrorInner::UnexpectedElement {
                            expected: "a hexadecimal digit ([0-9A-Fa-f])",
                        },
                        expected_none: false,
                    }))?,
            )),
            4 => Ok(Srgb::from_rgba8(
                hex_digit(self.offset_input()[0]).or(Err(ParseColorError {
                    span: self.span(0..1),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 17,
                hex_digit(self.offset_input()[1]).or(Err(ParseColorError {
                    span: self.span(1..2),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 17,
                hex_digit(self.offset_input()[2]).or(Err(ParseColorError {
                    span: self.span(2..3),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 17,
                hex_digit(self.offset_input()[3]).or(Err(ParseColorError {
                    span: self.span(3..4),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 17,
            )),
            3 => Ok(Srgb::from_rgb8(
                hex_digit(self.offset_input()[0]).or(Err(ParseColorError {
                    span: self.span(0..1),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 17,
                hex_digit(self.offset_input()[1]).or(Err(ParseColorError {
                    span: self.span(1..2),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 17,
                hex_digit(self.offset_input()[2]).or(Err(ParseColorError {
                    span: self.span(2..3),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a hexadecimal digit ([0-9A-Fa-f])",
                    },
                    expected_none: false,
                }))? * 17,
            )),
            _ => Err(ParseColorError {
                span: self.span(0..self.len()),
                inner_error: ParseColorErrorInner::UnexpectedElement {
                    expected: "a hex color code of length 8, 6, 4, or 3",
                },
                expected_none: false,
            }),
        }
    }

    // hsl()  = [ <legacy-hsl-syntax>  | <modern-hsl-syntax>  ]
    // hsla() = [ <legacy-hsla-syntax> | <modern-hsla-syntax> ]
    // <legacy-hsl-syntax>  = hsl(  <hue>, <percentage>, <percentage>, <alpha-value>? )
    // <legacy-hsla-syntax> = hsla( <hue>, <percentage>, <percentage>, <alpha-value>? )
    // <modern-hsl-syntax>  = hsl(  [<hue> | none]
    //                              [<percentage> | <number> | none]
    //                              [<percentage> | <number> | none]
    //                              [ / [<alpha-value> | none] ]? )
    // <modern-hsla-syntax> = hsla( [<hue> | none]
    //                              [<percentage> | <number> | none]
    //                              [<percentage> | <number> | none]
    //                              [ / [<alpha-value> | none] ]? )
    fn parse_hsl(&mut self) -> Result<Srgb, ParseColorError> {
        let (hue, legacy_syntax) = match self.parse_hue() {
            Ok(hue) => {
                self.skip_ws();
                match self.get(0) {
                    Some(b',') => {
                        self.offset += 1;
                        self.skip_ws();
                        (hue, true)
                    }
                    _ => (hue, false),
                }
            }
            Err(e) => {
                self.consume_none(e)?;
                self.skip_ws();
                (NONE, false)
            }
        };

        let (saturation, lightness) = if legacy_syntax {
            let saturation = self.parse_percentage()?;
            self.skip_ws();
            self.consume_byte(b',')?;
            self.skip_ws();
            let lightness = self.parse_percentage()?;
            self.skip_ws();
            (saturation, lightness)
        } else {
            let saturation = match self.parse_number_or_percentage() {
                Ok(saturation) => {
                    self.skip_ws();
                    saturation.frac(100.)
                }
                Err(e) => {
                    self.consume_none(e)?;
                    self.skip_ws();
                    NONE
                }
            };
            let lightness = match self.parse_number_or_percentage() {
                Ok(lightness) => {
                    self.skip_ws();
                    lightness.frac(100.)
                }
                Err(e) => {
                    self.consume_none(e)?;
                    self.skip_ws();
                    NONE
                }
            };
            (saturation, lightness)
        };

        let alpha = match (self.get(0), legacy_syntax) {
            (Some(b'/'), false) | (Some(b','), true) => {
                self.offset += 1;
                self.skip_ws();
                match self.parse_alpha_value() {
                    Ok(alpha) => {
                        self.skip_ws();
                        alpha
                    }
                    Err(e) if !legacy_syntax => {
                        self.consume_none(e)?;
                        self.skip_ws();
                        NONE
                    }
                    _ => {
                        if legacy_syntax {
                            return Err(ParseColorError {
                                span: self.span(0..1),
                                inner_error: ParseColorErrorInner::UnexpectedByte {
                                    expected: b',',
                                },
                                expected_none: false,
                            });
                        } else {
                            return Err(ParseColorError {
                                span: self.span(0..1),
                                inner_error: ParseColorErrorInner::UnexpectedByte {
                                    expected: b'/',
                                },
                                expected_none: false,
                            });
                        }
                    }
                }
            }
            _ => 1.,
        };

        if self.offset_input() != b")" {
            return Err(ParseColorError {
                span: self.span(0..1),
                inner_error: ParseColorErrorInner::UnexpectedByte { expected: b')' },
                expected_none: false,
            });
        }

        Ok(Srgb::from(Hsla {
            hue: normalize_hue(hue),
            saturation: clamp_unit_f32(saturation),
            lightness: clamp_unit_f32(lightness),
            alpha,
        }))
    }

    // hwb() = hwb( [<hue> | none]
    //              [<percentage> | <number> | none]
    //              [<percentage> | <number> | none]
    //              [ / [<alpha-value> | none] ]? )
    fn parse_hwb(&mut self) -> Result<Srgb, ParseColorError> {
        let hue = match self.parse_hue() {
            Ok(hue) => {
                self.skip_ws();
                hue
            }
            Err(e) => {
                self.consume_none(e)?;
                self.skip_ws();
                NONE
            }
        };

        let whiteness = match self.parse_number_or_percentage() {
            Ok(whiteness) => {
                self.skip_ws();
                whiteness.frac(100.)
            }
            Err(e) => {
                self.consume_none(e)?;
                self.skip_ws();
                NONE
            }
        };

        let blackness = match self.parse_number_or_percentage() {
            Ok(blackness) => {
                self.skip_ws();
                blackness.frac(100.)
            }
            Err(e) => {
                self.consume_none(e)?;
                self.skip_ws();
                NONE
            }
        };

        let alpha = match self.get(0) {
            Some(b'/') => {
                self.offset += 1;
                self.skip_ws();
                match self.parse_alpha_value() {
                    Ok(alpha) => {
                        self.skip_ws();
                        alpha
                    }
                    Err(e) => {
                        self.consume_none(e)?;
                        self.skip_ws();
                        NONE
                    }
                }
            }
            _ => 1.,
        };

        if self.offset_input() != b")" {
            return Err(ParseColorError {
                span: self.span(0..1),
                inner_error: ParseColorErrorInner::UnexpectedByte { expected: b')' },
                expected_none: false,
            });
        }

        Ok(Srgb::from(Hwba {
            hue: normalize_hue(hue),
            whiteness: clamp_unit_f32(whiteness),
            blackness: clamp_unit_f32(blackness),
            alpha,
        }))
    }

    // rgb()  = [ <legacy-rgb-syntax>  | <modern-rgb-syntax>  ]
    // rgba() = [ <legacy-rgba-syntax> | <modern-rgba-syntax> ]
    // <legacy-rgb-syntax>  = rgb(  <percentage>#{3} , <alpha-value>? ) |
    //                        rgb(  <number>#{3}     , <alpha-value>? )
    // <legacy-rgba-syntax> = rgba( <percentage>#{3} , <alpha-value>? ) |
    //                        rgba( <number>#{3}     , <alpha-value>? )
    // <modern-rgb-syntax>  = rgb(  [ <number> | <percentage> | none]{3} [ / [<alpha-value> | none] ]? )
    // <modern-rgba-syntax> = rgba( [ <number> | <percentage> | none]{3} [ / [<alpha-value> | none] ]? )
    fn parse_rgb(&mut self) -> Result<Srgb, ParseColorError> {
        let (red, legacy_syntax) = match self.parse_number_or_percentage() {
            Ok(red) => {
                self.skip_ws();
                match self.get(0) {
                    Some(b',') => {
                        self.offset += 1;
                        self.skip_ws();
                        (Some(red), true)
                    }
                    _ => (Some(red), false),
                }
            }
            Err(e) => {
                self.consume_none(e)?;
                self.skip_ws();
                (None, false)
            }
        };

        let (red, green, blue) = if legacy_syntax {
            match red.unwrap() {
                (Number, red) => {
                    let green = self.parse_number()?;
                    self.skip_ws();
                    self.consume_byte(b',')?;
                    self.skip_ws();
                    let blue = self.parse_number()?;
                    self.skip_ws();
                    (red / 255., green / 255., blue / 255.)
                }
                (Percentage, red) => {
                    let green = self.parse_percentage()?;
                    self.skip_ws();
                    self.consume_byte(b',')?;
                    self.skip_ws();
                    let blue = self.parse_percentage()?;
                    self.skip_ws();
                    (red, green, blue)
                }
            }
        } else {
            let red = red.map_or(NONE, |red| red.frac(255.));
            let green = match self.parse_number_or_percentage() {
                Ok(green) => {
                    self.skip_ws();
                    green.frac(255.)
                }
                Err(e) => {
                    self.consume_none(e)?;
                    self.skip_ws();
                    NONE
                }
            };
            let blue = match self.parse_number_or_percentage() {
                Ok(blue) => {
                    self.skip_ws();
                    blue.frac(255.)
                }
                Err(e) => {
                    self.consume_none(e)?;
                    self.skip_ws();
                    NONE
                }
            };
            (red, green, blue)
        };

        let alpha = match (self.get(0), legacy_syntax) {
            (Some(b'/'), false) | (Some(b','), true) => {
                self.offset += 1;
                self.skip_ws();
                match self.parse_alpha_value() {
                    Ok(alpha) => {
                        self.skip_ws();
                        alpha
                    }
                    Err(e) if !legacy_syntax => {
                        self.consume_none(e)?;
                        self.skip_ws();
                        NONE
                    }
                    _ => {
                        if legacy_syntax {
                            return Err(ParseColorError {
                                span: self.span(0..1),
                                inner_error: ParseColorErrorInner::UnexpectedByte {
                                    expected: b',',
                                },
                                expected_none: false,
                            });
                        } else {
                            return Err(ParseColorError {
                                span: self.span(0..1),
                                inner_error: ParseColorErrorInner::UnexpectedByte {
                                    expected: b'/',
                                },
                                expected_none: false,
                            });
                        }
                    }
                }
            }
            _ => 1.,
        };

        if self.offset_input() != b")" {
            return Err(ParseColorError {
                span: self.span(0..1),
                inner_error: ParseColorErrorInner::UnexpectedByte { expected: b')' },
                expected_none: false,
            });
        }

        Ok(Srgb::new(
            clamp_unit_f32(red),
            clamp_unit_f32(green),
            clamp_unit_f32(blue),
            alpha,
        ))
    }

    fn parse_named(&mut self) -> Result<Srgb, ParseColorError> {
        const NAMED_MAX_LEN: usize = 20;
        if self.len() > NAMED_MAX_LEN {
            return Err(ParseColorError {
                span: self.span(0..self.len()),
                inner_error: ParseColorErrorInner::UnexpectedElement {
                    expected: "a CSS color name",
                },
                expected_none: false,
            });
        }
        let mut name = [b'\0'; NAMED_MAX_LEN];
        let name = &mut name[..self.len()];
        for (i, c) in self.offset_input().iter().enumerate() {
            name[i] = c.to_ascii_lowercase();
        }
        Ok(match &*name {
            b"aliceblue" => rgb!(240, 248, 255),
            b"antiquewhite" => rgb!(250, 235, 215),
            b"aqua" => rgb!(0, 255, 255),
            b"aquamarine" => rgb!(127, 255, 212),
            b"azure" => rgb!(240, 255, 255),
            b"beige" => rgb!(245, 245, 220),
            b"bisque" => rgb!(255, 228, 196),
            b"black" => rgb!(0, 0, 0),
            b"blanchedalmond" => rgb!(255, 235, 205),
            b"blue" => rgb!(0, 0, 255),
            b"blueviolet" => rgb!(138, 43, 226),
            b"brown" => rgb!(165, 42, 42),
            b"burlywood" => rgb!(222, 184, 135),
            b"cadetblue" => rgb!(95, 158, 160),
            b"chartreuse" => rgb!(127, 255, 0),
            b"chocolate" => rgb!(210, 105, 30),
            b"coral" => rgb!(255, 127, 80),
            b"cornflowerblue" => rgb!(100, 149, 237),
            b"cornsilk" => rgb!(255, 248, 220),
            b"crimson" => rgb!(220, 20, 60),
            b"cyan" => rgb!(0, 255, 255),
            b"darkblue" => rgb!(0, 0, 139),
            b"darkcyan" => rgb!(0, 139, 139),
            b"darkgoldenrod" => rgb!(184, 134, 11),
            b"darkgray" => rgb!(169, 169, 169),
            b"darkgreen" => rgb!(0, 100, 0),
            b"darkgrey" => rgb!(169, 169, 169),
            b"darkkhaki" => rgb!(189, 183, 107),
            b"darkmagenta" => rgb!(139, 0, 139),
            b"darkolivegreen" => rgb!(85, 107, 47),
            b"darkorange" => rgb!(255, 140, 0),
            b"darkorchid" => rgb!(153, 50, 204),
            b"darkred" => rgb!(139, 0, 0),
            b"darksalmon" => rgb!(233, 150, 122),
            b"darkseagreen" => rgb!(143, 188, 143),
            b"darkslateblue" => rgb!(72, 61, 139),
            b"darkslategray" => rgb!(47, 79, 79),
            b"darkslategrey" => rgb!(47, 79, 79),
            b"darkturquoise" => rgb!(0, 206, 209),
            b"darkviolet" => rgb!(148, 0, 211),
            b"deeppink" => rgb!(255, 20, 147),
            b"deepskyblue" => rgb!(0, 191, 255),
            b"dimgray" => rgb!(105, 105, 105),
            b"dimgrey" => rgb!(105, 105, 105),
            b"dodgerblue" => rgb!(30, 144, 255),
            b"firebrick" => rgb!(178, 34, 34),
            b"floralwhite" => rgb!(255, 250, 240),
            b"forestgreen" => rgb!(34, 139, 34),
            b"fuchsia" => rgb!(255, 0, 255),
            b"gainsboro" => rgb!(220, 220, 220),
            b"ghostwhite" => rgb!(248, 248, 255),
            b"gold" => rgb!(255, 215, 0),
            b"goldenrod" => rgb!(218, 165, 32),
            b"gray" => rgb!(128, 128, 128),
            b"green" => rgb!(0, 128, 0),
            b"greenyellow" => rgb!(173, 255, 47),
            b"grey" => rgb!(128, 128, 128),
            b"honeydew" => rgb!(240, 255, 240),
            b"hotpink" => rgb!(255, 105, 180),
            b"indianred" => rgb!(205, 92, 92),
            b"indigo" => rgb!(75, 0, 130),
            b"ivory" => rgb!(255, 255, 240),
            b"khaki" => rgb!(240, 230, 140),
            b"lavender" => rgb!(230, 230, 250),
            b"lavenderblush" => rgb!(255, 240, 245),
            b"lawngreen" => rgb!(124, 252, 0),
            b"lemonchiffon" => rgb!(255, 250, 205),
            b"lightblue" => rgb!(173, 216, 230),
            b"lightcoral" => rgb!(240, 128, 128),
            b"lightcyan" => rgb!(224, 255, 255),
            b"lightgoldenrodyellow" => rgb!(250, 250, 210),
            b"lightgray" => rgb!(211, 211, 211),
            b"lightgreen" => rgb!(144, 238, 144),
            b"lightgrey" => rgb!(211, 211, 211),
            b"lightpink" => rgb!(255, 182, 193),
            b"lightsalmon" => rgb!(255, 160, 122),
            b"lightseagreen" => rgb!(32, 178, 170),
            b"lightskyblue" => rgb!(135, 206, 250),
            b"lightslategray" => rgb!(119, 136, 153),
            b"lightslategrey" => rgb!(119, 136, 153),
            b"lightsteelblue" => rgb!(176, 196, 222),
            b"lightyellow" => rgb!(255, 255, 224),
            b"lime" => rgb!(0, 255, 0),
            b"limegreen" => rgb!(50, 205, 50),
            b"linen" => rgb!(250, 240, 230),
            b"magenta" => rgb!(255, 0, 255),
            b"maroon" => rgb!(128, 0, 0),
            b"mediumaquamarine" => rgb!(102, 205, 170),
            b"mediumblue" => rgb!(0, 0, 205),
            b"mediumorchid" => rgb!(186, 85, 211),
            b"mediumpurple" => rgb!(147, 112, 219),
            b"mediumseagreen" => rgb!(60, 179, 113),
            b"mediumslateblue" => rgb!(123, 104, 238),
            b"mediumspringgreen" => rgb!(0, 250, 154),
            b"mediumturquoise" => rgb!(72, 209, 204),
            b"mediumvioletred" => rgb!(199, 21, 133),
            b"midnightblue" => rgb!(25, 25, 112),
            b"mintcream" => rgb!(245, 255, 250),
            b"mistyrose" => rgb!(255, 228, 225),
            b"moccasin" => rgb!(255, 228, 181),
            b"navajowhite" => rgb!(255, 222, 173),
            b"navy" => rgb!(0, 0, 128),
            b"oldlace" => rgb!(253, 245, 230),
            b"olive" => rgb!(128, 128, 0),
            b"olivedrab" => rgb!(107, 142, 35),
            b"orange" => rgb!(255, 165, 0),
            b"orangered" => rgb!(255, 69, 0),
            b"orchid" => rgb!(218, 112, 214),
            b"palegoldenrod" => rgb!(238, 232, 170),
            b"palegreen" => rgb!(152, 251, 152),
            b"paleturquoise" => rgb!(175, 238, 238),
            b"palevioletred" => rgb!(219, 112, 147),
            b"papayawhip" => rgb!(255, 239, 213),
            b"peachpuff" => rgb!(255, 218, 185),
            b"peru" => rgb!(205, 133, 63),
            b"pink" => rgb!(255, 192, 203),
            b"plum" => rgb!(221, 160, 221),
            b"powderblue" => rgb!(176, 224, 230),
            b"purple" => rgb!(128, 0, 128),
            b"rebeccapurple" => rgb!(102, 51, 153),
            b"red" => rgb!(255, 0, 0),
            b"rosybrown" => rgb!(188, 143, 143),
            b"royalblue" => rgb!(65, 105, 225),
            b"saddlebrown" => rgb!(139, 69, 19),
            b"salmon" => rgb!(250, 128, 114),
            b"sandybrown" => rgb!(244, 164, 96),
            b"seagreen" => rgb!(46, 139, 87),
            b"seashell" => rgb!(255, 245, 238),
            b"sienna" => rgb!(160, 82, 45),
            b"silver" => rgb!(192, 192, 192),
            b"skyblue" => rgb!(135, 206, 235),
            b"slateblue" => rgb!(106, 90, 205),
            b"slategray" => rgb!(112, 128, 144),
            b"slategrey" => rgb!(112, 128, 144),
            b"snow" => rgb!(255, 250, 250),
            b"springgreen" => rgb!(0, 255, 127),
            b"steelblue" => rgb!(70, 130, 180),
            b"tan" => rgb!(210, 180, 140),
            b"teal" => rgb!(0, 128, 128),
            b"thistle" => rgb!(216, 191, 216),
            b"tomato" => rgb!(255, 99, 71),
            b"turquoise" => rgb!(64, 224, 208),
            b"violet" => rgb!(238, 130, 238),
            b"wheat" => rgb!(245, 222, 179),
            b"white" => rgb!(255, 255, 255),
            b"whitesmoke" => rgb!(245, 245, 245),
            b"yellow" => rgb!(255, 255, 0),
            b"yellowgreen" => rgb!(154, 205, 50),
            b"transparent" => Srgb::new(0., 0., 0., 0.),
            _ => {
                return Err(ParseColorError {
                    span: self.span(0..self.len()),
                    inner_error: ParseColorErrorInner::UnexpectedElement {
                        expected: "a CSS color name",
                    },
                    expected_none: false,
                })
            }
        })
    }

    fn skip_sign(&mut self) {
        match self.input.get(self.offset) {
            Some(b'+') | Some(b'-') => self.offset += 1,
            _ => {}
        }
    }

    fn skip_ws(&mut self) {
        while self.input.len() > 0 && is_whitespace(self.get(0).unwrap_or_default()) {
            self.offset += 1;
        }
    }

    fn len(&self) -> usize {
        self.input.len() - self.offset
    }

    fn offset_input(&self) -> &[u8] {
        &self.input[self.offset..]
    }

    fn get(&self, index: usize) -> Option<u8> {
        self.input.get(self.offset + index).copied()
    }

    fn span(&self, span: std::ops::Range<usize>) -> std::ops::Range<usize> {
        let mut span = self.offset + span.start..(self.offset + span.end).min(self.input.len());

        if self.input[span.clone()].len() == 0 {
            span.start = span.start.saturating_sub(1);
        }

        span
    }
}

fn is_ident_start(input: &[u8]) -> bool {
    match input.get(0) {
        Some(b'-') => match input.get(1) {
            Some(b'-') => true,
            Some(c) => is_name_start(*c),
            _ => false,
        },
        Some(c) => is_name_start(*c),
        _ => false,
    }
}

fn is_name_start(c: u8) -> bool {
    match c {
        b'a'..=b'z' | b'A'..=b'Z' | b'_' => true,
        c => !c.is_ascii(),
    }
}

fn is_name(c: u8) -> bool {
    match c {
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' => true,
        c => !c.is_ascii(),
    }
}

fn is_whitespace(c: u8) -> bool {
    c <= b' ' && (c == b' ' || c == b'\n' || c == b'\t' || c == b'\r' || c == b'\x0C')
}

fn digit(c: u8) -> Result<u8, ()> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        _ => Err(()),
    }
}

fn hex_digit(c: u8) -> Result<u8, ()> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(()),
    }
}

enum NumberOrPercentage {
    Number,
    Percentage,
}
use thiserror::Error;
use NumberOrPercentage::*;

trait Frac {
    fn frac(&self, denom: f32) -> f32;
}

impl Frac for (NumberOrPercentage, f32) {
    fn frac(&self, denom: f32) -> f32 {
        match self.0 {
            Number => self.1 / denom,
            Percentage => self.1,
        }
    }
}

#[cfg(test)]
mod tests;
