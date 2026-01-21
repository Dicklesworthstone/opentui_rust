//! Text buffer performance benchmarks.

#![allow(clippy::semicolon_if_nothing_returned)]

use criterion::{Criterion, criterion_group, criterion_main};
use opentui::{EditBuffer, TextBuffer};
use std::hint::black_box;

fn text_buffer_creation(c: &mut Criterion) {
    c.bench_function("textbuffer_new", |b| {
        b.iter(|| TextBuffer::new());
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
    let mut buffer = TextBuffer::with_text("Hello, World!\nLine 2\nLine 3\nLine 4");

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

fn edit_buffer_creation(c: &mut Criterion) {
    c.bench_function("editbuffer_new", |b| {
        b.iter(|| EditBuffer::new());
    });

    c.bench_function("editbuffer_with_text", |b| {
        b.iter(|| EditBuffer::with_text(black_box("Hello, World!")));
    });

    let long_text = "x".repeat(10_000);
    c.bench_function("editbuffer_with_text_10k", |b| {
        b.iter(|| EditBuffer::with_text(black_box(&long_text)));
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
    let text = (0..100).map(|i| format!("Line number {} with some content\n", i)).collect::<String>();
    let mut editor = EditBuffer::with_text(&text);
    editor.move_to(50, 10);

    c.bench_function("editbuffer_move_left", |b| {
        b.iter(|| {
            editor.move_left();
            editor.move_right();
        });
    });

    c.bench_function("editbuffer_move_up_down", |b| {
        b.iter(|| {
            editor.move_up();
            editor.move_down();
        });
    });

    c.bench_function("editbuffer_move_to_line_start", |b| {
        b.iter(|| {
            editor.move_to_line_start();
            editor.move_to_line_end();
        });
    });
}

fn edit_buffer_undo_redo(c: &mut Criterion) {
    c.bench_function("editbuffer_commit", |b| {
        let mut editor = EditBuffer::new();
        editor.insert("test ");
        b.iter(|| {
            editor.commit();
        });
    });

    c.bench_function("editbuffer_undo_redo_cycle", |b| {
        let mut editor = EditBuffer::new();
        editor.insert("Hello");
        editor.commit();
        editor.insert(" World");
        editor.commit();
        b.iter(|| {
            editor.undo();
            editor.redo();
        });
    });
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
    edit_buffer_creation,
    edit_buffer_insertion,
    edit_buffer_cursor_movement,
    edit_buffer_undo_redo,
    edit_buffer_deletion
);
criterion_main!(benches);
