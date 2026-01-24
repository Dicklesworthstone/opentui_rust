//! Color blending performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{Criterion, criterion_group, criterion_main};
use opentui::Rgba;
use std::hint::black_box;

fn color_creation(c: &mut Criterion) {
    c.bench_function("color_from_rgb_u8", |b| {
        b.iter(|| Rgba::from_rgb_u8(black_box(100), black_box(149), black_box(237)));
    });

    c.bench_function("color_from_rgba_u8", |b| {
        b.iter(|| {
            Rgba::from_rgba_u8(
                black_box(100),
                black_box(149),
                black_box(237),
                black_box(128),
            )
        });
    });

    c.bench_function("color_from_hex", |b| {
        b.iter(|| Rgba::from_hex(black_box("#6495ED")));
    });

    c.bench_function("color_from_hex_rgba", |b| {
        b.iter(|| Rgba::from_hex(black_box("#6495ED80")));
    });
}

fn color_blending(c: &mut Criterion) {
    let fg = Rgba::RED.with_alpha(0.5);
    let bg = Rgba::BLUE;

    c.bench_function("blend_over_single", |b| {
        b.iter(|| black_box(fg).blend_over(black_box(bg)));
    });

    c.bench_function("blend_over_chain_5", |b| {
        let colors = [
            Rgba::RED.with_alpha(0.2),
            Rgba::GREEN.with_alpha(0.3),
            Rgba::BLUE.with_alpha(0.4),
            Rgba::WHITE.with_alpha(0.5),
            Rgba::from_rgb_u8(128, 64, 192).with_alpha(0.6),
        ];
        b.iter(|| {
            let mut result = Rgba::BLACK;
            for color in &colors {
                result = black_box(*color).blend_over(result);
            }
            result
        });
    });

    c.bench_function("blend_over_100_layers", |b| {
        let layer = Rgba::RED.with_alpha(0.1);
        b.iter(|| {
            let mut result = Rgba::BLACK;
            for _ in 0..100 {
                result = black_box(layer).blend_over(result);
            }
            result
        });
    });
}

fn color_conversion(c: &mut Criterion) {
    let color = Rgba::from_rgb_u8(100, 149, 237);

    c.bench_function("to_256_color", |b| {
        b.iter(|| black_box(color).to_256_color());
    });

    c.bench_function("to_16_color", |b| {
        b.iter(|| black_box(color).to_16_color());
    });

    c.bench_function("to_rgb_u8", |b| {
        b.iter(|| black_box(color).to_rgb_u8());
    });
}

fn color_interpolation(c: &mut Criterion) {
    let from = Rgba::RED;
    let to = Rgba::BLUE;

    c.bench_function("lerp_single", |b| {
        b.iter(|| black_box(from).lerp(black_box(to), black_box(0.5)));
    });

    c.bench_function("lerp_gradient_10_steps", |b| {
        b.iter(|| {
            let mut colors = [Rgba::BLACK; 10];
            for (i, color) in colors.iter_mut().enumerate() {
                #[allow(clippy::cast_precision_loss)]
                let t = i as f32 / 9.0;
                *color = black_box(from).lerp(black_box(to), t);
            }
            colors
        });
    });
}

criterion_group!(
    benches,
    color_creation,
    color_blending,
    color_conversion,
    color_interpolation
);
criterion_main!(benches);
