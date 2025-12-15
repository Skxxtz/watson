use gtk4::cairo::Context;

pub struct CairoShapesExt;
impl CairoShapesExt {
    pub fn rounded_rectangle(ctx: &Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
        let r = radius.min(width / 2.0).min(height / 2.0);

        ctx.new_sub_path();
        ctx.arc(
            x + width - r,
            y + r,
            r,
            -90_f64.to_radians(),
            0_f64.to_radians(),
        );
        ctx.arc(
            x + width - r,
            y + height - r,
            r,
            0_f64.to_radians(),
            90_f64.to_radians(),
        );
        ctx.arc(
            x + r,
            y + height - r,
            r,
            90_f64.to_radians(),
            180_f64.to_radians(),
        );
        ctx.arc(x + r, y + r, r, 180_f64.to_radians(), 270_f64.to_radians());
        ctx.close_path();
    }

    pub fn circle(ctx: &Context, x: f64, y: f64, radius: f64) {
        ctx.new_path();
        ctx.arc(x, y, radius, 0.0, 2.0 * std::f64::consts::PI);
        ctx.close_path();
        ctx.fill().unwrap();
    }
    pub fn centered_text(ctx: &Context, text: &str, x: f64, y: f64) {
        let extends = ctx.text_extents(text).unwrap();
        let x_offset = extends.width() / 2.0 + extends.x_bearing();
        let y_offset = extends.height() / 2.0 + extends.y_bearing();
        ctx.move_to(x - x_offset, y - y_offset);
        ctx.show_text(text).unwrap();
    }
}

pub struct Conversions;
impl Conversions {
    pub fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return (0.0, 0.0, 0.0);
        }

        let r = u8::from_str_radix(&hex[0..2], 16).unwrap() as f64 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap() as f64 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap() as f64 / 255.0;
        (r, g, b)
    }
}
