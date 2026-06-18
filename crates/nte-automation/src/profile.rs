use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{Point, Rect, Size};

const PROFILE_SCHEMA: &str = "nte-gacha-auto-profile";
const PROFILE_SCHEMA_VERSION: u32 = 2;
const SUPPORTED_ASPECT_RATIO: &str = "16:9";
const ASPECT_RATIO_TOLERANCE: f64 = 0.01;
const DEFAULT_PROFILE_JSON: &str = include_str!("../assets/default.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSpec {
    pub exe: String,
    pub class_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSpec {
    pub file: String,
    pub rect: Rect,
    pub search_padding: Point,
    #[serde(default)]
    pub threshold: f32,
}

impl TemplateSpec {
    pub fn edge_threshold(&self, name: &str) -> f32 {
        match name {
            "recordTabSelectedCap" => 0.50,
            "forkActivityTimeIcon" => 0.48,
            "forkDetailFileIcon" => 0.62,
            _ => 0.56,
        }
    }

    pub fn gray_floor(&self, name: &str) -> f32 {
        match name {
            "recordTabSelectedCap" => 0.20,
            "forkActivityTimeIcon" => 0.25,
            _ => 0.32,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStep {
    pub action: String,
    pub status: String,
    pub point: Option<String>,
    #[serde(default)]
    pub point_sequence: Vec<String>,
    pub template: Option<String>,
    pub target_template: Option<String>,
    pub page_rect: Option<String>,
    pub next_button: Option<String>,
    pub pool: Option<String>,
    pub settle: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationProfile {
    pub schema: String,
    pub schema_version: u32,
    pub profile: String,
    pub base_client_size: Size,
    pub aspect_ratio: String,
    pub window: WindowSpec,
    pub points: BTreeMap<String, Point>,
    pub rects: BTreeMap<String, Rect>,
    pub templates: BTreeMap<String, TemplateSpec>,
    pub workflow: Vec<WorkflowStep>,
}

impl AutomationProfile {
    pub fn scaled(&self, client_size: Size) -> AutomationResult<Self> {
        ensure_supported_client_size(client_size)?;
        let scale_x = client_size.width as f64 / self.base_client_size.width as f64;
        let scale_y = client_size.height as f64 / self.base_client_size.height as f64;
        Ok(Self {
            schema: self.schema.clone(),
            schema_version: self.schema_version,
            profile: self.profile.clone(),
            base_client_size: client_size,
            aspect_ratio: self.aspect_ratio.clone(),
            window: self.window.clone(),
            points: self
                .points
                .iter()
                .map(|(key, value)| (key.clone(), scale_point(*value, scale_x, scale_y)))
                .collect(),
            rects: self
                .rects
                .iter()
                .map(|(key, value)| (key.clone(), scale_rect(*value, scale_x, scale_y)))
                .collect(),
            templates: self
                .templates
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        TemplateSpec {
                            file: value.file.clone(),
                            rect: scale_rect(value.rect, scale_x, scale_y),
                            search_padding: scale_point(value.search_padding, scale_x, scale_y),
                            threshold: value.threshold,
                        },
                    )
                })
                .collect(),
            workflow: self.workflow.clone(),
        })
    }
}

pub fn load_profile() -> AutomationResult<AutomationProfile> {
    let profile: AutomationProfile = serde_json::from_str(DEFAULT_PROFILE_JSON)?;
    validate_profile(&profile)?;
    Ok(profile)
}

pub fn ensure_supported_client_size(size: Size) -> AutomationResult<()> {
    if is_supported_client_size(size) {
        Ok(())
    } else {
        Err(AutomationError::message(format!(
            "auto page supports 16:9 game client only, got {}x{}",
            size.width, size.height
        )))
    }
}

pub fn is_supported_client_size(size: Size) -> bool {
    if size.width == 0 || size.height == 0 {
        return false;
    }
    let ratio = size.width as f64 / size.height as f64;
    (ratio - 16.0 / 9.0).abs() <= ASPECT_RATIO_TOLERANCE
}

fn validate_profile(profile: &AutomationProfile) -> AutomationResult<()> {
    let mut errors = Vec::new();
    if profile.schema != PROFILE_SCHEMA {
        errors.push(format!("schema must be {PROFILE_SCHEMA}"));
    }
    if profile.schema_version != PROFILE_SCHEMA_VERSION {
        errors.push(format!("schemaVersion must be {PROFILE_SCHEMA_VERSION}"));
    }
    if profile.aspect_ratio != SUPPORTED_ASPECT_RATIO {
        errors.push(format!("aspectRatio must be {SUPPORTED_ASPECT_RATIO}"));
    }
    if !is_supported_client_size(profile.base_client_size) {
        errors.push("baseClientSize must be 16:9".to_string());
    }
    for (name, point) in &profile.points {
        validate_point(
            *point,
            profile.base_client_size,
            &format!("points.{name}"),
            &mut errors,
        );
    }
    for (name, rect) in &profile.rects {
        validate_rect(
            *rect,
            profile.base_client_size,
            &format!("rects.{name}"),
            &mut errors,
        );
    }
    for (name, template) in &profile.templates {
        validate_rect(
            template.rect,
            profile.base_client_size,
            &format!("templates.{name}.rect"),
            &mut errors,
        );
        validate_point(
            template.search_padding,
            profile.base_client_size,
            &format!("templates.{name}.searchPadding"),
            &mut errors,
        );
        if template_bytes(&template.file).is_none() {
            errors.push(format!("template not bundled: {}", template.file));
        }
    }
    for (index, step) in profile.workflow.iter().enumerate() {
        validate_workflow_step(index, step, profile, &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AutomationError::message(errors.join("; ")))
    }
}

pub(crate) fn template_bytes(file: &str) -> Option<&'static [u8]> {
    match file {
        "templates/home_board_file_icon.png" => Some(include_bytes!(
            "../assets/templates/home_board_file_icon.png"
        )),
        "templates/home_fork_entry_icon.png" => Some(include_bytes!(
            "../assets/templates/home_fork_entry_icon.png"
        )),
        "templates/record_tab_selected_cap.png" => Some(include_bytes!(
            "../assets/templates/record_tab_selected_cap.png"
        )),
        "templates/fork_shop_selected_icon.png" => Some(include_bytes!(
            "../assets/templates/fork_shop_selected_icon.png"
        )),
        "templates/fork_activity_time_icon.png" => Some(include_bytes!(
            "../assets/templates/fork_activity_time_icon.png"
        )),
        "templates/fork_detail_file_icon.png" => Some(include_bytes!(
            "../assets/templates/fork_detail_file_icon.png"
        )),
        _ => None,
    }
}

fn validate_workflow_step(
    index: usize,
    step: &WorkflowStep,
    profile: &AutomationProfile,
    errors: &mut Vec<String>,
) {
    let prefix = format!("workflow[{index}]");
    match step.action.as_str() {
        "verifyTemplate" => validate_ref(
            &step.template,
            &profile.templates,
            "template",
            &prefix,
            errors,
        ),
        "click" => validate_ref(&step.point, &profile.points, "point", &prefix, errors),
        "clickUntilTemplate" => {
            validate_ref(
                &step.template,
                &profile.templates,
                "template",
                &prefix,
                errors,
            );
            if step.point_sequence.is_empty() {
                errors.push(format!("{prefix}.pointSequence is required"));
            }
            for point in &step.point_sequence {
                if !profile.points.contains_key(point) {
                    errors.push(format!(
                        "{prefix}.pointSequence references missing point {point}"
                    ));
                }
            }
        }
        "clickTemplateUntilTemplate" => {
            validate_ref(
                &step.template,
                &profile.templates,
                "template",
                &prefix,
                errors,
            );
            validate_ref(
                &step.target_template,
                &profile.templates,
                "targetTemplate",
                &prefix,
                errors,
            );
            if let Some(point) = &step.point {
                if !profile.points.contains_key(point) {
                    errors.push(format!("{prefix}.point references missing point {point}"));
                }
            }
        }
        "pressEsc" => {}
        "page" => {
            validate_ref(&step.page_rect, &profile.rects, "pageRect", &prefix, errors);
            validate_ref(
                &step.next_button,
                &profile.points,
                "nextButton",
                &prefix,
                errors,
            );
            if step.pool.as_deref().unwrap_or_default().is_empty() {
                errors.push(format!("{prefix}.pool is required"));
            }
        }
        other => errors.push(format!("{prefix}.action is unsupported: {other}")),
    }
    if step.settle.is_some_and(|value| value < 0.0) {
        errors.push(format!("{prefix}.settle must be non-negative"));
    }
}

fn validate_ref<T>(
    value: &Option<String>,
    map: &BTreeMap<String, T>,
    field: &str,
    prefix: &str,
    errors: &mut Vec<String>,
) {
    match value {
        Some(value) if map.contains_key(value) => {}
        Some(value) => errors.push(format!("{prefix}.{field} references missing id {value}")),
        None => errors.push(format!("{prefix}.{field} is required")),
    }
}

fn validate_point(point: Point, size: Size, field: &str, errors: &mut Vec<String>) {
    if point.x < 0 || point.y < 0 || point.x > size.width as i32 || point.y > size.height as i32 {
        errors.push(format!(
            "{field} outside client: {},{} for {}x{}",
            point.x, point.y, size.width, size.height
        ));
    }
}

fn validate_rect(rect: Rect, size: Size, field: &str, errors: &mut Vec<String>) {
    if rect.width == 0 || rect.height == 0 {
        errors.push(format!("{field} must be positive"));
    }
    if rect.x < 0
        || rect.y < 0
        || rect.right() > size.width as i32
        || rect.bottom() > size.height as i32
    {
        errors.push(format!(
            "{field} outside client: {},{} {}x{} for {}x{}",
            rect.x, rect.y, rect.width, rect.height, size.width, size.height
        ));
    }
}

fn scale_point(point: Point, scale_x: f64, scale_y: f64) -> Point {
    Point {
        x: (point.x as f64 * scale_x).round() as i32,
        y: (point.y as f64 * scale_y).round() as i32,
    }
}

fn scale_rect(rect: Rect, scale_x: f64, scale_y: f64) -> Rect {
    Rect {
        x: (rect.x as f64 * scale_x).round() as i32,
        y: (rect.y as f64 * scale_y).round() as i32,
        width: ((rect.width as f64 * scale_x).round() as u32).max(1),
        height: ((rect.height as f64 * scale_y).round() as u32).max(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_loads_and_scales_16_9_sizes() {
        let profile = load_profile().unwrap();
        assert_eq!(profile.base_client_size.width, 1920);
        let scaled = profile
            .scaled(Size {
                width: 3840,
                height: 2160,
            })
            .unwrap();
        assert_eq!(scaled.points["homeBoardFile"].x, 900);
        assert_eq!(scaled.rects["boardPageNumber"].width, 190);
        assert!(scaled.templates["recordTabSelectedCap"].rect.width >= 176);
    }

    #[test]
    fn profile_rejects_non_16_9_sizes() {
        let profile = load_profile().unwrap();
        let error = profile
            .scaled(Size {
                width: 1920,
                height: 1200,
            })
            .unwrap_err();
        assert!(error.to_string().contains("16:9"));
    }
}
