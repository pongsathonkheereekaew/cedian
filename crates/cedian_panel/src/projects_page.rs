use crate::permissions_page::{
    OpenProjects, ProjectEntry, ProjectSettings, Row, dropdown, folders_control, folders_state,
    muted, policy_control, section,
};
use gpui::{Context, Render, Window};
use settings_ui::{RowState, ScopeBadge, render_row};
use ui::{Button, ButtonStyle, Chip, prelude::*};

pub struct ProjectsPage {
    state: ProjectSettings,
    projects: OpenProjects,
}

impl ProjectsPage {
    pub fn new(projects: OpenProjects) -> Self {
        Self {
            state: ProjectSettings::load_from_env(),
            projects,
        }
    }

    pub fn state(&self) -> &ProjectSettings {
        &self.state
    }

    fn state_mut(&mut self) -> &mut ProjectSettings {
        &mut self.state
    }

    fn card(
        &self,
        index: usize,
        entry: &ProjectEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = |what: &str| SharedString::from(format!("cedian-project-{index}-{what}"));
        let key = entry.key.clone();
        let path_state = self.state.row_state(&Row::Entry(key.clone())).or_else(|| {
            (!entry.notes.is_empty()).then(|| RowState::Error(entry.notes.join("; ").into()))
        });
        let stored = self.state.settings().project_keys().contains(&key.as_str());
        let remove = Button::new(id("remove"), "Remove entry")
            .style(ButtonStyle::OutlinedGhost)
            .disabled(!stored)
            .on_click(cx.listener({
                let key = key.clone();
                move |this, _, _, cx| {
                    this.state.remove_entry(&key);
                    cx.notify();
                }
            }));
        let title = h_flex()
            .gap_2()
            .child(Label::new(entry.key.clone()))
            .when(entry.open, |this| {
                this.child(Chip::new("Open").label_size(LabelSize::XSmall))
            });
        let policy = policy_control(
            id("policy"),
            entry.policy,
            {
                let key = key.clone();
                move |this: &mut Self, policy, _| this.state.set_policy(&key, policy)
            },
            window,
            cx,
        );
        let pstack = dropdown(
            id("pstack"),
            entry.pstack,
            vec![("Inherit", None), ("On", Some(true)), ("Off", Some(false))],
            false,
            {
                let key = key.clone();
                move |this: &mut Self, pstack, _| this.state.set_pstack(&key, pstack)
            },
            window,
            cx,
        );
        let rows = vec![
            render_row(
                id("path-row"),
                "Project Path".into(),
                None,
                None,
                path_state.as_ref(),
                remove.into_any_element(),
                cx,
            )
            .into_any_element(),
            render_row(
                id("policy-row"),
                "Agent Policy".into(),
                None,
                Some(ScopeBadge::User),
                self.state.row_state(&Row::Policy(key.clone())).as_ref(),
                policy,
                cx,
            )
            .into_any_element(),
            render_row(
                id("pstack-row"),
                "pstack".into(),
                Some("Inherit follows Methodology.".into()),
                Some(ScopeBadge::User),
                self.state.row_state(&Row::Pstack(key.clone())).as_ref(),
                pstack,
                cx,
            )
            .into_any_element(),
            render_row(
                id("folders-row"),
                "Folders the Agent May Read Without Asking".into(),
                None,
                Some(ScopeBadge::User),
                folders_state(&self.state, &key, &entry.read_folders).as_ref(),
                folders_control(
                    &format!("cedian-project-{index}-folders"),
                    &key,
                    &entry.read_folders,
                    Self::state_mut,
                    cx,
                ),
                cx,
            )
            .into_any_element(),
        ];
        v_flex()
            .gap_3()
            .pb_6()
            .child(title)
            .children(rows)
            .into_any_element()
    }
}

impl Render for ProjectsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self
            .state
            .entries(&self.projects.all, self.projects.current.as_deref());
        let cards: Vec<AnyElement> = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| self.card(index, entry, window, cx))
            .collect();
        v_flex()
            .w_full()
            .pt_2()
            .when_some(self.state.load_error.clone(), |this, error| {
                this.child(
                    div()
                        .mb_4()
                        .child(Label::new(error).size(LabelSize::Small).color(Color::Error)),
                )
            })
            .when(cards.is_empty(), |this| {
                this.child(muted(
                    "No projects yet. Open a folder to set its agent policy.",
                ))
            })
            .child(section("Projects", cards))
    }
}
