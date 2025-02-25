use miette::IntoDiagnostic;
use std::{
    io::{stdin, BufRead},
    str::FromStr,
};

use css_color::Srgb;

fn main() -> miette::Result<()> {
    let stdin = stdin().lock().lines();

    println!("Enter CSS color strings for parsing");

    for line in stdin {
        let line = line.into_diagnostic()?;

        match Srgb::from_str(&line) {
            Ok(color) => println!("{:?}", color),
            Err(e) => eprintln!("{:?}", miette::Error::new(e).with_source_code(line)),
        }
    }

    Ok(())
}
