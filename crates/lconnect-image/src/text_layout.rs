//! Glyph placement shared by measurement, wrapping and rasterization.
use ab_glyph::{point, Font, FontRef, GlyphId, OutlinedGlyph, ScaleFont};
use lconnect_ipc::TextStyle;
use std::ops::Range;
use unicode_bidi::{BidiInfo, Level};
use unicode_script::{Script, UnicodeScript};
use unicode_segmentation::UnicodeSegmentation;

pub(crate) struct PositionedGlyph {
    pub id: GlyphId,
    pub font_index: usize,
    pub x: f32,
    pub y: f32,
}
pub(crate) struct Line {
    pub glyphs: Vec<PositionedGlyph>,
    pub width: f32,
}

pub(crate) struct Shaper<'a, F: Font> {
    font: &'a F,
    fallback: Vec<(FontRef<'static>, rustybuzz::Face<'static>)>,
    em_size: f32,
    face: Option<rustybuzz::Face<'a>>,
    scale: f32,
    text: &'a str,
    bidi: BidiInfo<'a>,
}
impl<'a, F: Font> Shaper<'a, F> {
    pub fn new(font: &'a F, style: &TextStyle, text: &'a str) -> Option<Self> {
        let face = if style.line_box {
            Some(rustybuzz::Face::from_slice(font.font_data(), 0)?)
        } else {
            None
        };
        let scale = if style.line_box {
            f32::from(style.size) * font.height_unscaled() / font.units_per_em()?
        } else {
            f32::from(style.size)
        };
        let mut fallback = Vec::new();
        if style.line_box {
            for bytes in [
                include_bytes!("../assets/DejaVuSans.ttf").as_slice(),
                include_bytes!("../assets/NotoSansTC-Regular.otf").as_slice(),
            ] {
                fallback.push((
                    FontRef::try_from_slice(bytes).ok()?,
                    rustybuzz::Face::from_slice(bytes, 0)?,
                ));
            }
        }
        Some(Self {
            font,
            fallback,
            em_size: f32::from(style.size),
            face,
            scale,
            text,
            bidi: BidiInfo::new(text, Some(Level::ltr())),
        })
    }

    // Select whole grapheme clusters, so a mark is never detached from its base.
    // Control/variation characters do not need standalone glyphs.
    fn font_for_cluster(&self, cluster: &str) -> usize {
        let supports = |font: &dyn Fn(char) -> bool, face: &rustybuzz::Face<'_>| {
            if cluster.chars().filter(|&c| requires_glyph(c)).all(font) {
                return true;
            }
            // HarfBuzz can compose a base+mark to an available precomposed
            // glyph even when the individual mark is absent from cmap.
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(cluster);
            buffer.guess_segment_properties();
            rustybuzz::shape(face, &[], buffer)
                .glyph_infos()
                .iter()
                .all(|glyph| glyph.glyph_id != 0)
        };
        if self
            .face
            .as_ref()
            .is_some_and(|face| supports(&|c| self.font.glyph_id(c).0 != 0, face))
        {
            return 0;
        }
        self.fallback
            .iter()
            .position(|(font, face)| supports(&|c| font.glyph_id(c).0 != 0, face))
            .map_or(0, |index| index + 1)
    }

    pub fn outline(&self, placed: &PositionedGlyph, x: f32, y: f32) -> Option<OutlinedGlyph> {
        let position = point(x + placed.x, y + placed.y);
        if placed.font_index == 0 {
            self.font
                .outline_glyph(placed.id.with_scale_and_position(self.scale, position))
        } else {
            let font = &self.fallback[placed.font_index - 1].0;
            let scale = self.em_size * font.height_unscaled() / font.units_per_em()?;
            font.outline_glyph(placed.id.with_scale_and_position(scale, position))
        }
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn line(&self, range: Range<usize>) -> Line {
        self.line_break(range, false)
    }

    pub fn line_break(&self, range: Range<usize>, hyphenate: bool) -> Line {
        let text = &self.text[range.clone()];
        let mut line = Line {
            glyphs: Vec::new(),
            width: 0.0,
        };
        let Some(face) = &self.face else {
            let scaled = self.font.as_scaled(self.scale);
            let mut previous = None;
            for c in text.chars() {
                let id = scaled.glyph_id(c);
                if let Some(prev) = previous {
                    line.width += scaled.kern(prev, id);
                }
                line.glyphs.push(PositionedGlyph {
                    id,
                    font_index: 0,
                    x: line.width,
                    y: 0.0,
                });
                line.width += scaled.h_advance(id);
                previous = Some(id);
            }
            return line;
        };
        // Windows' canvas document inherits CSS direction:ltr. Resolve mixed runs;
        // the shaper handles marks, substitutions and mirroring within each run.

        let scaled = self.font.as_scaled(self.scale);
        let space = scaled.h_advance(scaled.glyph_id(' '));
        for paragraph in &self.bidi.paragraphs {
            let start = paragraph.range.start.max(range.start);
            let end = paragraph.range.end.min(range.end);
            if start >= end {
                continue;
            }
            let (levels, runs) = self.bidi.visual_runs(paragraph, start..end);
            for run in runs {
                let rtl = levels[run.start].is_rtl();
                let mut scripts = Vec::new();
                for (text, script) in script_runs(&self.text[run]) {
                    let mut start = 0;
                    let mut selected = None;
                    for (index, grapheme) in text.grapheme_indices(true) {
                        // Transparent controls inherit the surrounding font, keeping
                        // joining intact. SHY is a discretionary glyph: browser font
                        // fallback selects it independently, even when not drawn.
                        if grapheme != "\u{ad}" && !grapheme.chars().any(requires_glyph) {
                            continue;
                        }
                        let font_index = self.font_for_cluster(grapheme);
                        if selected.is_some_and(|previous| previous != font_index) {
                            scripts.push((&text[start..index], script, selected.unwrap()));
                            start = index;
                        }
                        selected = Some(font_index);
                    }
                    scripts.push((&text[start..], script, selected.unwrap_or(0)));
                }
                if rtl {
                    scripts.reverse();
                }
                for (text, script, font_index) in scripts {
                    let face = if font_index == 0 {
                        face
                    } else {
                        &self.fallback[font_index - 1].1
                    };
                    let factor = self.em_size / face.units_per_em() as f32;
                    if text == "\t" {
                        // CSS default tab-size:8. A stop closer than half a
                        // space is skipped so the tab remains visibly separated.
                        let interval = space * 8.0;
                        if interval > 0.0 {
                            let mut advance = interval - line.width.rem_euclid(interval);
                            if advance < space / 2.0 {
                                advance += interval;
                            }
                            line.width += advance;
                        }
                        continue;
                    }
                    let mut buffer = rustybuzz::UnicodeBuffer::new();
                    let at_line_end = text.as_ptr() as usize + text.len()
                        == self.text.as_ptr() as usize + range.end;
                    if hyphenate && at_line_end && text.ends_with('\u{ad}') {
                        buffer.push_str(text.trim_end_matches('\u{ad}'));
                        // CSS's default visible hyphen, with a same-font fallback.
                        buffer.push_str(if face.glyph_index('\u{2010}').is_some() {
                            "\u{2010}"
                        } else {
                            "-"
                        });
                    } else {
                        buffer.push_str(text);
                    }
                    buffer.set_direction(if rtl {
                        rustybuzz::Direction::RightToLeft
                    } else {
                        rustybuzz::Direction::LeftToRight
                    });
                    if let Some(script) = rustybuzz::Script::from_iso15924_tag(
                        rustybuzz::ttf_parser::Tag(script.as_iso15924_tag()),
                    ) {
                        buffer.set_script(script);
                    }
                    buffer.guess_segment_properties();
                    let output = rustybuzz::shape(face, &[], buffer);
                    for (info, position) in
                        output.glyph_infos().iter().zip(output.glyph_positions())
                    {
                        // OpenType glyph indices are bounded by the u16 maxp glyph count.
                        line.glyphs.push(PositionedGlyph {
                            id: GlyphId(info.glyph_id as u16),
                            font_index,
                            x: line.width + position.x_offset as f32 * factor,
                            y: -(position.y_offset as f32) * factor,
                        });
                        line.width += position.x_advance as f32 * factor;
                    }
                }
            }
        }
        line
    }
}

// Keep combining/ZWJ clusters together. Common/inherited characters follow the
// surrounding script; initial punctuation joins the first explicit script.
fn script_runs(text: &str) -> Vec<(&str, Script)> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = Script::Common;
    for (index, grapheme) in text.grapheme_indices(true) {
        if grapheme == "\t" {
            if start < index {
                runs.push((&text[start..index], current));
            }
            runs.push((grapheme, Script::Common));
            start = index + grapheme.len();
            current = Script::Common;
            continue;
        }
        let Some(script) = grapheme
            .chars()
            .map(|c| c.script())
            .find(|s| !matches!(s, Script::Common | Script::Inherited | Script::Unknown))
        else {
            continue;
        };
        if current == Script::Common {
            current = script;
        } else if script != current {
            runs.push((&text[start..index], current));
            start = index;
            current = script;
        }
    }
    if start < text.len() {
        runs.push((&text[start..], current));
    }
    runs
}

fn requires_glyph(c: char) -> bool {
    !c.is_control()
        && !matches!(c,
            '\u{ad}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' |
            '\u{2060}'..='\u{206f}' | '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}
