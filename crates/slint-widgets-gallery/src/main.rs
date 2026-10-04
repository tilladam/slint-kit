//! Gallery of slint-widgets' components. `GALLERY_SCHEME=light|dark` forces
//! the colour scheme, for screenshots of both.

use md_segments::highlight;
use md_segments::segmenter::{self, Segment};
use slint::{Color, ModelRc, VecModel};

slint::include_modules!();

const SAMPLE: &str = r#"/// Sum of the squares of the even numbers.
fn even_square_sum(xs: &[i64]) -> i64 {
    xs.iter().filter(|x| *x % 2 == 0).map(|x| x * x).sum()
}

fn main() {
    println!("{}", even_square_sum(&[1, 2, 3, 4])); // a long trailing comment that makes this line wider than the card"#;

/// Sample entries in slinty-pi's style: (id, icon, label, detail).
const ENTRIES: &[(&str, &str, &str, &str)] = &[
    (
        "action:new-session",
        "▸",
        "New session",
        "start fresh in this project",
    ),
    (
        "action:open-tree",
        "▸",
        "Open session tree",
        "browse and fork from any point",
    ),
    ("action:toggle-sidebar", "▸", "Toggle sidebar", ""),
    ("session:1", "≡", "Refactor the parser module", "2026-10-03"),
    ("session:2", "≡", "Test run walkthrough", "2026-09-28"),
    ("command:compact", "/", "/compact", "compact the context"),
    ("command:model", "/", "/model", "choose a model"),
    (
        "model:0",
        "◇",
        "qwen3.5-4b · rapid-mlx · local",
        "load model",
    ),
];

fn palette_rows(query: &str) -> ModelRc<PaletteRow> {
    let ranked = palette_rank::rank(ENTRIES, query, palette_rank::DEFAULT_LIMIT, |e| {
        format!("{} {}", e.2, e.3)
    });
    ModelRc::new(VecModel::from(
        ranked
            .into_iter()
            .map(|(id, icon, label, detail)| PaletteRow {
                id: id.into(),
                icon: icon.into(),
                label: label.into(),
                detail: detail.into(),
            })
            .collect::<Vec<_>>(),
    ))
}

fn code_lines(code: &str, dark: bool) -> Vec<CodeLine> {
    highlight::highlight_lines(code, "rust", dark)
        .into_iter()
        .map(|line| CodeLine {
            spans: ModelRc::new(VecModel::from(
                line.spans
                    .into_iter()
                    .map(|s| ColoredSpan {
                        text: s.text.into(),
                        color: Color::from_rgb_u8(s.color.0, s.color.1, s.color.2),
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect()
}

const MARKDOWN: &str = "# Shared markdown blocks\n\n\
Some **bold** prose with a [link](https://slint.dev) and `inline code`, long enough to wrap \
onto a second line in the gallery window.\n\n\
> A quoted line with *emphasis*.\n\n\
---\n\n\
| Crate | What it does | Notes |\n|---|---|---|\n\
| md-segments | Markdown split into renderable segments with syntax highlighting, bare-URL linking and shortcode emoji for any frontend | short |\n\
| x | y | A deliberately long cell that has to wrap over several lines in a narrow column so the row must grow to fit it all |\n\
| desktop-notify | Notifications | ok |\n\n\
| A | B | C | D | E | F |\n|---|---|---|---|---|---|\n\
| one two three four five six seven | x | eight nine ten eleven twelve thirteen fourteen | y | z | fifteen sixteen seventeen eighteen nineteen twenty |\n\
| a | b | c | d | twenty-one twenty-two twenty-three twenty-four twenty-five | e |\n";

fn styled(md: &str) -> slint::StyledText {
    slint::StyledText::from_markdown(md).unwrap_or_else(|_| slint::StyledText::from_plain_text(md))
}

fn table(rows: &[Vec<segmenter::TableCell>]) -> (ModelRc<TableRowCells>, f32) {
    let (weights, natural_width) = segmenter::column_layout(rows);
    let rows: Vec<TableRowCells> = rows
        .iter()
        .map(|row| TableRowCells {
            cells: ModelRc::new(VecModel::from(
                row.iter()
                    .enumerate()
                    .map(|(i, c)| TableCell {
                        text: c.text.as_str().into(),
                        header: c.header,
                        weight: weights.get(i).copied().unwrap_or(1.0),
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect();
    (ModelRc::new(VecModel::from(rows)), natural_width)
}

fn fill_markdown_page(app: &Gallery) {
    let mut tables = Vec::new();
    for segment in segmenter::segment_markdown(MARKDOWN) {
        match segment {
            Segment::Heading { level, text } => {
                app.set_heading(text.into());
                app.set_heading_level(level.into());
            }
            Segment::Prose(md) => {
                app.set_prose(styled(&md));
                app.set_prose_raw(md.into());
            }
            Segment::Quote(md) => app.set_quote(styled(&md)),
            Segment::Table(rows) => tables.push(table(&rows)),
            _ => {}
        }
    }
    let mut tables = tables.into_iter();
    if let Some((rows, width)) = tables.next() {
        app.set_table3(rows);
        app.set_table3_width(width);
    }
    if let Some((rows, width)) = tables.next() {
        app.set_table6(rows);
        app.set_table6_width(width);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let app = Gallery::new()?;
    app.global::<KitStyle>()
        .set_mono_font(slint_widgets::default_mono_font().into());
    if let Ok(scheme) = std::env::var("GALLERY_SCHEME") {
        app.invoke_force_scheme(scheme.into());
    }
    let dark = app.get_dark();
    app.set_code(SAMPLE.into());
    app.set_code_lines(ModelRc::new(VecModel::from(code_lines(SAMPLE, dark))));
    let (r, g, b) = highlight::theme_background(dark);
    app.set_code_background(Color::from_rgb_u8(r, g, b));

    app.set_palette_entries(palette_rows(""));
    let weak = app.as_weak();
    app.on_palette_query(move |q| {
        if let Some(app) = weak.upgrade() {
            app.set_palette_entries(palette_rows(&q));
        }
    });
    let weak = app.as_weak();
    app.on_palette_exec(move |id| {
        if let Some(app) = weak.upgrade() {
            app.set_status(format!("ran {id}").into());
            app.set_palette_entries(palette_rows(""));
        }
    });
    if let Ok(page) = std::env::var("GALLERY_PAGE") {
        app.set_page(page.into());
    }
    fill_markdown_page(&app);
    if std::env::var_os("GALLERY_PALETTE").is_some() {
        app.set_palette_visible(true);
    }
    app.run()
}
