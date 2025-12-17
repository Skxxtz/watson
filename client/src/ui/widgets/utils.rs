use gtk4::cairo::Context;
use std::str::FromStr;

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
    pub fn centered_text(ctx: &Context, text: &str, cx: f64, cy: f64) {
        let ext = ctx.text_extents(text).unwrap();
        let font_ext = ctx.font_extents().unwrap();

        // Horizontal: center ink box
        let x = cx - (ext.width() / 2.0 + ext.x_bearing());

        // Vertical: center using baseline + ascent/descent
        let y = cy + (font_ext.ascent() - font_ext.descent()) / 2.0;

        ctx.move_to(x, y);
        ctx.show_text(text).unwrap();
    }

}


#[derive(Default, Debug, Clone, Copy)]
pub struct Rgba {
    pub r: f64, // 0.0 to 1.0
    pub g: f64,
    pub b: f64,
    pub a: f64,
}
impl Rgba {
    pub fn darken(&mut self, alpha: f64) {
        self.r = self.r * (1.0 - alpha);
        self.g = self.g * (1.0 - alpha);
        self.b = self.b * (1.0 - alpha);
    }
    
}

#[derive(Debug, Clone, Copy)]
pub struct Hsl {
    pub h: f64, // 0.0 to 360.0
    pub s: f64, // 0.0 to 1.0
    pub l: f64,
}

// Implement FromStr to allow: let color: Rgba = "#ff0000".parse().unwrap();
impl FromStr for Rgba {
    type Err = ();

    fn from_str(hex: &str) -> Result<Self, Self::Err> {
        let hex = hex.trim_start_matches('#');
        let len = hex.len();
        if len != 6 && len != 8 { return Err(()); }

        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ())? as f64 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ())? as f64 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ())? as f64 / 255.0;
        let a = if len == 8 {
            u8::from_str_radix(&hex[6..8], 16).map_err(|_| ())? as f64 / 255.0
        } else {
            1.0
        };

        Ok(Self { r, g, b, a })
    }
}

// Trait for easy conversion between types
impl From<Rgba> for Hsl {
    fn from(rgba: Rgba) -> Self {
        let max = rgba.r.max(rgba.g).max(rgba.b);
        let min = rgba.r.min(rgba.g).min(rgba.b);
        let delta = max - min;
        let l = (max + min) / 2.0;

        let s = if delta == 0.0 { 0.0 } else { delta / (1.0 - (2.0 * l - 1.0).abs()) };

        let mut h = if delta == 0.0 { 0.0 } 
            else if max == rgba.r { 60.0 * (((rgba.g - rgba.b) / delta) % 6.0) }
            else if max == rgba.g { 60.0 * (((rgba.b - rgba.r) / delta) + 2.0) }
            else { 60.0 * (((rgba.r - rgba.g) / delta) + 4.0) };

        if h < 0.0 { h += 360.0; }
        Self { h, s, l }
    }
}

impl From<Hsl> for Rgba {
    fn from(hsl: Hsl) -> Self {
        let q = if hsl.l < 0.5 { hsl.l * (1.0 + hsl.s) } else { hsl.l + hsl.s - hsl.l * hsl.s };
        let p = 2.0 * hsl.l - q;
        let h_norm = hsl.h / 360.0;

        let res = (
            hue_to_rgb(p, q, h_norm + 1.0/3.0),
            hue_to_rgb(p, q, h_norm),
            hue_to_rgb(p, q, h_norm - 1.0/3.0)
        );
        Self { r: res.0, g: res.1, b: res.2, a: 1.0 }
    }
}

fn hue_to_rgb(p: f64, q: f64, t: f64) -> f64 {
    let mut t = t;
    if t < 0.0 { t += 1.0 } else if t > 1.0 { t -= 1.0 };
    if t < 1.0/6.0 { p + (q - p) * 6.0 * t }
    else if t < 1.0/2.0 { q }
    else if t < 2.0/3.0 { p + (q - p) * (2.0/3.0 - t) * 6.0 }
    else { p }
}

