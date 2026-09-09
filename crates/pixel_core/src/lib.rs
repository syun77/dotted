use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        if a == 0 {
            Self::TRANSPARENT
        } else {
            Self { r, g, b, a }
        }
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Result<Self, EditError> {
        let count = width.checked_mul(height).ok_or(EditError::InvalidSize)?;
        if count == 0 {
            Err(EditError::InvalidSize)
        } else {
            Ok(Self { width, height })
        }
    }
    #[must_use]
    pub fn len(self) -> usize {
        (self.width as usize) * (self.height as usize)
    }
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct Coordinate {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PixelGrid {
    size: Size,
    pixels: Vec<Rgba8>,
}

impl PixelGrid {
    pub fn transparent(size: Size) -> Self {
        Self {
            size,
            pixels: vec![Rgba8::TRANSPARENT; size.len()],
        }
    }

    pub fn from_pixels(size: Size, pixels: Vec<Rgba8>) -> Result<Self, EditError> {
        if pixels.len() != size.len() {
            return Err(EditError::PixelCount);
        }
        Ok(Self {
            size,
            pixels: pixels
                .into_iter()
                .map(|p| Rgba8::new(p.r, p.g, p.b, p.a))
                .collect(),
        })
    }

    #[must_use]
    pub const fn size(&self) -> Size {
        self.size
    }
    #[must_use]
    pub fn pixels(&self) -> &[Rgba8] {
        &self.pixels
    }
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        self.pixels.iter().all(|p| p.a == 0)
    }
    #[must_use]
    pub fn used_colors(&self) -> HashSet<Rgba8> {
        self.pixels.iter().copied().filter(|p| p.a > 0).collect()
    }
    #[must_use]
    pub fn opaque_pixel_count(&self) -> usize {
        self.pixels.iter().filter(|p| p.a > 0).count()
    }

    fn index(&self, c: Coordinate) -> Result<usize, EditError> {
        if c.x < 0 || c.y < 0 || c.x >= self.size.width as i32 || c.y >= self.size.height as i32 {
            return Err(EditError::OutOfBounds(c));
        }
        Ok(c.y as usize * self.size.width as usize + c.x as usize)
    }
    pub fn get(&self, c: Coordinate) -> Result<Rgba8, EditError> {
        Ok(self.pixels[self.index(c)?])
    }
    fn set(&mut self, c: Coordinate, color: Rgba8) -> Result<(), EditError> {
        let i = self.index(c)?;
        self.pixels[i] = Rgba8::new(color.r, color.g, color.b, color.a);
        Ok(())
    }
    #[must_use]
    pub fn rgba_bytes(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|p| p.bytes()).collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct LayerId(pub Uuid);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct FrameId(pub Uuid);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct PaletteEntryId(pub Uuid);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaletteEntry {
    pub id: PaletteEntryId,
    pub color: Rgba8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    entries: Vec<PaletteEntry>,
}

impl Palette {
    pub fn new(entries: Vec<PaletteEntry>) -> Result<Self, EditError> {
        if !(2..=8).contains(&entries.len()) || entries.iter().any(|e| e.color.a == 0) {
            return Err(EditError::InvalidPalette);
        }
        let ids: HashSet<_> = entries.iter().map(|e| e.id).collect();
        if ids.len() != entries.len() {
            return Err(EditError::InvalidPalette);
        }
        Ok(Self { entries })
    }
    #[must_use]
    pub fn entries(&self) -> &[PaletteEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RasterDocument {
    pub size: Size,
    pub layer_id: LayerId,
    pub frame_id: FrameId,
    pub frame_duration_ms: u32,
    pub palette: Palette,
    grid: PixelGrid,
}

impl RasterDocument {
    pub fn blank(size: Size, palette: Palette) -> Self {
        Self {
            size,
            layer_id: LayerId(Uuid::new_v4()),
            frame_id: FrameId(Uuid::new_v4()),
            frame_duration_ms: 100,
            palette,
            grid: PixelGrid::transparent(size),
        }
    }
    #[must_use]
    pub fn grid(&self) -> &PixelGrid {
        &self.grid
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PixelChange {
    pub coordinate: Coordinate,
    pub before: Rgba8,
    pub after: Rgba8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditDelta {
    Pixels(Vec<PixelChange>),
    Palette { before: Palette, after: Palette },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrokeTool {
    Pencil(Rgba8),
    Eraser,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum EditError {
    #[error("invalid canvas size")]
    InvalidSize,
    #[error("pixel count does not match canvas size")]
    PixelCount,
    #[error("coordinate is outside canvas: {0:?}")]
    OutOfBounds(Coordinate),
    #[error("palette must contain 2–8 unique opaque entries")]
    InvalidPalette,
    #[error("an edit gesture is already active")]
    GestureActive,
    #[error("no edit gesture is active")]
    NoGesture,
    #[error("document changed while applying history")]
    HistoryConflict,
}

#[derive(Clone, Debug)]
struct Gesture {
    tool: StrokeTool,
    original: PixelGrid,
    last: Coordinate,
}

#[derive(Clone, Debug)]
pub struct EditSession {
    document: RasterDocument,
    undo: VecDeque<EditDelta>,
    redo: Vec<EditDelta>,
    gesture: Option<Gesture>,
    render_epoch: u64,
}

impl EditSession {
    pub fn new(document: RasterDocument) -> Self {
        Self {
            document,
            undo: VecDeque::new(),
            redo: Vec::new(),
            gesture: None,
            render_epoch: 0,
        }
    }
    #[must_use]
    pub fn document(&self) -> &RasterDocument {
        &self.document
    }
    #[must_use]
    pub fn has_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    #[must_use]
    pub const fn render_epoch(&self) -> u64 {
        self.render_epoch
    }

    pub fn begin_stroke(&mut self, at: Coordinate, tool: StrokeTool) -> Result<(), EditError> {
        if self.gesture.is_some() {
            return Err(EditError::GestureActive);
        }
        self.document.grid.index(at)?;
        let original = self.document.grid.clone();
        self.gesture = Some(Gesture {
            tool,
            original,
            last: at,
        });
        self.paint_line(at, at, tool)?;
        Ok(())
    }
    pub fn continue_stroke(&mut self, at: Coordinate) -> Result<(), EditError> {
        let gesture = self.gesture.as_ref().ok_or(EditError::NoGesture)?;
        self.document.grid.index(at)?;
        let (last, tool) = (gesture.last, gesture.tool);
        self.paint_line(last, at, tool)?;
        self.gesture.as_mut().expect("gesture exists").last = at;
        Ok(())
    }
    fn paint_line(
        &mut self,
        from: Coordinate,
        to: Coordinate,
        tool: StrokeTool,
    ) -> Result<(), EditError> {
        let color = match tool {
            StrokeTool::Pencil(c) => c,
            StrokeTool::Eraser => Rgba8::TRANSPARENT,
        };
        for c in bresenham(from, to) {
            self.document.grid.set(c, color)?;
        }
        self.render_epoch = self.render_epoch.wrapping_add(1);
        Ok(())
    }
    pub fn commit_stroke(&mut self) -> Result<bool, EditError> {
        let gesture = self.gesture.take().ok_or(EditError::NoGesture)?;
        let changes = diff(&gesture.original, &self.document.grid);
        if changes.is_empty() {
            return Ok(false);
        }
        self.push(EditDelta::Pixels(changes));
        Ok(true)
    }
    pub fn cancel_stroke(&mut self) -> Result<(), EditError> {
        let gesture = self.gesture.take().ok_or(EditError::NoGesture)?;
        self.document.grid = gesture.original;
        self.render_epoch = self.render_epoch.wrapping_add(1);
        Ok(())
    }
    pub fn fill(&mut self, at: Coordinate, color: Rgba8) -> Result<bool, EditError> {
        if self.gesture.is_some() {
            return Err(EditError::GestureActive);
        }
        let before = self.document.grid.clone();
        let target = before.get(at)?;
        if target == color {
            return Ok(false);
        }
        let mut queue = VecDeque::from([at]);
        while let Some(c) = queue.pop_front() {
            if self.document.grid.get(c)? != target {
                continue;
            }
            self.document.grid.set(c, color)?;
            for n in neighbors(c, self.document.size) {
                if self.document.grid.get(n)? == target {
                    queue.push_back(n);
                }
            }
        }
        let changes = diff(&before, &self.document.grid);
        self.push(EditDelta::Pixels(changes));
        self.render_epoch = self.render_epoch.wrapping_add(1);
        Ok(true)
    }
    pub fn eyedropper(&self, at: Coordinate) -> Result<Option<Rgba8>, EditError> {
        let color = self.document.grid.get(at)?;
        Ok((color.a > 0).then_some(color))
    }
    pub fn set_palette(&mut self, after: Palette) -> Result<bool, EditError> {
        if self.gesture.is_some() {
            return Err(EditError::GestureActive);
        }
        let before = self.document.palette.clone();
        if before == after {
            return Ok(false);
        }
        self.document.palette = after.clone();
        self.push(EditDelta::Palette { before, after });
        Ok(true)
    }
    pub fn undo(&mut self) -> Result<bool, EditError> {
        if let Some(delta) = self.undo.pop_back() {
            apply_delta(&mut self.document, &delta, false)?;
            self.redo.push(delta);
            self.render_epoch = self.render_epoch.wrapping_add(1);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn redo(&mut self) -> Result<bool, EditError> {
        if let Some(delta) = self.redo.pop() {
            apply_delta(&mut self.document, &delta, true)?;
            self.undo.push_back(delta);
            self.render_epoch = self.render_epoch.wrapping_add(1);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn replace_document(&mut self, document: RasterDocument) {
        self.document = document;
        self.gesture = None;
        self.undo.clear();
        self.redo.clear();
        self.render_epoch = self.render_epoch.wrapping_add(1);
    }
    pub fn into_document(self) -> RasterDocument {
        self.document
    }
    fn push(&mut self, delta: EditDelta) {
        if self.undo.len() == 100 {
            self.undo.pop_front();
        }
        self.undo.push_back(delta);
        self.redo.clear();
    }
}

fn apply_delta(
    doc: &mut RasterDocument,
    delta: &EditDelta,
    forward: bool,
) -> Result<(), EditError> {
    match delta {
        EditDelta::Pixels(changes) => {
            if changes.iter().any(|c| {
                doc.grid.get(c.coordinate).ok() != Some(if forward { c.before } else { c.after })
            }) {
                return Err(EditError::HistoryConflict);
            }
            for c in changes {
                doc.grid
                    .set(c.coordinate, if forward { c.after } else { c.before })?;
            }
        }
        EditDelta::Palette { before, after } => {
            let (expected, value) = if forward {
                (before, after)
            } else {
                (after, before)
            };
            if &doc.palette != expected {
                return Err(EditError::HistoryConflict);
            }
            doc.palette = value.clone();
        }
    }
    Ok(())
}

fn diff(before: &PixelGrid, after: &PixelGrid) -> Vec<PixelChange> {
    before
        .pixels
        .iter()
        .zip(&after.pixels)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (&a, &b))| PixelChange {
            coordinate: Coordinate {
                x: (i % before.size.width as usize) as i32,
                y: (i / before.size.width as usize) as i32,
            },
            before: a,
            after: b,
        })
        .collect()
}

fn neighbors(c: Coordinate, size: Size) -> impl Iterator<Item = Coordinate> {
    [
        Coordinate { x: c.x - 1, y: c.y },
        Coordinate { x: c.x + 1, y: c.y },
        Coordinate { x: c.x, y: c.y - 1 },
        Coordinate { x: c.x, y: c.y + 1 },
    ]
    .into_iter()
    .filter(move |p| p.x >= 0 && p.y >= 0 && p.x < size.width as i32 && p.y < size.height as i32)
}

#[must_use]
pub fn bresenham(from: Coordinate, to: Coordinate) -> Vec<Coordinate> {
    let (mut x, mut y) = (from.x, from.y);
    let dx = (to.x - x).abs();
    let sx = if x < to.x { 1 } else { -1 };
    let dy = -(to.y - y).abs();
    let sy = if y < to.y { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = Vec::new();
    loop {
        out.push(Coordinate { x, y });
        if x == to.x && y == to.y {
            break;
        }
        let twice = 2 * err;
        if twice >= dy {
            err += dy;
            x += sx;
        }
        if twice <= dx {
            err += dx;
            y += sy;
        }
    }
    out
}

#[must_use]
pub fn silhouette(source: &PixelGrid, color: Rgba8) -> PixelGrid {
    PixelGrid::from_pixels(
        source.size,
        source
            .pixels
            .iter()
            .map(|p| {
                if p.a == 0 {
                    Rgba8::TRANSPARENT
                } else {
                    Rgba8::new(color.r, color.g, color.b, p.a)
                }
            })
            .collect(),
    )
    .expect("same pixel count")
}
#[must_use]
pub fn flip_horizontal(source: &PixelGrid) -> PixelGrid {
    let mut out = PixelGrid::transparent(source.size);
    for y in 0..source.size.height as i32 {
        for x in 0..source.size.width as i32 {
            let from = Coordinate { x, y };
            let to = Coordinate {
                x: source.size.width as i32 - 1 - x,
                y,
            };
            out.set(to, source.get(from).expect("valid coordinate"))
                .expect("valid coordinate");
        }
    }
    out
}
#[must_use]
pub fn grayscale(source: &PixelGrid) -> PixelGrid {
    PixelGrid::from_pixels(
        source.size,
        source
            .pixels
            .iter()
            .map(|p| {
                if p.a == 0 {
                    return Rgba8::TRANSPARENT;
                }
                let linear = |v: u8| {
                    let s = f32::from(v) / 255.0;
                    if s <= 0.04045 {
                        s / 12.92
                    } else {
                        ((s + 0.055) / 1.055).powf(2.4)
                    }
                };
                let l = 0.2126 * linear(p.r) + 0.7152 * linear(p.g) + 0.0722 * linear(p.b);
                let s = if l <= 0.003_130_8 {
                    12.92 * l
                } else {
                    1.055 * l.powf(1.0 / 2.4) - 0.055
                };
                let v = (s * 255.0).round() as u8;
                Rgba8::new(v, v, v, p.a)
            })
            .collect(),
    )
    .expect("same pixel count")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTransform {
    pub origin: [f32; 2],
    pub integer_zoom: u32,
    pub pixels_per_point: f32,
    pub size: Size,
}
impl CanvasTransform {
    #[must_use]
    pub fn screen_to_pixel(self, point: [f32; 2]) -> Option<Coordinate> {
        if self.integer_zoom == 0 || self.pixels_per_point <= 0.0 {
            return None;
        }
        let cell = self.integer_zoom as f32 / self.pixels_per_point;
        let x = ((point[0] - self.origin[0]) / cell).floor() as i32;
        let y = ((point[1] - self.origin[1]) / cell).floor() as i32;
        (x >= 0 && y >= 0 && x < self.size.width as i32 && y < self.size.height as i32)
            .then_some(Coordinate { x, y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn palette() -> Palette {
        Palette::new(vec![
            PaletteEntry {
                id: PaletteEntryId(Uuid::new_v4()),
                color: Rgba8::BLACK,
            },
            PaletteEntry {
                id: PaletteEntryId(Uuid::new_v4()),
                color: Rgba8::new(255, 255, 255, 255),
            },
        ])
        .unwrap()
    }
    #[test]
    fn tools_palette_colors() {
        let size = Size::new(4, 4).unwrap();
        let mut edit = EditSession::new(RasterDocument::blank(size, palette()));
        edit.begin_stroke(
            Coordinate { x: 0, y: 0 },
            StrokeTool::Pencil(Rgba8::new(3, 4, 5, 127)),
        )
        .unwrap();
        edit.continue_stroke(Coordinate { x: 3, y: 3 }).unwrap();
        assert!(edit.commit_stroke().unwrap());
        assert_eq!(edit.document.grid.used_colors().len(), 1);
        assert_eq!(
            edit.eyedropper(Coordinate { x: 1, y: 1 })
                .unwrap()
                .unwrap()
                .a,
            127
        );
        assert!(edit.fill(Coordinate { x: 0, y: 3 }, Rgba8::BLACK).unwrap());
    }
    #[test]
    fn edit_history_invariants() {
        let mut edit = EditSession::new(RasterDocument::blank(Size::new(4, 4).unwrap(), palette()));
        edit.begin_stroke(Coordinate { x: 0, y: 0 }, StrokeTool::Pencil(Rgba8::BLACK))
            .unwrap();
        edit.continue_stroke(Coordinate { x: 2, y: 0 }).unwrap();
        edit.commit_stroke().unwrap();
        assert_eq!(edit.document.grid.opaque_pixel_count(), 3);
        edit.undo().unwrap();
        assert_eq!(edit.document.grid.opaque_pixel_count(), 0);
        edit.redo().unwrap();
        assert_eq!(edit.document.grid.opaque_pixel_count(), 3);
    }
    #[test]
    fn display_readonly() {
        let mut edit = EditSession::new(RasterDocument::blank(Size::new(2, 1).unwrap(), palette()));
        edit.begin_stroke(
            Coordinate { x: 0, y: 0 },
            StrokeTool::Pencil(Rgba8::new(255, 0, 0, 127)),
        )
        .unwrap();
        edit.commit_stroke().unwrap();
        let original = edit.document.grid.clone();
        assert_eq!(
            silhouette(&original, Rgba8::BLACK)
                .get(Coordinate { x: 0, y: 0 })
                .unwrap()
                .a,
            127
        );
        assert_eq!(
            flip_horizontal(&original)
                .get(Coordinate { x: 1, y: 0 })
                .unwrap()
                .a,
            127
        );
        assert_eq!(original, edit.document.grid);
    }
    #[test]
    fn canvas_half_open() {
        let t = CanvasTransform {
            origin: [10.0, 20.0],
            integer_zoom: 4,
            pixels_per_point: 2.0,
            size: Size::new(16, 16).unwrap(),
        };
        assert_eq!(
            t.screen_to_pixel([10.0, 20.0]),
            Some(Coordinate { x: 0, y: 0 })
        );
        assert_eq!(t.screen_to_pixel([42.0, 20.0]), None);
    }
}
