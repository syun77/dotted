use chrono::{DateTime, Utc};
use pixel_core::{Palette, PixelGrid, RasterDocument, Size};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}
id_type!(ProjectId);
id_type!(SessionId);
id_type!(ArtworkId);
id_type!(ReferenceId);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkRole {
    Copy,
    Memory,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkStatus {
    Draft,
    Finalized,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    InProgress,
    Completed,
    Abandoned,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Setup,
    Observe,
    Copy,
    Memory,
    Validate,
    Compare,
    Reflect,
    Complete,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Background {
    White,
    Black,
    Gray,
    Checker,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceVisibility {
    Hidden,
    Revealed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
    pub size: Size,
    pub initial_palette: Palette,
    pub color_limit: u8,
    pub time_limit_ms: u64,
    pub background: Background,
}
impl Constraints {
    pub fn validate(&self) -> Result<(), TrainingError> {
        if !matches!((self.size.width, self.size.height), (16, 16) | (32, 32))
            || !(2..=8).contains(&self.color_limit)
            || self.time_limit_ms == 0
        {
            Err(TrainingError::InvalidConstraints)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationNotes {
    pub outline: String,
    pub value: String,
    pub identifying_feature: String,
}
impl ObservationNotes {
    fn valid(&self) -> bool {
        [&self.outline, &self.value, &self.identifying_feature]
            .iter()
            .all(|s| !s.trim().is_empty() && s.chars().count() <= 4096)
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reflection {
    pub goal: String,
    pub success: String,
    pub next_try: String,
}
impl Reflection {
    fn valid(&self) -> bool {
        [&self.goal, &self.success, &self.next_try]
            .iter()
            .all(|s| !s.trim().is_empty() && s.chars().count() <= 140)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artwork {
    pub id: ArtworkId,
    pub role: ArtworkRole,
    pub source_artwork_id: Option<ArtworkId>,
    pub status: ArtworkStatus,
    pub document: RasterDocument,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub id: ReferenceId,
    pub image: PixelGrid,
    pub source_title: Option<String>,
    pub creator: Option<String>,
    pub source_url: Option<String>,
    pub usage_note: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageArtifact {
    pub stage: Stage,
    pub artwork_id: ArtworkId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PracticeEventKind {
    StageCompleted {
        completed: Stage,
        artwork_id: Option<ArtworkId>,
        used_colors: Option<u8>,
        opaque_pixels: Option<u32>,
    },
    ReferenceRevealed {
        reference_id: ReferenceId,
    },
    TimeLimitExceeded,
    SessionCompleted,
    SessionAbandoned,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PracticeEvent {
    pub sequence: u16,
    pub at: DateTime<Utc>,
    pub stage: Stage,
    pub elapsed_ms: u64,
    pub kind: PracticeEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PracticeSession {
    pub id: SessionId,
    pub status: SessionStatus,
    pub stage: Stage,
    pub subject: String,
    pub reference_id: ReferenceId,
    pub constraints: Constraints,
    pub observations: ObservationNotes,
    pub artifacts: Vec<StageArtifact>,
    pub reference_visibility: ReferenceVisibility,
    pub elapsed_ms: u64,
    pub events: Vec<PracticeEvent>,
    pub validation_note: String,
    pub reflection_draft: Reflection,
    pub reflection: Option<Reflection>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub artworks: Vec<Artwork>,
    pub references: Vec<Reference>,
    pub sessions: Vec<PracticeSession>,
}

impl Project {
    pub fn new(
        title: String,
        subject: String,
        constraints: Constraints,
        reference: Reference,
        now: DateTime<Utc>,
    ) -> Result<Self, TrainingError> {
        constraints.validate()?;
        validate_text(&title, 4096)?;
        validate_text(&subject, 4096)?;
        let session = PracticeSession {
            id: SessionId::new(),
            status: SessionStatus::InProgress,
            stage: Stage::Setup,
            subject,
            reference_id: reference.id,
            constraints,
            observations: ObservationNotes::default(),
            artifacts: vec![],
            reference_visibility: ReferenceVisibility::Revealed,
            elapsed_ms: 0,
            events: vec![],
            validation_note: String::new(),
            reflection_draft: Reflection::default(),
            reflection: None,
        };
        Ok(Self {
            id: ProjectId::new(),
            title,
            created_at: now,
            updated_at: now,
            artworks: vec![],
            references: vec![reference],
            sessions: vec![session],
        })
    }
    #[must_use]
    pub fn session(&self) -> &PracticeSession {
        &self.sessions[0]
    }
    pub fn session_mut(&mut self) -> &mut PracticeSession {
        &mut self.sessions[0]
    }
    #[must_use]
    pub fn active_artwork(&self) -> Option<&Artwork> {
        let id = self
            .session()
            .artifacts
            .iter()
            .find(|a| a.stage == self.session().stage)
            .map(|a| a.artwork_id)?;
        self.artworks.iter().find(|a| a.id == id)
    }
    pub fn replace_draft_document(
        &mut self,
        artwork_id: ArtworkId,
        document: RasterDocument,
        now: DateTime<Utc>,
    ) -> Result<(), TrainingError> {
        let art = self
            .artworks
            .iter_mut()
            .find(|a| a.id == artwork_id)
            .ok_or(TrainingError::ArtworkNotFound)?;
        if art.status != ArtworkStatus::Draft || art.document.size != document.size {
            return Err(TrainingError::Finalized);
        }
        art.document = document;
        self.updated_at = now;
        Ok(())
    }
}

pub enum SessionCommand {
    StartObservation,
    SetObservations(ObservationNotes),
    CompleteObservation,
    CompleteCopy,
    RevealReference,
    HideReference,
    CompleteMemory,
    SetValidationNote(String),
    CompleteValidation,
    CompleteComparison,
    SetReflectionDraft(Reflection),
    CompleteReflection(Reflection),
    AddElapsed(u64),
    Abandon,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TrainingError {
    #[error("invalid constraints")]
    InvalidConstraints,
    #[error("invalid text")]
    InvalidText,
    #[error("command is not allowed in this stage")]
    InvalidTransition,
    #[error("artwork not found")]
    ArtworkNotFound,
    #[error("finalized artwork cannot be edited")]
    Finalized,
    #[error("an edit gesture must finish first")]
    GestureActive,
    #[error("event limit reached")]
    EventLimit,
}

pub fn apply_session_command(
    project: &mut Project,
    command: SessionCommand,
    now: DateTime<Utc>,
    gesture_active: bool,
) -> Result<bool, TrainingError> {
    if gesture_active {
        return Err(TrainingError::GestureActive);
    }
    let mut next = project.clone();
    apply(&mut next, command, now)?;
    if &next == project {
        return Ok(false);
    }
    next.updated_at = now;
    *project = next;
    Ok(true)
}

fn apply(
    project: &mut Project,
    command: SessionCommand,
    now: DateTime<Utc>,
) -> Result<(), TrainingError> {
    if project.session().status != SessionStatus::InProgress {
        return Err(TrainingError::InvalidTransition);
    }
    match command {
        SessionCommand::StartObservation if project.session().stage == Stage::Setup => {
            project.session().constraints.validate()?;
            project.session_mut().stage = Stage::Observe;
        }
        SessionCommand::SetObservations(notes) if project.session().stage == Stage::Observe => {
            if !notes.valid() {
                return Err(TrainingError::InvalidText);
            }
            project.session_mut().observations = notes;
        }
        SessionCommand::CompleteObservation if project.session().stage == Stage::Observe => {
            if !project.session().observations.valid() {
                return Err(TrainingError::InvalidText);
            }
            let id = ArtworkId::new();
            let doc = RasterDocument::blank(
                project.session().constraints.size,
                project.session().constraints.initial_palette.clone(),
            );
            project.artworks.push(Artwork {
                id,
                role: ArtworkRole::Copy,
                source_artwork_id: None,
                status: ArtworkStatus::Draft,
                document: doc,
            });
            complete_stage(project, Stage::Observe, None, now)?;
            project.session_mut().artifacts.push(StageArtifact {
                stage: Stage::Copy,
                artwork_id: id,
            });
            project.session_mut().stage = Stage::Copy;
        }
        SessionCommand::CompleteCopy if project.session().stage == Stage::Copy => {
            let copy_id = artifact_id(project, Stage::Copy)?;
            let copy = project
                .artworks
                .iter_mut()
                .find(|a| a.id == copy_id)
                .ok_or(TrainingError::ArtworkNotFound)?;
            copy.status = ArtworkStatus::Finalized;
            let palette = copy.document.palette.clone();
            let size = project.session().constraints.size;
            let memory_id = ArtworkId::new();
            let document = new_memory_blank(size, palette);
            project.artworks.push(Artwork {
                id: memory_id,
                role: ArtworkRole::Memory,
                source_artwork_id: Some(copy_id),
                status: ArtworkStatus::Draft,
                document,
            });
            complete_stage(project, Stage::Copy, Some(copy_id), now)?;
            let s = project.session_mut();
            s.artifacts.push(StageArtifact {
                stage: Stage::Memory,
                artwork_id: memory_id,
            });
            s.stage = Stage::Memory;
            s.reference_visibility = ReferenceVisibility::Hidden;
        }
        SessionCommand::RevealReference if project.session().stage == Stage::Memory => {
            if project.session().reference_visibility == ReferenceVisibility::Hidden {
                let reference_id = project.session().reference_id;
                project.session_mut().reference_visibility = ReferenceVisibility::Revealed;
                push_event(
                    project,
                    now,
                    PracticeEventKind::ReferenceRevealed { reference_id },
                )?;
            }
        }
        SessionCommand::HideReference if project.session().stage == Stage::Memory => {
            project.session_mut().reference_visibility = ReferenceVisibility::Hidden;
        }
        SessionCommand::CompleteMemory if project.session().stage == Stage::Memory => {
            let id = artifact_id(project, Stage::Memory)?;
            project
                .artworks
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or(TrainingError::ArtworkNotFound)?
                .status = ArtworkStatus::Finalized;
            complete_stage(project, Stage::Memory, Some(id), now)?;
            let s = project.session_mut();
            s.stage = Stage::Validate;
            s.reference_visibility = ReferenceVisibility::Revealed;
        }
        SessionCommand::SetValidationNote(text) if project.session().stage == Stage::Validate => {
            validate_text(&text, 4096)?;
            project.session_mut().validation_note = text;
        }
        SessionCommand::CompleteValidation if project.session().stage == Stage::Validate => {
            complete_stage(project, Stage::Validate, None, now)?;
            project.session_mut().stage = Stage::Compare;
        }
        SessionCommand::CompleteComparison if project.session().stage == Stage::Compare => {
            complete_stage(project, Stage::Compare, None, now)?;
            project.session_mut().stage = Stage::Reflect;
        }
        SessionCommand::SetReflectionDraft(value) if project.session().stage == Stage::Reflect => {
            if [&value.goal, &value.success, &value.next_try]
                .iter()
                .any(|s| s.chars().count() > 140)
            {
                return Err(TrainingError::InvalidText);
            }
            project.session_mut().reflection_draft = value;
        }
        SessionCommand::CompleteReflection(value) if project.session().stage == Stage::Reflect => {
            if !value.valid() {
                return Err(TrainingError::InvalidText);
            }
            project.session_mut().reflection = Some(value);
            complete_stage(project, Stage::Reflect, None, now)?;
            let s = project.session_mut();
            s.stage = Stage::Complete;
            s.status = SessionStatus::Completed;
            push_event(project, now, PracticeEventKind::SessionCompleted)?;
        }
        SessionCommand::AddElapsed(ms) => {
            let before = project.session().elapsed_ms;
            let limit = project.session().constraints.time_limit_ms;
            project.session_mut().elapsed_ms = before.saturating_add(ms);
            let already = project
                .session()
                .events
                .iter()
                .any(|e| matches!(e.kind, PracticeEventKind::TimeLimitExceeded));
            if before < limit && project.session().elapsed_ms >= limit && !already {
                push_event(project, now, PracticeEventKind::TimeLimitExceeded)?;
            }
        }
        SessionCommand::Abandon => {
            project.session_mut().status = SessionStatus::Abandoned;
            push_event(project, now, PracticeEventKind::SessionAbandoned)?;
        }
        _ => return Err(TrainingError::InvalidTransition),
    }
    Ok(())
}

#[must_use]
pub fn new_memory_blank(size: Size, palette: Palette) -> RasterDocument {
    RasterDocument::blank(size, palette)
}
fn artifact_id(project: &Project, stage: Stage) -> Result<ArtworkId, TrainingError> {
    project
        .session()
        .artifacts
        .iter()
        .find(|a| a.stage == stage)
        .map(|a| a.artwork_id)
        .ok_or(TrainingError::ArtworkNotFound)
}
fn complete_stage(
    project: &mut Project,
    completed: Stage,
    artwork_id: Option<ArtworkId>,
    now: DateTime<Utc>,
) -> Result<(), TrainingError> {
    let metrics = artwork_id
        .and_then(|id| project.artworks.iter().find(|a| a.id == id))
        .map(|a| {
            (
                a.document.grid().used_colors().len() as u8,
                a.document.grid().opaque_pixel_count() as u32,
            )
        });
    push_event(
        project,
        now,
        PracticeEventKind::StageCompleted {
            completed,
            artwork_id,
            used_colors: metrics.map(|m| m.0),
            opaque_pixels: metrics.map(|m| m.1),
        },
    )
}
fn push_event(
    project: &mut Project,
    at: DateTime<Utc>,
    kind: PracticeEventKind,
) -> Result<(), TrainingError> {
    let s = project.session_mut();
    if s.events.len() >= 256 {
        return Err(TrainingError::EventLimit);
    }
    s.events.push(PracticeEvent {
        sequence: s.events.len() as u16,
        at,
        stage: s.stage,
        elapsed_ms: s.elapsed_ms,
        kind,
    });
    Ok(())
}
fn validate_text(value: &str, max: usize) -> Result<(), TrainingError> {
    if value.chars().count() > max {
        Err(TrainingError::InvalidText)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixel_core::{PaletteEntry, PaletteEntryId, Rgba8};
    fn project() -> Project {
        let palette = Palette::new(vec![
            PaletteEntry {
                id: PaletteEntryId(Uuid::new_v4()),
                color: Rgba8::BLACK,
            },
            PaletteEntry {
                id: PaletteEntryId(Uuid::new_v4()),
                color: Rgba8::new(255, 255, 255, 255),
            },
        ])
        .unwrap();
        let size = Size::new(16, 16).unwrap();
        Project::new(
            "練習".into(),
            "りんご".into(),
            Constraints {
                size,
                initial_palette: palette,
                color_limit: 4,
                time_limit_ms: 1000,
                background: Background::Checker,
            },
            Reference {
                id: ReferenceId::new(),
                image: PixelGrid::transparent(size),
                source_title: Some("同梱サンプル".into()),
                creator: None,
                source_url: None,
                usage_note: Some("練習用".into()),
            },
            Utc::now(),
        )
        .unwrap()
    }
    #[test]
    fn setup_constraints() {
        assert!(project().session().constraints.validate().is_ok());
    }
    #[test]
    fn memory_atomic_blank() {
        let now = Utc::now();
        let mut p = project();
        apply_session_command(&mut p, SessionCommand::StartObservation, now, false).unwrap();
        apply_session_command(
            &mut p,
            SessionCommand::SetObservations(ObservationNotes {
                outline: "丸い".into(),
                value: "暗部".into(),
                identifying_feature: "葉".into(),
            }),
            now,
            false,
        )
        .unwrap();
        apply_session_command(&mut p, SessionCommand::CompleteObservation, now, false).unwrap();
        let copy = p.active_artwork().unwrap().id;
        apply_session_command(&mut p, SessionCommand::CompleteCopy, now, false).unwrap();
        let memory = p.active_artwork().unwrap();
        assert_ne!(copy, memory.id);
        assert!(memory.document.grid().is_transparent());
        assert_eq!(memory.source_artwork_id, Some(copy));
        assert_eq!(
            p.session().reference_visibility,
            ReferenceVisibility::Hidden
        );
    }
    #[test]
    fn reflection_timer() {
        let now = Utc::now();
        let mut p = project();
        apply_session_command(&mut p, SessionCommand::AddElapsed(1200), now, false).unwrap();
        apply_session_command(&mut p, SessionCommand::AddElapsed(1200), now, false).unwrap();
        assert_eq!(
            p.session()
                .events
                .iter()
                .filter(|e| matches!(e.kind, PracticeEventKind::TimeLimitExceeded))
                .count(),
            1
        );
    }
}
