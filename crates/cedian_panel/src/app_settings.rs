//! cedian's app settings the panel reads (Settings > General, Composer and
//! Settings > Notifications); the settings window writes them into the
//! user's settings file under `cedian`.

use settings::{
    FollowUp, NotificationSound, RegisterSetting, Settings, SettingsContent,
    TurnFinishedNotification,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, RegisterSetting)]
pub struct CedianSettings {
    pub follow_up: FollowUp,
    pub notify_turn_finished: TurnFinishedNotification,
    pub notify_approval_needed: bool,
    pub notify_question_asked: bool,
    pub bounce_dock_icon: bool,
    pub notification_sound: NotificationSound,
}

impl Settings for CedianSettings {
    fn from_settings(content: &SettingsContent) -> Self {
        let cedian = content.cedian.clone().unwrap_or_default();
        Self {
            follow_up: cedian.follow_up.unwrap_or_default(),
            notify_turn_finished: cedian.notify_turn_finished.unwrap_or_default(),
            notify_approval_needed: cedian.notify_approval_needed.unwrap_or(true),
            notify_question_asked: cedian.notify_question_asked.unwrap_or(true),
            bounce_dock_icon: cedian.bounce_dock_icon.unwrap_or(true),
            notification_sound: cedian.notification_sound.unwrap_or_default(),
        }
    }
}
