use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings_macros::{MergeFrom, with_fallible_options};

/// cedian's own app settings: the agent composer and notifications.
#[with_fallible_options]
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema, MergeFrom)]
pub struct CedianSettingsContent {
    /// What a message sent while the agent works does: wait for the current
    /// turn (queue) or stop it and go now (steer).
    ///
    /// Default: queue
    pub follow_up: Option<FollowUp>,
    /// When to tell you that a turn finished.
    ///
    /// Default: when_unfocused
    pub notify_turn_finished: Option<TurnFinishedNotification>,
    /// Tell you when the agent waits for an approval.
    ///
    /// Default: true
    pub notify_approval_needed: Option<bool>,
    /// Tell you when the agent asks you something.
    ///
    /// Default: true
    pub notify_question_asked: Option<bool>,
    /// Bounce the Dock icon when a notification arrives in the background.
    ///
    /// Default: true
    pub bounce_dock_icon: Option<bool>,
    /// The sound a notification plays.
    ///
    /// Default: system
    pub notification_sound: Option<NotificationSound>,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum FollowUp {
    #[default]
    Queue,
    Steer,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum TurnFinishedNotification {
    #[default]
    #[strum(serialize = "Only when unfocused")]
    WhenUnfocused,
    Always,
    Never,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    MergeFrom,
    strum::VariantArray,
    strum::VariantNames,
)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSound {
    None,
    /// The system alert sound.
    #[default]
    #[strum(serialize = "System alert")]
    System,
}
