use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

use crate::{DockPosition, PixelSetting};

#[with_fallible_options]
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct AgentPanelSettingsContent {
    /// Whether to show the agent panel button in the status bar.
    ///
    /// Default: true
    pub button: Option<bool>,

    /// Where to dock the agent panel.
    ///
    /// Default: right
    pub dock: Option<DockPosition>,

    /// Default width of the panel in pixels.
    ///
    /// Default: 420
    pub default_width: Option<PixelSetting>,

    /// Whether the agent panel should open on startup.
    ///
    /// Default: false
    pub starts_open: Option<bool>,
}
