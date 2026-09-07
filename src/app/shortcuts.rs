use super::*;

impl PaintApp {
    pub(in crate::app) fn shortcut(&mut self, ctx: &Context) {
        if self.dialog.is_some() || self.pending.is_some() {
            return;
        }
        // Menus own Escape and arrow keys. Closing a menu must not discard the
        // shape or text currently being edited underneath it.
        if ctx.memory(|memory| memory.any_popup_open()) {
            return;
        }
        let temporary_ribbon = Id::new("paint10-ribbon-revealed");
        if ctx.data(|data| data.get_temp::<bool>(temporary_ribbon).unwrap_or(false))
            && ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::Escape))
        {
            ctx.data_mut(|data| data.insert_temp(temporary_ribbon, false));
            return;
        }
        let global = [
            (Modifiers::CTRL, Key::N, Action::New),
            (Modifiers::CTRL, Key::O, Action::Open),
            (Modifiers::CTRL, Key::S, Action::Save),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::S, Action::SaveAs),
            (Modifiers::CTRL, Key::W, Action::Resize),
            (Modifiers::CTRL, Key::E, Action::Properties),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::X, Action::Crop),
            (
                Modifiers::CTRL | Modifiers::SHIFT,
                Key::V,
                Action::PasteFrom,
            ),
            (Modifiers::CTRL, Key::P, Action::Print),
            (
                Modifiers::CTRL | Modifiers::SHIFT,
                Key::N,
                Action::ClearPicture,
            ),
            (Modifiers::NONE, Key::F12, Action::SaveAs),
        ];
        for (mods, key, action) in global {
            if ctx.input_mut(|i| consume_shortcut(i, mods, key)) {
                self.action(action, ctx);
                return;
            }
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::F11)) {
            self.show_picture(ctx);
            return;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::F1)) {
            self.dialog = Some(Dialog::About);
            return;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::F1)) {
            self.collapsed = !self.collapsed;
        }
        self.text_shortcuts(ctx);
        if self.text_edit.is_some() {
            return;
        }
        for (mods, key, action) in [
            (Modifiers::CTRL, Key::Z, Action::Undo),
            (Modifiers::CTRL, Key::Y, Action::Redo),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::Z, Action::Redo),
            (Modifiers::CTRL, Key::A, Action::SelectAll),
            (Modifiers::CTRL | Modifiers::SHIFT, Key::I, Action::Invert),
        ] {
            if ctx.input_mut(|i| consume_shortcut(i, mods, key)) {
                self.action(action, ctx);
                return;
            }
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::Delete)) {
            self.delete_selection();
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                Event::Copy => self.copy(),
                Event::Cut => {
                    self.copy();
                    self.delete_selection();
                }
                Event::Paste(_) => {
                    self.paste_clipboard();
                }
                _ => {}
            }
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, Key::Escape)) {
            self.doc.cancel();
            self.curve = None;
            self.shape_draft = None;
            self.polygon.clear();
            self.gesture = None;
            self.clear_selection();
            self.preview = false;
            self.fullscreen = false;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            self.refresh = true;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::G)) {
            self.grid = !self.grid;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::R)) {
            self.rulers = !self.rulers;
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::PageUp)) {
            self.zoom = (self.zoom * 2.).min(8.);
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::PageDown)) {
            self.zoom = (self.zoom / 2.).max(0.125);
        }
        if ctx.input_mut(|i| {
            consume_shortcut(i, Modifiers::CTRL, Key::Plus)
                || consume_shortcut(i, Modifiers::CTRL | Modifiers::SHIFT, Key::Plus)
                || consume_shortcut(i, Modifiers::CTRL, Key::Equals)
        }) {
            self.size = (self.size + 1).min(100);
        }
        if ctx.input_mut(|i| consume_shortcut(i, Modifiers::CTRL, Key::Minus)) {
            self.size = self.size.saturating_sub(1).max(1);
        }
        for (key, delta) in [
            (Key::ArrowLeft, (-1, 0)),
            (Key::ArrowRight, (1, 0)),
            (Key::ArrowUp, (0, -1)),
            (Key::ArrowDown, (0, 1)),
        ] {
            if !ctx.wants_keyboard_input()
                && ctx.input_mut(|i| consume_shortcut(i, Modifiers::NONE, key))
            {
                if let Some(i) = self.lift_selection() {
                    self.doc.objects[i].pos.0 += delta.0;
                    self.doc.objects[i].pos.1 += delta.1;
                    self.doc.commit();
                    self.refresh = true;
                }
            }
        }
    }
}

pub(super) fn consume_shortcut(input: &mut InputState, modifiers: Modifiers, key: Key) -> bool {
    let mut consumed = false;
    input.events.retain(|event| {
        let matched = matches!(event, Event::Key { key: pressed_key, modifiers: pressed_modifiers, pressed: true, .. }
            if *pressed_key == key && pressed_modifiers.matches_exact(modifiers));
        consumed |= matched;
        !matched
    });
    consumed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shifted_actions_are_not_consumed_by_plain_shortcuts() {
        for key in [Key::S, Key::X, Key::N, Key::V] {
            let mut input = InputState::default();
            input.events.push(Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT,
            });
            assert!(!consume_shortcut(&mut input, Modifiers::CTRL, key));
            assert!(consume_shortcut(
                &mut input,
                Modifiers::CTRL | Modifiers::SHIFT,
                key
            ));
            assert!(input.events.is_empty());
        }
    }
}
