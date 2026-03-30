// ProgressSpineWidget: vertical step indicator for the onboard wizard.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::onboard_flow::OnboardFlowController;
use crate::onboard_state::OnboardWizardStep;

#[allow(dead_code)] // consumed by runner/layout in later tasks
pub(crate) struct ProgressSpineWidget {
    current_step: OnboardWizardStep,
}

#[allow(dead_code)] // consumed by runner/layout in later tasks
impl ProgressSpineWidget {
    pub fn new(current_step: OnboardWizardStep) -> Self {
        Self { current_step }
    }

    fn step_entries() -> &'static [(OnboardWizardStep, &'static str)] {
        &[
            (OnboardWizardStep::Welcome, "Welcome"),
            (OnboardWizardStep::Authentication, "Auth"),
            (OnboardWizardStep::RuntimeDefaults, "Runtime"),
            (OnboardWizardStep::Workspace, "Workspace"),
            (OnboardWizardStep::Protocols, "Protocols"),
            (OnboardWizardStep::EnvironmentCheck, "Env Check"),
            (OnboardWizardStep::ReviewAndWrite, "Review"),
            (OnboardWizardStep::Ready, "Ready"),
        ]
    }

    fn step_state(&self, step: OnboardWizardStep) -> StepState {
        let steps = OnboardFlowController::ordered_steps();
        let current_idx = steps
            .iter()
            .position(|s| *s == self.current_step)
            .unwrap_or(0);
        let step_idx = steps.iter().position(|s| *s == step).unwrap_or(0);
        if step_idx < current_idx {
            StepState::Done
        } else if step_idx == current_idx {
            StepState::Active
        } else {
            StepState::Pending
        }
    }
}

#[allow(dead_code)] // internal to ProgressSpineWidget rendering
enum StepState {
    Done,
    Active,
    Pending,
}

impl Widget for ProgressSpineWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, (step, label)) in Self::step_entries().iter().enumerate() {
            if i as u16 >= area.height {
                break;
            }
            let y = area.y + i as u16;
            let (icon, style) = match self.step_state(*step) {
                StepState::Done => ("\u{2713}", Style::default().fg(Color::Green)),
                StepState::Active => (
                    "\u{25cf}",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                StepState::Pending => ("\u{25cb}", Style::default().fg(Color::DarkGray)),
            };
            let line = Line::from(vec![
                Span::styled(format!(" {icon} "), style),
                Span::styled(*label, style),
            ]);
            buf.set_line(area.x, y, &line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    #[test]
    fn spine_renders_current_step_highlighted() {
        let widget = ProgressSpineWidget::new(OnboardWizardStep::Authentication);
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content = buffer_text(&buf);
        assert!(
            content.contains("Welcome"),
            "should show Welcome as completed"
        );
        assert!(content.contains("Auth"), "should show current step");
    }

    #[test]
    fn spine_marks_earlier_steps_as_done() {
        let widget = ProgressSpineWidget::new(OnboardWizardStep::Workspace);
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content = buffer_text(&buf);
        // Welcome, Auth, Runtime should show checkmark
        // Workspace should show active dot
        // Protocols, Env, Review, Ready should show empty circle
        assert!(content.contains('\u{2713}'));
        assert!(content.contains('\u{25cf}'));
        assert!(content.contains('\u{25cb}'));
    }

    fn buffer_text(buf: &Buffer) -> String {
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }
}
