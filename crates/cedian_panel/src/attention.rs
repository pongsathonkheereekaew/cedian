//! Telling the person the agent finished or needs them (Settings >
//! Notifications): an in-app toast, a Dock bounce and a sound while cedian
//! is in the background. macOS notification-center delivery waits for a
//! signed build.

use crate::app_settings::CedianSettings;
use settings::{NotificationSound, TurnFinishedNotification};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attention {
    TurnFinished,
    ApprovalNeeded,
    QuestionAsked,
}

impl Attention {
    pub fn message(self) -> &'static str {
        match self {
            Self::TurnFinished => "cedian agent finished its turn",
            Self::ApprovalNeeded => "cedian agent is waiting for your approval",
            Self::QuestionAsked => "cedian agent asked you something",
        }
    }

    /// An agent dialog asks for an approval when OMP's title names a tool
    /// ("Allow tool: …"); any other dialog is a question.
    pub fn for_dialog(title: &str) -> Self {
        if title.starts_with("Allow tool") {
            Self::ApprovalNeeded
        } else {
            Self::QuestionAsked
        }
    }
}

/// How one attention is delivered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Delivery {
    pub toast: bool,
    pub bounce: bool,
    pub sound: bool,
}

pub fn delivery(attention: Attention, settings: &CedianSettings, focused: bool) -> Delivery {
    let wanted = match attention {
        Attention::TurnFinished => match settings.notify_turn_finished {
            TurnFinishedNotification::Always => true,
            TurnFinishedNotification::WhenUnfocused => !focused,
            TurnFinishedNotification::Never => false,
        },
        Attention::ApprovalNeeded => settings.notify_approval_needed,
        Attention::QuestionAsked => settings.notify_question_asked,
    };
    if !wanted {
        return Delivery::default();
    }
    Delivery {
        toast: true,
        bounce: !focused && settings.bounce_dock_icon,
        sound: !focused && settings.notification_sound == NotificationSound::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use settings::{FollowUp, SendWith};

    fn defaults() -> CedianSettings {
        CedianSettings {
            send_with: SendWith::Enter,
            follow_up: FollowUp::Queue,
            notify_turn_finished: TurnFinishedNotification::WhenUnfocused,
            notify_approval_needed: true,
            notify_question_asked: true,
            bounce_dock_icon: true,
            notification_sound: NotificationSound::System,
        }
    }

    #[test]
    fn a_finished_turn_is_told_only_in_the_background_by_default() {
        let settings = defaults();
        assert_eq!(
            delivery(Attention::TurnFinished, &settings, true),
            Delivery::default()
        );
        assert_eq!(
            delivery(Attention::TurnFinished, &settings, false),
            Delivery {
                toast: true,
                bounce: true,
                sound: true
            }
        );
    }

    #[test]
    fn switches_turn_each_attention_off() {
        let settings = CedianSettings {
            notify_turn_finished: TurnFinishedNotification::Never,
            notify_approval_needed: false,
            bounce_dock_icon: false,
            notification_sound: NotificationSound::None,
            ..defaults()
        };
        assert_eq!(
            delivery(Attention::TurnFinished, &settings, false),
            Delivery::default()
        );
        assert_eq!(
            delivery(Attention::ApprovalNeeded, &settings, false),
            Delivery::default()
        );
        assert_eq!(
            delivery(Attention::QuestionAsked, &settings, false),
            Delivery {
                toast: true,
                bounce: false,
                sound: false
            }
        );
    }

    #[test]
    fn an_approval_is_told_from_a_question_by_its_title() {
        assert_eq!(
            Attention::for_dialog("Allow tool: bash\nrm -rf build"),
            Attention::ApprovalNeeded
        );
        assert_eq!(
            Attention::for_dialog("Which branch?"),
            Attention::QuestionAsked
        );
    }
}
