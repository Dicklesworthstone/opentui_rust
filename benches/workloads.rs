//! Realistic application workload benchmarks.
//!
//! These benchmarks simulate actual application behavior rather than
//! micro-benchmarks of individual operations. They help users understand
//! expected performance for common use cases.

#![allow(clippy::semicolon_if_nothing_returned)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names
)] // Benchmarks use small, bounded values for clarity.

use criterion::{Criterion, criterion_group, criterion_main};
use opentui::buffer::{BoxStyle, ClipRect};
use opentui::renderer::BufferDiff;
use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use std::hint::black_box;

/// Simulate rendering a code editor frame.
///
/// This benchmark represents a typical text editor view with:
/// - Line numbers column
/// - Code content area
/// - Status bar
/// - Frame diff computation
fn bench_editor_frame(c: &mut Criterion) {
    let code = include_str!("../src/lib.rs");
    let lines: Vec<&str> = code.lines().collect();

    let mut group = c.benchmark_group("editor_frame");

    group.bench_function("render_80x24_scroll target<1ms", |b| {
        let mut front = OptimizedBuffer::new(80, 24);
        let mut back = OptimizedBuffer::new(80, 24);
        let mut scroll = 0usize;

        let bg = Rgba::from_rgb_u8(30, 30, 40);
        let line_num_style = Style::fg(Rgba::from_rgb_u8(100, 100, 120));
        let code_style = Style::fg(Rgba::from_rgb_u8(220, 220, 240));
        let status_bg = Rgba::from_rgb_u8(60, 60, 80);

        b.iter(|| {
            back.clear(bg);

            // Line numbers
            for y in 0..22 {
                let line_num = scroll + y;
                if line_num < lines.len() {
                    let num_str = format!("{:4} ", line_num + 1);
                    back.draw_text(0, y as u32, &num_str, line_num_style);
                }
            }

            // Code lines
            for y in 0..22 {
                let line_num = scroll + y;
                if line_num < lines.len() {
                    let line = lines[line_num];
                    let display: String = line.chars().take(74).collect();
                    back.draw_text(5, y as u32, &display, code_style);
                }
            }

            // Status bar
            back.fill_rect(0, 23, 80, 1, status_bg);
            back.draw_text(1, 23, " src/lib.rs | Ln 1, Col 1 ", Style::default());

            // Diff computation
            let diff = BufferDiff::compute(&front, &back);
            std::mem::swap(&mut front, &mut back);
            scroll = (scroll + 1) % lines.len().saturating_sub(22);

            black_box(diff);
        })
    });

    group.finish();
}

/// Simulate an animation frame with multiple moving elements.
///
/// This benchmark represents a typical animation scenario with:
/// - Background clear
/// - Multiple animated particles
/// - Progress bar update
fn bench_animation_frame(c: &mut Criterion) {
    struct Particle {
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        color: Rgba,
    }

    let mut particles: Vec<Particle> = (0..50)
        .map(|i| {
            let hue = ((i * 15) % 360) as f32;
            Particle {
                x: ((i * 17) % 80) as f32,
                y: ((i * 13) % 24) as f32,
                vx: (((i % 5) as f32) - 2.0) * 0.5,
                vy: (((i % 7) as f32) - 3.0) * 0.3,
                color: Rgba::from_hsv(hue, 0.8, 1.0),
            }
        })
        .collect();

    let mut group = c.benchmark_group("animation_frame");

    group.bench_function("render_50_particles target<1ms", |b| {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let mut frame = 0u32;
        let bg = Rgba::from_rgb_u8(20, 20, 30);

        b.iter(|| {
            buffer.clear(bg);

            // Update and draw particles
            for p in &mut particles {
                p.x = (p.x + p.vx).rem_euclid(80.0);
                p.y = (p.y + p.vy).rem_euclid(24.0);
                buffer.set(p.x as u32, p.y as u32, Cell::new('●', Style::fg(p.color)));
            }

            // Progress bar
            let progress = (frame % 100) as f32 / 100.0;
            let filled = (progress * 60.0) as u32;
            buffer.draw_text(10, 22, "[", Style::default());
            for i in 0..60 {
                let ch = if i < filled { '█' } else { '░' };
                buffer.set(11 + i, 22, Cell::new(ch, Style::default()));
            }
            buffer.draw_text(71, 22, "]", Style::default());

            frame = frame.wrapping_add(1);
            black_box(&buffer);
        })
    });

    group.finish();
}

/// Simulate a dashboard with multiple panels.
///
/// This benchmark represents a dashboard-style application with:
/// - Header/footer
/// - Multiple data panels with borders
/// - Progress bars and data displays
fn bench_dashboard_refresh(c: &mut Criterion) {
    let mut group = c.benchmark_group("dashboard_refresh");

    group.bench_function("render_5_panels target<1ms", |b| {
        let mut buffer = OptimizedBuffer::new(120, 40);
        let mut tick = 0u32;

        let bg = Rgba::from_rgb_u8(25, 25, 35);
        let border_color = Rgba::from_rgb_u8(80, 80, 100);
        let title_style = Style::fg(Rgba::WHITE).with_bold();
        let dim_style = Style::fg(Rgba::from_rgb_u8(150, 150, 170));
        let bar_color = Rgba::from_rgb_u8(80, 200, 120);

        b.iter(|| {
            buffer.clear(bg);

            // Header
            buffer.draw_box(0, 0, 120, 3, BoxStyle::double(Style::fg(border_color)));
            buffer.draw_text(3, 1, "System Dashboard", title_style);

            // Panel 1: CPU (left top)
            buffer.draw_box(0, 3, 60, 18, BoxStyle::single(Style::fg(border_color)));
            buffer.draw_text(2, 4, "CPU Usage", dim_style);
            let cpu = (tick * 7) % 100;
            for i in 0..50 {
                let ch = if i < cpu / 2 { '█' } else { '░' };
                buffer.set(5 + i, 6, Cell::new(ch, Style::fg(bar_color)));
            }
            buffer.draw_text(5, 8, &format!("{cpu}%"), Style::default());

            // Panel 2: Memory (right top)
            buffer.draw_box(60, 3, 60, 18, BoxStyle::single(Style::fg(border_color)));
            buffer.draw_text(62, 4, "Memory", dim_style);
            let mem = (tick * 3) % 100;
            for i in 0..50 {
                let ch = if i < mem / 2 { '█' } else { '░' };
                buffer.set(
                    65 + i,
                    6,
                    Cell::new(ch, Style::fg(Rgba::from_rgb_u8(200, 150, 80))),
                );
            }

            // Panel 3: Network (left bottom)
            buffer.draw_box(0, 21, 60, 16, BoxStyle::single(Style::fg(border_color)));
            buffer.draw_text(2, 22, "Network I/O", dim_style);
            buffer.draw_text(
                5,
                24,
                &format!("↓ {} KB/s", (tick * 13) % 1000),
                Style::default(),
            );
            buffer.draw_text(
                5,
                25,
                &format!("↑ {} KB/s", (tick * 7) % 500),
                Style::default(),
            );

            // Panel 4: Disk (right bottom)
            buffer.draw_box(60, 21, 60, 16, BoxStyle::single(Style::fg(border_color)));
            buffer.draw_text(62, 22, "Disk I/O", dim_style);
            buffer.draw_text(
                65,
                24,
                &format!("Read: {} MB/s", (tick * 11) % 200),
                Style::default(),
            );
            buffer.draw_text(
                65,
                25,
                &format!("Write: {} MB/s", (tick * 5) % 100),
                Style::default(),
            );

            // Footer
            buffer.draw_box(0, 37, 120, 3, BoxStyle::double(Style::fg(border_color)));
            buffer.draw_text(3, 38, &format!("Last update: tick {tick}"), dim_style);

            tick = tick.wrapping_add(1);
            black_box(&buffer);
        })
    });

    group.finish();
}

/// Simulate scrolling through a large document.
///
/// This benchmark represents viewing a large document (10K lines) with
/// continuous scrolling, useful for measuring viewport rendering performance.
fn bench_large_document_scroll(c: &mut Criterion) {
    let document: String = (0..10000)
        .map(|i| format!("Line {i:5}: This is content for testing scroll performance"))
        .collect::<Vec<_>>()
        .join("\n");
    let lines: Vec<&str> = document.lines().collect();

    let mut group = c.benchmark_group("large_document");

    group.bench_function("scroll_10k_lines target<500us", |b| {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let mut scroll = 0usize;
        let style = Style::fg(Rgba::from_rgb_u8(200, 200, 220));

        b.iter(|| {
            buffer.clear(Rgba::BLACK);

            for y in 0..24 {
                let line_idx = scroll + y;
                if line_idx < lines.len() {
                    buffer.draw_text(0, y as u32, lines[line_idx], style);
                }
            }

            scroll = (scroll + 1) % (lines.len() - 24);
            black_box(&buffer);
        })
    });

    group.finish();
}

/// Simulate a popup dialog with clipping.
///
/// This benchmark represents rendering a modal dialog that requires
/// scissor clipping to constrain content within bounds.
fn bench_popup_dialog(c: &mut Criterion) {
    let mut group = c.benchmark_group("popup_dialog");

    group.bench_function("render_with_scissor target<500us", |b| {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let bg = Rgba::from_rgb_u8(40, 40, 50);
        let popup_bg = Rgba::from_rgb_u8(60, 60, 80);
        let border_style = BoxStyle::double(Style::fg(Rgba::WHITE));

        b.iter(|| {
            // Background content
            buffer.clear(bg);
            for y in 0..24 {
                buffer.draw_text(
                    0,
                    y,
                    "Background text that should be clipped by popup...",
                    Style::dim(),
                );
            }

            // Popup with scissor
            let popup_x: u32 = 20;
            let popup_y: u32 = 5;
            let popup_w: u32 = 40;
            let popup_h: u32 = 14;

            let popup_x_i32 = i32::try_from(popup_x).unwrap_or(i32::MAX);
            let popup_y_i32 = i32::try_from(popup_y).unwrap_or(i32::MAX);

            buffer.push_scissor(ClipRect::new(popup_x_i32, popup_y_i32, popup_w, popup_h));

            buffer.fill_rect(popup_x, popup_y, popup_w, popup_h, popup_bg);
            buffer.draw_box(popup_x, popup_y, popup_w, popup_h, border_style.clone());
            buffer.draw_text(
                popup_x + 2,
                popup_y + 1,
                "Confirm Action",
                Style::fg(Rgba::WHITE).with_bold(),
            );
            buffer.draw_text(
                popup_x + 2,
                popup_y + 3,
                "Are you sure you want to",
                Style::default(),
            );
            buffer.draw_text(
                popup_x + 2,
                popup_y + 4,
                "proceed with this action?",
                Style::default(),
            );

            // Buttons
            buffer.draw_text(popup_x + 8, popup_y + 8, "[ OK ]", Style::fg(Rgba::GREEN));
            buffer.draw_text(
                popup_x + 22,
                popup_y + 8,
                "[ Cancel ]",
                Style::fg(Rgba::RED),
            );

            buffer.pop_scissor();

            black_box(&buffer);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_editor_frame,
    bench_animation_frame,
    bench_dashboard_refresh,
    bench_large_document_scroll,
    bench_popup_dialog
);
criterion_main!(benches);
