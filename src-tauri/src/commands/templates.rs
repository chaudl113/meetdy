use crate::settings::{get_settings, write_settings, MeetingTemplate};
use log::debug;
use tauri::AppHandle;

#[tauri::command]
#[specta::specta]
pub fn list_meeting_templates(app: AppHandle) -> Result<Vec<MeetingTemplate>, String> {
    debug!("list_meeting_templates command called");
    let settings = get_settings(&app);
    Ok(settings.meeting_templates)
}

#[tauri::command]
#[specta::specta]
pub fn create_meeting_template(
    app: AppHandle,
    name: String,
    icon: String,
    title_template: String,
    audio_source: String,
    prompt_id: Option<String>,
    summary_prompt_template: Option<String>,
) -> Result<MeetingTemplate, String> {
    debug!("create_meeting_template command called: name={}", name);

    // Validation
    if name.trim().is_empty() {
        return Err("Template name cannot be empty".to_string());
    }

    if name.len() > 50 {
        return Err("Template name must be 50 characters or less".to_string());
    }

    // Validate audio_source
    if !["microphone_only", "system_only", "mixed"].contains(&audio_source.as_str()) {
        return Err(format!("Invalid audio_source: {}", audio_source));
    }

    // Validate summary_prompt_template if provided
    if let Some(ref spt) = summary_prompt_template {
        if !spt.contains("{}") {
            return Err("summary_prompt_template must contain '{}' placeholder for transcript".to_string());
        }
        if spt.len() > 10000 {
            return Err("summary_prompt_template is too long (max 10000 characters)".to_string());
        }
    }

    let mut settings = get_settings(&app);

    // Check for duplicate names
    if settings
        .meeting_templates
        .iter()
        .any(|t| t.name == name.trim())
    {
        return Err(format!("Template with name '{}' already exists", name.trim()));
    }

    // Generate new template
    let new_template = MeetingTemplate {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().to_string(),
        icon,
        title_template,
        audio_source,
        prompt_id,
        summary_prompt_template,
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
    };

    settings.meeting_templates.push(new_template.clone());

    write_settings(&app, settings);
    debug!("Template created successfully: {}", new_template.id);
    Ok(new_template)
}

#[tauri::command]
#[specta::specta]
pub fn update_meeting_template(
    app: AppHandle,
    id: String,
    name: Option<String>,
    icon: Option<String>,
    title_template: Option<String>,
    audio_source: Option<String>,
    prompt_id: Option<String>,
    summary_prompt_template: Option<String>,
) -> Result<MeetingTemplate, String> {
    debug!("update_meeting_template command called: id={}", id);

    let mut settings = get_settings(&app);

    // Find template
    let template = settings
        .meeting_templates
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("Template with id '{}' not found", id))?;

    // Update fields if provided
    if let Some(n) = name {
        if n.trim().is_empty() {
            return Err("Template name cannot be empty".to_string());
        }
        if n.len() > 50 {
            return Err("Template name must be 50 characters or less".to_string());
        }
        template.name = n.trim().to_string();
    }

    if let Some(i) = icon {
        template.icon = i;
    }

    if let Some(tt) = title_template {
        template.title_template = tt;
    }

    if let Some(as_val) = audio_source {
        if !["microphone_only", "system_only", "mixed"].contains(&as_val.as_str()) {
            return Err(format!("Invalid audio_source: {}", as_val));
        }
        template.audio_source = as_val;
    }

    // Note: prompt_id can be None to clear it
    if prompt_id.is_some() {
        template.prompt_id = prompt_id;
    }

    // Handle summary_prompt_template update
    if let Some(ref spt) = summary_prompt_template {
        if !spt.is_empty() && !spt.contains("{}") {
            return Err("summary_prompt_template must contain '{}' placeholder for transcript".to_string());
        }
        if spt.len() > 10000 {
            return Err("summary_prompt_template is too long (max 10000 characters)".to_string());
        }
    }
    if summary_prompt_template.is_some() {
        template.summary_prompt_template = summary_prompt_template;
    }

    template.updated_at = chrono::Utc::now().timestamp();

    let updated_template = template.clone();

    write_settings(&app, settings);
    debug!("Template updated successfully: {}", id);
    Ok(updated_template)
}

#[tauri::command]
#[specta::specta]
pub fn delete_meeting_template(app: AppHandle, id: String) -> Result<(), String> {
    debug!("delete_meeting_template command called: id={}", id);

    let mut settings = get_settings(&app);

    // Prevent deleting default templates
    if id.starts_with("template_") {
        return Err("Cannot delete default templates".to_string());
    }

    // Find and remove template
    let initial_len = settings.meeting_templates.len();
    settings.meeting_templates.retain(|t| t.id != id);

    if settings.meeting_templates.len() == initial_len {
        return Err(format!("Template with id '{}' not found", id));
    }

    write_settings(&app, settings);
    debug!("Template deleted successfully: {}", id);
    Ok(())
}
