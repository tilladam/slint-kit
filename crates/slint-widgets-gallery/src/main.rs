//! Gallery of slint-widgets' components. `GALLERY_SCHEME=light|dark` forces
//! the colour scheme, for screenshots of both.

use md_segments::highlight;
use slint::{Color, ModelRc, VecModel};

slint::include_modules!();

const SAMPLE: &str = r#"/// Sum of the squares of the even numbers.
fn even_square_sum(xs: &[i64]) -> i64 {
    xs.iter().filter(|x| *x % 2 == 0).map(|x| x * x).sum()
}

fn main() {
    println!("{}", even_square_sum(&[1, 2, 3, 4])); // a long trailing comment that makes this line wider than the card"#;

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
    app.run()
}
