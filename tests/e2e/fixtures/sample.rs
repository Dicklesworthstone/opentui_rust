// Sample Rust fixture for E2E highlighting

use std::fmt::{self, Display};

#[derive(Debug, Clone)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn magnitude(&self) -> f32 {
        let sum = (self.x * self.x + self.y * self.y) as f32;
        sum.sqrt()
    }
}

fn borrow<'a>(s: &'a str) -> &'a str {
    s
}

/// Doc comment
fn main() {
    let mut value = 42u8;
    let name = "opentui";
    let raw = r#"raw \"string\" content"#;
    let ch = 'x';
    let bytes = b"abc";
    let opt: Option<i32> = Some(1);

    // Line comment
    /* block comment */
    if value > 0 {
        println!("{} {}", name, value);
    } else if value == 0 {
        println!("zero");
    }

    match opt {
        Some(v) => println!("value = {}", v),
        None => println!("none"),
    }

    for i in 0..3 {
        value += i;
    }

    let _ = Point::new(1, 2).magnitude();
    let _ = borrow("text");
    let _ = fmt::Error;
    let _ = Display::fmt;
}
