//! Responsive ribbon groups. Collapsed groups contain the same controls as the
//! expanded ribbon, so resizing never changes a command's behavior.

use super::ribbon_controls::{self as controls, Scope};
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Group {
    pub label: &'static str,
    pub width: f32,
    pub icon: Icon,
    pub keys: &'static str,
    pub popup: &'static str,
}

pub(super) const COLLAPSED_WIDTH: f32 = 66.0;

#[derive(Clone, Copy)]
struct OpenGroup {
    tab: &'static str,
    group: &'static str,
    owner: Id,
}

pub(super) fn close_groups(ctx: &Context) {
    let previous = ctx.data_mut(|data| {
        let id = Id::new("paint10-open-ribbon-group");
        let previous = data.get_temp::<OpenGroup>(id);
        data.remove::<OpenGroup>(id);
        previous
    });
    if let Some(previous) = previous {
        egui::menu::BarState::default().store(ctx, previous.owner);
        ctx.request_repaint();
    }
}

/// Preserve command order while collapsing the least essential expanded groups
/// first. Each priority is an index into `groups`.
pub(super) fn widths(groups: &[Group], available: f32, priority: &[usize]) -> Vec<f32> {
    let mut widths: Vec<_> = groups.iter().map(|group| group.width).collect();
    let mut total: f32 = widths.iter().sum();
    for &index in priority {
        if total <= available {
            break;
        }
        let compact = COLLAPSED_WIDTH.min(widths[index]);
        total -= widths[index] - compact;
        widths[index] = compact;
    }
    // A late collapse can free enough space to restore an earlier group. Use
    // that space in priority order instead of leaving an unnecessarily empty bar.
    for &index in priority.iter().rev() {
        let extra = groups[index].width - widths[index];
        if total + extra <= available {
            widths[index] += extra;
            total += extra;
        }
    }
    widths
}

pub(super) fn show(
    ui: &mut Ui,
    origin: Pos2,
    width: f32,
    tab: &'static str,
    group: Group,
    contents: impl FnOnce(&mut Ui, Pos2),
) {
    let rect = Rect::from_min_size(origin, vec2(width, 112.0));
    let mut child = ui.new_child(UiBuilder::new().id_salt((tab, group.label)).max_rect(rect));
    if width >= group.width {
        let was_open = ui
            .ctx()
            .data(|data| data.get_temp::<OpenGroup>(Id::new("paint10-open-ribbon-group")))
            .is_some_and(|open| open.tab == tab && open.group == group.label);
        if was_open {
            close_groups(ui.ctx());
        }
        controls::scope(&mut child, Scope::new(tab, group.label), |ui| {
            contents(ui, origin)
        });
        return;
    }

    PaintApp::group(&child, origin, 0.0, width - 1.0, "");
    let mut button_ui = child.new_child(UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, 5.0))));
    let menu = egui::menu::menu_custom_button(
        &mut button_ui,
        Button::new("").min_size(vec2(width - 8.0, 96.0)),
        |ui| {
            let scope = Scope::new(group.popup, group.label);
            controls::scope(ui, scope, |ui| {
                ui.set_min_size(vec2(group.width, 112.0));
                let origin = ui.cursor().min;
                ui.allocate_space(vec2(group.width, 112.0));
                contents(ui, origin);
            });
        },
    );
    let response = &menu.response;
    if response.clicked() {
        let id = Id::new("paint10-open-ribbon-group");
        let previous = ui.ctx().data(|data| data.get_temp::<OpenGroup>(id));
        if let Some(previous) = previous.filter(|previous| previous.owner != button_ui.id()) {
            egui::menu::BarState::default().store(ui.ctx(), previous.owner);
            ui.ctx().request_repaint();
        }
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                id,
                OpenGroup {
                    tab,
                    group: group.label,
                    owner: button_ui.id(),
                },
            )
        });
    }
    icons::draw(
        button_ui.painter(),
        Rect::from_center_size(
            response.rect.center_top() + vec2(0.0, 27.0),
            vec2(30.0, 30.0),
        ),
        group.icon,
    );
    // A short two-line label keeps the visibility group legible at 66 px.
    let label = match group.label {
        "Show or hide" => "Show or\nhide",
        other => other,
    };
    button_ui.painter().text(
        response.rect.center_top() + vec2(0.0, 65.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(if group.label == "Background" {
            10.0
        } else {
            11.0
        }),
        Color32::from_gray(35),
    );
    icons::draw(
        button_ui.painter(),
        Rect::from_center_size(
            response.rect.center_bottom() - vec2(0.0, 8.0),
            vec2(12.0, 12.0),
        ),
        Icon::ChevronDown,
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, group.label));
    keytips::register(
        &button_ui,
        response,
        tab,
        group.label,
        group.keys,
        keytips::Kind::PopupGroup { scope: group.popup },
    );
    response
        .clone()
        .on_hover_text(format!("{} commands", group.label));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_fit_and_restore_at_each_width() {
        let groups = [119.0, 161.0, 92.0, 71.0, 252.0, 65.0, 347.0].map(|width| Group {
            label: "Group",
            width,
            icon: Icon::Colors,
            keys: "Z",
            popup: "group",
        });
        for available in [498.0, 640.0, 800.0, 1000.0, 1200.0] {
            let result = widths(&groups, available, &[6, 4, 1, 0, 2, 3]);
            assert!(result.iter().sum::<f32>() <= available);
            assert!(result
                .iter()
                .zip(groups)
                .all(|(width, group)| { *width == group.width || *width == COLLAPSED_WIDTH }));
        }
        assert_eq!(
            widths(&groups, 1200.0, &[6, 4, 1, 0, 2, 3]),
            groups.map(|g| g.width)
        );
    }
}
