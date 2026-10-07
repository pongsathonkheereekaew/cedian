//! Headless answers to OMP's extension UI requests (approval dialogs, `ask`).
//!
//! A process with no UI must not leave a dialog open: OMP waits on it until
//! the prompt deadline. Fail closed instead — deny an approval, decline a
//! confirm, dismiss everything else that expects an answer — and record the
//! dialog title so the caller can say what was refused (ADR-0012 strict-wins).
//! Fire-and-forget requests (notify, status, widget, …) need no reply.

use omp_rpc::wire::{
    CancelUiResponse, ConfirmUiResponse, ExtensionUiRequest, ExtensionUiResponse, LitTrue,
    ValueUiResponse,
};

/// The fail-closed reply to `request` plus a one-line label for it, or
/// `None` when the request expects no reply.
pub fn headless_answer(request: &ExtensionUiRequest) -> Option<(ExtensionUiResponse, String)> {
    let cancel = |id: &str| {
        ExtensionUiResponse::CancelUiResponse(CancelUiResponse {
            id: id.to_string(),
            cancelled: LitTrue,
            timed_out: None,
        })
    };
    let label = |title: &str| title.lines().collect::<Vec<_>>().join(" — ");
    match request {
        ExtensionUiRequest::Select(r) => {
            let reply = match r.options.iter().find(|o| o.eq_ignore_ascii_case("deny")) {
                Some(deny) => ExtensionUiResponse::ValueUiResponse(ValueUiResponse {
                    id: r.id.clone(),
                    value: deny.clone(),
                }),
                None => cancel(&r.id),
            };
            Some((reply, label(&r.title)))
        }
        ExtensionUiRequest::Confirm(r) => Some((
            ExtensionUiResponse::ConfirmUiResponse(ConfirmUiResponse {
                id: r.id.clone(),
                confirmed: false,
            }),
            label(&r.title),
        )),
        ExtensionUiRequest::Input(r) => Some((cancel(&r.id), label(&r.title))),
        ExtensionUiRequest::Editor(r) => Some((cancel(&r.id), label(&r.title))),
        ExtensionUiRequest::Ask(r) => Some((cancel(&r.id), "ask".to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(v: serde_json::Value) -> ExtensionUiRequest {
        ExtensionUiRequest::from_value(v).expect("valid request")
    }

    #[test]
    fn approval_select_is_denied() {
        let r = request(json!({
            "type": "extension_ui_request", "method": "select", "id": "u1",
            "title": "Allow tool: bash\nCommand: ls", "options": ["Approve", "Deny"]
        }));
        let (reply, label) = headless_answer(&r).unwrap();
        assert_eq!(
            reply,
            ExtensionUiResponse::ValueUiResponse(ValueUiResponse {
                id: "u1".into(),
                value: "Deny".into()
            })
        );
        assert_eq!(label, "Allow tool: bash — Command: ls");
    }

    #[test]
    fn confirm_is_declined_and_notify_needs_no_reply() {
        let r = request(json!({
            "type": "extension_ui_request", "method": "confirm", "id": "u2",
            "title": "Run?", "message": "really"
        }));
        assert!(matches!(
            headless_answer(&r),
            Some((
                ExtensionUiResponse::ConfirmUiResponse(ConfirmUiResponse {
                    confirmed: false,
                    ..
                }),
                _
            ))
        ));
        let n = request(json!({
            "type": "extension_ui_request", "method": "notify", "id": "u3", "message": "hi"
        }));
        assert!(headless_answer(&n).is_none());
    }

    #[test]
    fn select_without_deny_is_dismissed() {
        let r = request(json!({
            "type": "extension_ui_request", "method": "select", "id": "u4",
            "title": "Pick", "options": ["a", "b"]
        }));
        assert!(matches!(
            headless_answer(&r),
            Some((ExtensionUiResponse::CancelUiResponse(_), _))
        ));
    }
}
