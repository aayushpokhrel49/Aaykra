use gpui::Pixels;
use settings::{IntoGpui, RegisterSetting, Settings, SettingsContent};
use workspace::dock::DockPosition;

#[derive(Debug, Clone, PartialEq, RegisterSetting)]
pub struct AgentPanelSettings {
    pub button: bool,
    pub dock: DockPosition,
    pub default_width: Pixels,
    pub starts_open: bool,
    pub api_url: String,
    pub api_key: String,
    pub model: String,
}

impl Settings for AgentPanelSettings {
    fn from_settings(content: &SettingsContent) -> Self {
        let agent_panel = content.agent_panel.clone().unwrap();
        Self {
            button: agent_panel.button.unwrap(),
            dock: agent_panel.dock.unwrap().into(),
            default_width: agent_panel.default_width.unwrap().into_gpui(),
            starts_open: agent_panel.starts_open.unwrap(),
            api_url: agent_panel.api_url.clone().unwrap_or_default(),
            api_key: agent_panel.api_key.clone().unwrap_or_default(),
            model: agent_panel.model.clone().unwrap_or_default(),
        }
    }
}
