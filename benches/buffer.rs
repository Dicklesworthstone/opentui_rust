//! Buffer performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use opentui::buffer::{BoxStyle, ClipRect};
use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use opentui_rust as opentui;
use std::hint::black_box;

fn buffer_creation(c: &mut Criterion) {
    c.bench_function("buffer_new_80x24", |b| {
        b.iter(|| OptimizedBuffer::new(black_box(80), black_box(24)));
    });

    c.bench_function("buffer_new_200x50", |b| {
        b.iter(|| OptimizedBuffer::new(black_box(200), black_box(50)));
    });
}

fn buffer_clear(c: &mut Criterion) {
    let mut buffer = OptimizedBuffer::new(200, 50);

    c.bench_function("buffer_clear", |b| {
        b.iter(|| buffer.clear(black_box(Rgba::BLACK)))
    });
}

fn buffer_draw_text(c: &mut Criterion) {
    let mut buffer = OptimizedBuffer::new(200, 50);
    let style = Style::fg(Rgba::WHITE);

    c.bench_function("buffer_draw_text_short", |b| {
        b.iter(|| {
            buffer.draw_text(0, 0, black_box("Hello, World!"), style);
        })
    });

    c.bench_function("buffer_draw_text_long", |b| {
        let long_text = "x".repeat(100);
        b.iter(|| {
            buffer.draw_text(0, 0, black_box(&long_text), style);
        })
    });
}

fn buffer_cell_ops(c: &mut Criterion) {
    // Expected: cell_set_80x24 <100us, cell_get_80x24 <50us on modern hardware.
    let cell = Cell::new('X', Style::fg(Rgba::RED));
    let sizes = [(80, 24), (120, 40), (200, 60)];

    {
        let mut set_group = c.benchmark_group("cell_set_full");
        for (width, height) in sizes {
            let mut buffer = OptimizedBuffer::new(width, height);
            set_group.bench_with_input(
                BenchmarkId::from_parameter(format!("{width}x{height}")),
                &(width, height),
                |b, _| {
                    b.iter(|| {
                        for y in 0..height {
                            for x in 0..width {
                                buffer.set(x, y, cell);
                            }
                        }
                    });
                },
            );
        }
        set_group.finish();
    }

    {
        let mut get_group = c.benchmark_group("cell_get_full");
        for (width, height) in sizes {
            let buffer = OptimizedBuffer::new(width, height);
            get_group.bench_with_input(
                BenchmarkId::from_parameter(format!("{width}x{height}")),
                &(width, height),
                |b, _| {
                    b.iter(|| {
                        for y in 0..height {
                            for x in 0..width {
                                black_box(buffer.get(x, y));
                            }
                        }
                    });
                },
            );
        }
        get_group.finish();
    }
}

fn buffer_fill_rect(c: &mut Criterion) {
    // Expected: fill_rect_80x24 <50us on modern hardware.
    let mut group = c.benchmark_group("fill_rect");
    for (width, height) in [(10, 5), (40, 20), (80, 24), (200, 60)] {
        let mut buffer = OptimizedBuffer::new(width, height);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{width}x{height}")),
            &(width, height),
            |b, _| {
                b.iter(|| buffer.fill_rect(0, 0, width, height, Rgba::BLUE));
            },
        );
    }
    group.finish();
}

fn buffer_scissor_ops(c: &mut Criterion) {
    let mut buffer = OptimizedBuffer::new(80, 24);

    c.bench_function("scissor_push_pop_10", |b| {
        b.iter(|| {
            for _ in 0..10 {
                buffer.push_scissor(ClipRect::new(10, 5, 60, 14));
            }
            for _ in 0..10 {
                buffer.pop_scissor();
            }
        });
    });

    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.push_scissor(ClipRect::new(10, 5, 60, 14));
    let style = Style::fg(Rgba::WHITE);
    c.bench_function("draw_text_with_scissor", |b| {
        b.iter(|| buffer.draw_text(0, 10, "Clipped text", style));
    });
}

fn buffer_draw_box(c: &mut Criterion) {
    // Expected: draw_box <20us for 80x24 region.
    let mut group = c.benchmark_group("draw_box");
    let mut buffer = OptimizedBuffer::new(80, 24);
    let border_style = Style::fg(Rgba::WHITE);

    for name in ["single", "double", "rounded", "heavy"] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let style = match name {
                    "single" => BoxStyle::single(border_style),
                    "double" => BoxStyle::double(border_style),
                    "rounded" => BoxStyle::rounded(border_style),
                    "heavy" => BoxStyle::heavy(border_style),
                    _ => unreachable!("invalid box style"),
                };
                buffer.draw_box(5, 5, 70, 14, style);
            });
        });
    }
    group.finish();
}

fn buffer_blend_ops(c: &mut Criterion) {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.set_respect_alpha(true);
    let cell = Cell::new('█', Style::fg(Rgba::RED.with_alpha(0.5)));

    c.bench_function("set_blended_80x24", |b| {
        b.iter(|| {
            for y in 0..24 {
                for x in 0..80 {
                    buffer.set_blended(x, y, cell);
                }
            }
        });
    });
}

criterion_group!(
    benches,
    buffer_creation,
    buffer_clear,
    buffer_draw_text,
    buffer_cell_ops,
    buffer_fill_rect,
    buffer_scissor_ops,
    buffer_draw_box,
    buffer_blend_ops
);
criterion_main!(benches);
