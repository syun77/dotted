use chrono::Utc;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use pixel_core::{
    CanvasTransform, EditSession, Palette, PaletteEntry, PaletteEntryId, PixelGrid, Rgba8, Size,
    StrokeTool, flip_horizontal, grayscale, silhouette,
};
use project_io::{ExportScale, export_png, load_project, save_project};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use training::{
    ArtworkId, Background, Constraints, ObservationNotes, Project, Reference, ReferenceId,
    ReferenceVisibility, Reflection, SessionCommand, Stage, apply_session_command,
};
use uuid::Uuid;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([820.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Dotted Training Studio",
        options,
        Box::new(|cc| Ok(Box::new(DottedApp::new(cc)))),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tool {
    Pencil,
    Eraser,
    Fill,
    Eyedropper,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DisplayMode {
    Normal,
    Silhouette,
    Grayscale,
    Flip,
}

struct DottedApp {
    project: Option<Project>,
    edit: Option<(ArtworkId, EditSession)>,
    tool: Tool,
    selected: Rgba8,
    zoom: u32,
    display: DisplayMode,
    status: String,
    save_path: Option<PathBuf>,
    revision: u64,
    saved_revision: u64,
    last_change: Instant,
    last_recovery: Instant,
    open_generation: Uuid,
    observations: ObservationNotes,
    validation_note: String,
    reflection: Reflection,
    compare_blink: bool,
}

impl DottedApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let japanese_font_loaded = configure_japanese_font(&cc.egui_ctx);
        Self {
            project: None,
            edit: None,
            tool: Tool::Pencil,
            selected: Rgba8::BLACK,
            zoom: 24,
            display: DisplayMode::Normal,
            status: if japanese_font_loaded {
                "準備完了".into()
            } else {
                "日本語フォントが見つかりません".into()
            },
            save_path: None,
            revision: 0,
            saved_revision: 0,
            last_change: Instant::now(),
            last_recovery: Instant::now(),
            open_generation: Uuid::new_v4(),
            observations: ObservationNotes::default(),
            validation_note: String::new(),
            reflection: Reflection::default(),
            compare_blink: false,
        }
    }

    fn default_project() -> Project {
        let size = Size::new(16, 16).expect("fixed size");
        let colors = [
            Rgba8::new(30, 30, 40, 255),
            Rgba8::new(232, 77, 62, 255),
            Rgba8::new(248, 190, 70, 255),
            Rgba8::new(245, 242, 225, 255),
        ];
        let palette = Palette::new(
            colors
                .into_iter()
                .map(|color| PaletteEntry {
                    id: PaletteEntryId(Uuid::new_v4()),
                    color,
                })
                .collect(),
        )
        .expect("fixed palette");
        let mut pixels = vec![Rgba8::TRANSPARENT; size.len()];
        for y in 3_i32..14 {
            for x in 3_i32..13 {
                if (x - 8).abs() + (y - 8).abs() < 8 {
                    pixels[(y * 16 + x) as usize] = colors[1];
                }
            }
        }
        let reference = Reference {
            id: ReferenceId::new(),
            image: PixelGrid::from_pixels(size, pixels).expect("fixed reference"),
            source_title: Some("Dotted bundled apple study".into()),
            creator: Some("Dotted Training Studio".into()),
            source_url: None,
            usage_note: Some("同梱の練習用素材".into()),
        };
        Project::new(
            "はじめてのドット絵".into(),
            "りんご".into(),
            Constraints {
                size,
                initial_palette: palette,
                color_limit: 4,
                time_limit_ms: 30 * 60 * 1000,
                background: Background::Checker,
            },
            reference,
            Utc::now(),
        )
        .expect("fixed project")
    }

    fn mark_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.last_change = Instant::now();
    }
    fn transition(&mut self, command: SessionCommand) {
        if let Some(project) = &mut self.project {
            let gesture = self.edit.as_ref().is_some_and(|(_, e)| e.has_gesture());
            match apply_session_command(project, command, Utc::now(), gesture) {
                Ok(true) => {
                    self.mark_changed();
                    self.sync_edit();
                    self.status = "段階を更新しました".into();
                }
                Ok(false) => {}
                Err(e) => self.status = e.to_string(),
            }
        }
    }
    fn sync_edit(&mut self) {
        let active = self
            .project
            .as_ref()
            .and_then(Project::active_artwork)
            .filter(|a| a.status == training::ArtworkStatus::Draft);
        match active {
            Some(a) if self.edit.as_ref().map(|e| e.0) != Some(a.id) => {
                self.edit = Some((a.id, EditSession::new(a.document.clone())))
            }
            None => self.edit = None,
            _ => {}
        }
    }
    fn commit_edit(&mut self) {
        if let (Some(project), Some((id, edit))) = (&mut self.project, &self.edit) {
            if let Err(e) = project.replace_draft_document(*id, edit.document().clone(), Utc::now())
            {
                self.status = e.to_string();
            } else {
                self.mark_changed();
            }
        }
    }
    fn displayed_grid(&self) -> Option<PixelGrid> {
        let grid = self
            .edit
            .as_ref()
            .map(|(_, e)| e.document().grid())
            .or_else(|| {
                self.project
                    .as_ref()?
                    .active_artwork()
                    .map(|a| a.document.grid())
            })?;
        Some(match self.display {
            DisplayMode::Normal => grid.clone(),
            DisplayMode::Silhouette => silhouette(grid, Rgba8::BLACK),
            DisplayMode::Grayscale => grayscale(grid),
            DisplayMode::Flip => flip_horizontal(grid),
        })
    }
    fn save_as(&mut self) {
        if let Some(project) = &self.project {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Dotted project", &["dotted"])
                .save_file()
            {
                match save_project(&path, project) {
                    Ok(()) => {
                        self.save_path = Some(path);
                        self.saved_revision = self.revision;
                        self.status = "保存しました".into();
                    }
                    Err(e) => self.status = e.to_string(),
                }
            }
        }
    }
    fn open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Dotted project", &["dotted"])
            .pick_file()
        {
            match load_project(&path) {
                Ok(project) => {
                    self.project = Some(project);
                    self.save_path = Some(path);
                    self.revision = 0;
                    self.saved_revision = 0;
                    self.open_generation = Uuid::new_v4();
                    self.sync_edit();
                    self.status = "読み込みました".into();
                }
                Err(e) => self.status = e.to_string(),
            }
        }
    }
    fn export(&mut self, scale: ExportScale) {
        let Some(grid) = self
            .project
            .as_ref()
            .and_then(Project::active_artwork)
            .map(|a| a.document.grid())
        else {
            return;
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .save_file()
        {
            match export_png(&path, grid, scale) {
                Ok(()) => self.status = "PNGを書き出しました".into(),
                Err(e) => self.status = e.to_string(),
            }
        }
    }
    fn autosave(&mut self) {
        if self.project.is_none()
            || self.revision == self.saved_revision
            || self.last_change.elapsed() < Duration::from_secs(1)
            || self.last_recovery.elapsed() < Duration::from_secs(1)
        {
            return;
        }
        let Some(root) = dirs_path() else { return };
        if let Some(project) = &self.project {
            match project_io::save_recovery(&root, self.open_generation, self.revision, project) {
                Ok(_) => {
                    self.last_recovery = Instant::now();
                    self.status = "復旧用に保存済み".into();
                }
                Err(e) => self.status = format!("自動保存失敗: {e}"),
            }
        }
    }
}

impl eframe::App for DottedApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.autosave();
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("新規").clicked() {
                    self.project = Some(Self::default_project());
                    self.edit = None;
                    self.sync_edit();
                    self.mark_changed();
                }
                if ui.button("開く…").clicked() {
                    self.open();
                }
                if ui.button("保存…").clicked() {
                    self.save_as();
                }
                ui.menu_button("PNG出力", |ui| {
                    for (label, s) in [
                        ("原寸", ExportScale::Original),
                        ("2×", ExportScale::X2),
                        ("4×", ExportScale::X4),
                        ("8×", ExportScale::X8),
                    ] {
                        if ui.button(label).clicked() {
                            self.export(s);
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                ui.label(if self.revision == self.saved_revision {
                    "保存済み"
                } else {
                    "未保存"
                });
                ui.label(&self.status);
            });
        });
        if self.project.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(180.0);
                    ui.heading("Dotted Training Studio");
                    ui.label("模写して、隠して、白紙から思い出すドット絵練習");
                    ui.add_space(18.0);
                    if ui.button("🍎 16×16・4色・30分の練習を始める").clicked() {
                        self.project = Some(Self::default_project());
                        self.mark_changed();
                    }
                });
            });
            return;
        }
        self.workspace(ctx);
    }
}

impl DottedApp {
    fn workspace(&mut self, ctx: &egui::Context) {
        let stage = self.project.as_ref().unwrap().session().stage;
        egui::SidePanel::left("left")
            .resizable(true)
            .default_width(230.0)
            .show(ctx, |ui| {
                ui.heading(format!("{:?}", stage));
                ui.label(&self.project.as_ref().unwrap().session().subject);
                ui.separator();
                self.stage_controls(ui, stage);
            });
        egui::SidePanel::right("preview")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("1× Preview");
                if let Some(grid) = self.displayed_grid() {
                    paint_grid(ui, &grid, 1, false);
                }
                ui.separator();
                ui.label("表示診断（保存画素は不変）");
                ui.selectable_value(&mut self.display, DisplayMode::Normal, "通常");
                ui.selectable_value(&mut self.display, DisplayMode::Silhouette, "シルエット");
                ui.selectable_value(&mut self.display, DisplayMode::Grayscale, "グレースケール");
                ui.selectable_value(&mut self.display, DisplayMode::Flip, "左右反転");
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            if matches!(stage, Stage::Copy | Stage::Memory) {
                self.editor_ui(ui);
            } else if stage == Stage::Compare {
                self.compare_ui(ui, ctx);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading(next_action(stage));
                });
            }
        });
    }
    fn stage_controls(&mut self, ui: &mut egui::Ui, stage: Stage) {
        match stage {
            Stage::Setup => {
                if ui.button("観察を開始").clicked() {
                    self.transition(SessionCommand::StartObservation);
                }
            }
            Stage::Observe => {
                ui.label("外形");
                ui.text_edit_singleline(&mut self.observations.outline);
                ui.label("明暗");
                ui.text_edit_singleline(&mut self.observations.value);
                ui.label("識別点");
                ui.text_edit_singleline(&mut self.observations.identifying_feature);
                if ui.button("メモを確定").clicked() {
                    self.transition(SessionCommand::SetObservations(self.observations.clone()));
                }
                if ui.button("模写へ").clicked() {
                    self.transition(SessionCommand::SetObservations(self.observations.clone()));
                    self.transition(SessionCommand::CompleteObservation);
                }
            }
            Stage::Copy => {
                self.reference_ui(ui);
                if ui.button("模写を確定してMemoryへ").clicked() {
                    self.transition(SessionCommand::CompleteCopy);
                }
            }
            Stage::Memory => {
                if self
                    .project
                    .as_ref()
                    .unwrap()
                    .session()
                    .reference_visibility
                    == ReferenceVisibility::Hidden
                {
                    ui.label("参照と模写は隠れています");
                    if ui.button("参照を再表示").clicked() {
                        self.transition(SessionCommand::RevealReference);
                    }
                } else {
                    self.reference_ui(ui);
                    if ui.button("もう一度隠す").clicked() {
                        self.transition(SessionCommand::HideReference);
                    }
                }
                if ui.button("記憶描きを確定").clicked() {
                    self.transition(SessionCommand::CompleteMemory);
                }
            }
            Stage::Validate => {
                ui.label("等倍・シルエット・明度を確認してください");
                ui.text_edit_multiline(&mut self.validation_note);
                if ui.button("検証を完了").clicked() {
                    self.transition(SessionCommand::SetValidationNote(
                        self.validation_note.clone(),
                    ));
                    self.transition(SessionCommand::CompleteValidation);
                }
            }
            Stage::Compare => {
                if ui.button("比較を完了").clicked() {
                    self.transition(SessionCommand::CompleteComparison);
                }
            }
            Stage::Reflect => {
                ui.label("今回の狙い");
                ui.text_edit_multiline(&mut self.reflection.goal);
                ui.label("できたこと");
                ui.text_edit_multiline(&mut self.reflection.success);
                ui.label("次に1つ試すこと");
                ui.text_edit_multiline(&mut self.reflection.next_try);
                if ui.button("振り返りを完了").clicked() {
                    self.transition(SessionCommand::CompleteReflection(self.reflection.clone()));
                }
            }
            Stage::Complete => {
                ui.heading("練習完了");
                ui.label("おつかれさまでした。次に試すことを履歴に残しました。");
            }
        }
    }
    fn reference_ui(&self, ui: &mut egui::Ui) {
        if let Some(reference) = self.project.as_ref().and_then(|p| p.references.first()) {
            ui.heading("参照");
            paint_grid(ui, &reference.image, 8, false);
            if let Some(title) = &reference.source_title {
                ui.small(title);
            }
        }
    }
    fn editor_ui(&mut self, ui: &mut egui::Ui) {
        if !ui
            .ctx()
            .input(|input| input.viewport().focused.unwrap_or(true))
        {
            if let Some((_, edit)) = &mut self.edit {
                if edit.has_gesture() {
                    let _ = edit.cancel_stroke();
                }
            }
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tool, Tool::Pencil, "Pencil");
            ui.selectable_value(&mut self.tool, Tool::Eraser, "Eraser");
            ui.selectable_value(&mut self.tool, Tool::Fill, "Fill");
            ui.selectable_value(&mut self.tool, Tool::Eyedropper, "Eyedropper");
            if ui.button("Undo").clicked() {
                if let Some((_, e)) = &mut self.edit {
                    if e.undo().unwrap_or(false) {
                        self.commit_edit();
                    }
                }
            }
            if ui.button("Redo").clicked() {
                if let Some((_, e)) = &mut self.edit {
                    if e.redo().unwrap_or(false) {
                        self.commit_edit();
                    }
                }
            }
            ui.add(
                egui::Slider::new(&mut self.zoom, 4..=32)
                    .integer()
                    .text("Zoom"),
            );
        });
        if let Some((_, edit)) = &self.edit {
            ui.horizontal_wrapped(|ui| {
                for entry in edit.document().palette.entries() {
                    let c = color32(entry.color);
                    if ui.add(egui::Button::new("  ").fill(c)).clicked() {
                        self.selected = entry.color;
                    }
                }
                let used = edit.document().grid().used_colors().len();
                let limit = self
                    .project
                    .as_ref()
                    .unwrap()
                    .session()
                    .constraints
                    .color_limit;
                ui.colored_label(
                    if used > limit as usize {
                        Color32::RED
                    } else {
                        Color32::GRAY
                    },
                    format!("色 {used}/{limit}"),
                );
            });
        }
        let Some(grid) = self.displayed_grid() else {
            return;
        };
        let size = grid.size();
        let pixels_per_point = ui.ctx().pixels_per_point();
        let cell_points = self.zoom as f32 / pixels_per_point;
        let desired = Vec2::new(
            size.width as f32 * cell_points,
            size.height as f32 * cell_points,
        );
        let (response, painter) = ui.allocate_painter(desired, Sense::click_and_drag());
        paint_checker(&painter, response.rect, cell_points);
        paint_pixels(&painter, response.rect, &grid, cell_points, true);
        if self.display == DisplayMode::Flip {
            return;
        }
        let transform = CanvasTransform {
            origin: [response.rect.min.x, response.rect.min.y],
            integer_zoom: self.zoom,
            pixels_per_point,
            size,
        };
        if response.drag_started() {
            if let Some(pos) = response
                .interact_pointer_pos()
                .and_then(|p| transform.screen_to_pixel([p.x, p.y]))
            {
                match self.tool {
                    Tool::Pencil | Tool::Eraser => {
                        let tool = if self.tool == Tool::Pencil {
                            StrokeTool::Pencil(self.selected)
                        } else {
                            StrokeTool::Eraser
                        };
                        if let Some((_, e)) = &mut self.edit {
                            let _ = e.begin_stroke(pos, tool);
                        }
                    }
                    Tool::Fill => {
                        if let Some((_, e)) = &mut self.edit {
                            if e.fill(pos, self.selected).unwrap_or(false) {
                                self.commit_edit();
                            }
                        }
                    }
                    Tool::Eyedropper => {
                        if let Some((_, e)) = &self.edit {
                            if let Ok(Some(c)) = e.eyedropper(pos) {
                                self.selected = c;
                            }
                        }
                    }
                }
            }
        }
        if response.dragged() {
            if let Some(pos) = response
                .interact_pointer_pos()
                .and_then(|p| transform.screen_to_pixel([p.x, p.y]))
            {
                if let Some((_, e)) = &mut self.edit {
                    if e.has_gesture() {
                        let _ = e.continue_stroke(pos);
                    }
                }
            }
        }
        if response.drag_stopped() {
            if let Some((_, e)) = &mut self.edit {
                if e.has_gesture() && e.commit_stroke().unwrap_or(false) {
                    self.commit_edit();
                }
            }
        }
        if response.clicked() {
            if let Some(pos) = response
                .interact_pointer_pos()
                .and_then(|p| transform.screen_to_pixel([p.x, p.y]))
            {
                match self.tool {
                    Tool::Fill => {
                        if let Some((_, e)) = &mut self.edit {
                            if e.fill(pos, self.selected).unwrap_or(false) {
                                self.commit_edit();
                            }
                        }
                    }
                    Tool::Eyedropper => {
                        if let Some((_, e)) = &self.edit {
                            if let Ok(Some(c)) = e.eyedropper(pos) {
                                self.selected = c;
                            }
                        }
                    }
                    Tool::Pencil | Tool::Eraser => {
                        let tool = if self.tool == Tool::Pencil {
                            StrokeTool::Pencil(self.selected)
                        } else {
                            StrokeTool::Eraser
                        };
                        if let Some((_, e)) = &mut self.edit {
                            if e.begin_stroke(pos, tool).is_ok() {
                                let _ = e.commit_stroke();
                                self.commit_edit();
                            }
                        }
                    }
                }
            }
        }
    }
    fn compare_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ctx.request_repaint_after(Duration::from_millis(500));
        self.compare_blink = ctx.input(|i| (i.time * 2.0) as u64 % 2 == 0);
        let arts = &self.project.as_ref().unwrap().artworks;
        ui.horizontal(|ui| {
            for art in arts {
                ui.vertical(|ui| {
                    ui.heading(format!("{:?}", art.role));
                    paint_grid(ui, art.document.grid(), 12, false);
                });
            }
        });
        ui.separator();
        ui.label("A/B 点滅");
        if let Some(art) = arts.get(if self.compare_blink { 0 } else { 1 }) {
            paint_grid(ui, art.document.grid(), 12, false);
        }
    }
}

fn color32(c: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}
fn paint_grid(ui: &mut egui::Ui, grid: &PixelGrid, zoom: u32, lines: bool) {
    let cell_points = zoom as f32 / ui.ctx().pixels_per_point();
    let desired = Vec2::new(
        grid.size().width as f32 * cell_points,
        grid.size().height as f32 * cell_points,
    );
    let (_, p) = ui.allocate_painter(desired, Sense::hover());
    let rect = p.clip_rect();
    paint_checker(&p, rect, cell_points);
    paint_pixels(&p, rect, grid, cell_points, lines);
}
fn paint_checker(p: &egui::Painter, rect: Rect, cell: f32) {
    for y in 0..((rect.height() / cell).ceil() as i32) {
        for x in 0..((rect.width() / cell).ceil() as i32) {
            let c = if (x + y) % 2 == 0 {
                Color32::from_gray(210)
            } else {
                Color32::from_gray(165)
            };
            p.rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.min.x + x as f32 * cell, rect.min.y + y as f32 * cell),
                    Vec2::splat(cell),
                ),
                0.0,
                c,
            );
        }
    }
}
fn paint_pixels(p: &egui::Painter, rect: Rect, grid: &PixelGrid, cell: f32, lines: bool) {
    let size = grid.size();
    for y in 0..size.height {
        for x in 0..size.width {
            let c = grid.pixels()[(y * size.width + x) as usize];
            if c.a > 0 {
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.min.x + x as f32 * cell, rect.min.y + y as f32 * cell),
                        Vec2::splat(cell),
                    ),
                    0.0,
                    color32(c),
                );
            }
        }
    }
    if lines && cell >= 8.0 {
        for x in 0..=size.width {
            let px = rect.min.x + x as f32 * cell;
            p.line_segment(
                [
                    Pos2::new(px, rect.min.y),
                    Pos2::new(px, rect.min.y + size.height as f32 * cell),
                ],
                Stroke::new(0.5, Color32::from_black_alpha(80)),
            );
        }
        for y in 0..=size.height {
            let py = rect.min.y + y as f32 * cell;
            p.line_segment(
                [
                    Pos2::new(rect.min.x, py),
                    Pos2::new(rect.min.x + size.width as f32 * cell, py),
                ],
                Stroke::new(0.5, Color32::from_black_alpha(80)),
            );
        }
    }
}
fn next_action(stage: Stage) -> &'static str {
    match stage {
        Stage::Setup => "観察を開始しましょう",
        Stage::Observe => "外形・明暗・識別点を言葉にします",
        Stage::Validate => "表示診断を切り替えて確認します",
        Stage::Reflect => "3つの短い振り返りを記録します",
        Stage::Complete => "練習完了",
        _ => "次の操作を選んでください",
    }
}
fn dirs_path() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) {
        "LOCALAPPDATA"
    } else {
        "HOME"
    })
    .map(PathBuf::from)
    .map(|p| {
        if cfg!(windows) {
            p.join("Dotted").join("recovery")
        } else {
            p.join("Library")
                .join("Application Support")
                .join("Dotted")
                .join("recovery")
        }
    })
}

fn configure_japanese_font(ctx: &egui::Context) -> bool {
    let Some(path) = find_japanese_font() else {
        return false;
    };
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let mut fonts = egui::FontDefinitions::default();
    const NAME: &str = "dotted_japanese";
    fonts
        .font_data
        .insert(NAME.to_owned(), egui::FontData::from_owned(bytes).into());
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, NAME.to_owned());
    }
    ctx.set_fonts(fonts);
    true
}

fn find_japanese_font() -> Option<PathBuf> {
    let fixed = if cfg!(target_os = "windows") {
        [
            Path::new(r"C:\Windows\Fonts\YuGothR.ttc"),
            Path::new(r"C:\Windows\Fonts\meiryo.ttc"),
            Path::new(r"C:\Windows\Fonts\msgothic.ttc"),
        ]
        .into_iter()
        .find(|path| path.is_file())
    } else {
        [
            Path::new("/System/Library/Fonts/Hiragino Sans GB.ttc"),
            Path::new("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"),
            Path::new("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        ]
        .into_iter()
        .find(|path| path.is_file())
    };
    fixed.map(Path::to_path_buf).or_else(find_macos_hiragino)
}

fn find_macos_hiragino() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    fs::read_dir("/System/Library/Fonts")
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("ヒラ") && name.contains("W3.ttc"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_japanese_font_is_available() {
        assert!(find_japanese_font().is_some());
    }
}
