//! Text and rope performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use opentui::{EditBuffer, TextBuffer};
use opentui_rust as opentui;
use std::hint::black_box;

fn text_buffer_creation(c: &mut Criterion) {
    c.bench_function("textbuffer_new", |b| {
        b.iter(TextBuffer::new);
    });

    c.bench_function("textbuffer_with_text_short", |b| {
        b.iter(|| TextBuffer::with_text(black_box("Hello, World!")));
    });

    let long_text = "x".repeat(10_000);
    c.bench_function("textbuffer_with_text_10k", |b| {
        b.iter(|| TextBuffer::with_text(black_box(&long_text)));
    });
}

fn text_buffer_ops(c: &mut Criterion) {
    let buffer = TextBuffer::with_text("Hello, World!\nLine 2\nLine 3\nLine 4");

    c.bench_function("textbuffer_len_chars", |b| {
        b.iter(|| black_box(&buffer).len_chars());
    });

    c.bench_function("textbuffer_len_lines", |b| {
        b.iter(|| black_box(&buffer).len_lines());
    });

    c.bench_function("textbuffer_line", |b| {
        b.iter(|| black_box(&buffer).line(black_box(1)));
    });

    c.bench_function("textbuffer_to_string", |b| {
        b.iter(|| black_box(&buffer).to_string());
    });
}

fn rope_insert(c: &mut Criterion) {
    // Expected: insert <10us for 10k-char buffers.
    let text = "x".repeat(10_000);
    let mut group = c.benchmark_group("rope_insert");

    group.bench_function("beginning", |b| {
        b.iter_batched(
            || TextBuffer::with_text(&text),
            |mut buffer| {
                buffer.rope_mut().insert(0, "abc");
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("middle", |b| {
        b.iter_batched(
            || TextBuffer::with_text(&text),
            |mut buffer| {
                buffer.rope_mut().insert(5_000, "abc");
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("end", |b| {
        b.iter_batched(
            || TextBuffer::with_text(&text),
            |mut buffer| {
                buffer.rope_mut().insert(10_000, "abc");
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn rope_delete(c: &mut Criterion) {
    // Expected: delete <10us for 10k-char buffers.
    let text = "x".repeat(10_000);
    let mut group = c.benchmark_group("rope_delete");

    group.bench_function("beginning_10", |b| {
        b.iter_batched(
            || TextBuffer::with_text(&text),
            |mut buffer| {
                buffer.rope_mut().remove(0..10);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("middle_10", |b| {
        b.iter_batched(
            || TextBuffer::with_text(&text),
            |mut buffer| {
                buffer.rope_mut().remove(5_000..5_010);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn line_iteration(c: &mut Criterion) {
    let text = build_lines(1000);
    let buffer = TextBuffer::with_text(&text);

    c.bench_function("iterate_1000_lines", |b| {
        b.iter(|| {
            for line in buffer.lines() {
                black_box(line);
            }
        });
    });
}

fn line_access(c: &mut Criterion) {
    let text = build_lines(1000);
    let buffer = TextBuffer::with_text(&text);

    c.bench_function("random_line_access_100", |b| {
        b.iter(|| {
            for index in [0, 500, 999, 250, 750, 100, 900, 50, 950, 333] {
                black_box(buffer.line(index));
            }
        });
    });
}

fn build_lines(count: usize) -> String {
    use std::fmt::Write;

    let mut text = String::with_capacity(count.saturating_mul(12));
    for i in 0..count {
        let _ = writeln!(&mut text, "Line {i}");
    }
    text
}

fn edit_buffer_creation(c: &mut Criterion) {
    c.bench_function("editbuffer_new", |b| {
        b.iter(EditBuffer::new);
    });

    c.bench_function("editbuffer_with_text", |b| {
        b.iter(|| EditBuffer::with_text(black_box("Hello, World!")));
    });

    let long_text = "x".repeat(10_000);
    c.bench_function("editbuffer_with_text_10k", |b| {
        b.iter(|| EditBuffer::with_text(black_box(&long_text)));
    });
}

fn edit_buffer_typing(c: &mut Criterion) {
    let sentence = "The quick brown fox jumps over the lazy dog. ";
    c.bench_function("type_sentence", |b| {
        b.iter_batched(
            EditBuffer::new,
            |mut editor| {
                for ch in sentence.chars() {
                    let mut buf = [0u8; 4];
                    let s = ch.encode_utf8(&mut buf);
                    editor.insert(s);
                }
                black_box(editor);
            },
            BatchSize::SmallInput,
        );
    });
}

fn edit_buffer_insertion(c: &mut Criterion) {
    c.bench_function("editbuffer_insert_char", |b| {
        let mut editor = EditBuffer::new();
        b.iter(|| {
            editor.insert(black_box("x"));
        });
    });

    c.bench_function("editbuffer_insert_word", |b| {
        let mut editor = EditBuffer::new();
        b.iter(|| {
            editor.insert(black_box("hello "));
        });
    });

    c.bench_function("editbuffer_insert_line", |b| {
        let mut editor = EditBuffer::new();
        b.iter(|| {
            editor.insert(black_box("This is a complete line of text.\n"));
        });
    });
}

fn edit_buffer_cursor_movement(c: &mut Criterion) {
    let mut text = String::with_capacity(4000);
    for i in 0..100 {
        use std::fmt::Write;
        writeln!(text, "Line number {i} with some content").unwrap();
    }
    let mut editor = EditBuffer::with_text(&text);
    editor.move_to(50, 10);

    let mut group = c.benchmark_group("cursor_movement");
    group.bench_function("move_right_1000", |b| {
        b.iter(|| {
            editor.goto_line(0);
            for _ in 0..1000 {
                editor.move_right();
            }
        });
    });

    group.bench_function("move_down_100", |b| {
        b.iter(|| {
            editor.goto_line(0);
            for _ in 0..100 {
                editor.move_down();
            }
        });
    });

    group.finish();
}

fn edit_buffer_undo_redo(c: &mut Criterion) {
    let mut group = c.benchmark_group("undo_redo");

    group.bench_function("undo_10_operations", |b| {
        b.iter_batched(
            || {
                let mut editor = EditBuffer::new();
                for i in 0..10 {
                    editor.insert(&format!("text{i}"));
                    editor.commit();
                }
                editor
            },
            |mut editor| {
                for _ in 0..10 {
                    editor.undo();
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("redo_10_operations", |b| {
        b.iter_batched(
            || {
                let mut editor = EditBuffer::new();
                for i in 0..10 {
                    editor.insert(&format!("text{i}"));
                    editor.commit();
                }
                for _ in 0..10 {
                    editor.undo();
                }
                editor
            },
            |mut editor| {
                for _ in 0..10 {
                    editor.redo();
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn edit_buffer_deletion(c: &mut Criterion) {
    c.bench_function("editbuffer_delete_backward", |b| {
        let text = "x".repeat(10_000);
        let mut editor = EditBuffer::with_text(&text);
        editor.move_to_line_end();
        b.iter(|| {
            if editor.cursor().col > 0 {
                editor.delete_backward();
            } else {
                editor.move_to_line_end();
            }
        });
    });

    c.bench_function("editbuffer_delete_forward", |b| {
        let text = "x".repeat(10_000);
        let mut editor = EditBuffer::with_text(&text);
        b.iter(|| {
            if editor.cursor().col < 9999 {
                editor.delete_forward();
            } else {
                editor.move_to_line_start();
            }
        });
    });
}

criterion_group!(
    benches,
    text_buffer_creation,
    text_buffer_ops,
    rope_insert,
    rope_delete,
    line_iteration,
    line_access,
    edit_buffer_creation,
    edit_buffer_typing,
    edit_buffer_insertion,
    edit_buffer_cursor_movement,
    edit_buffer_undo_redo,
    edit_buffer_deletion
);
criterion_main!(benches);
