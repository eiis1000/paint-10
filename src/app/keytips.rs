//! Keyboard navigation is attached to the responses rendered by the ribbon.
//! Activations use the same focused-widget Enter path as ordinary keyboard input.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Button,
    TextInput,
    NumericInput,
    Menu { scope: &'static str },
    Tab { scope: &'static str },
    PopupGroup { scope: &'static str },
}

impl Kind {
    fn is_text_input(self) -> bool {
        matches!(self, Self::TextInput | Self::NumericInput)
    }
}

#[derive(Clone)]
struct Target {
    id: Id,
    owner: Id,
    layer: LayerId,
    rect: Rect,
    clip: Rect,
    scope: String,
    group: String,
    keys: String,
    kind: Kind,
    enabled: bool,
    focusable: bool,
    visible: bool,
    left_parent: Option<Id>,
}

#[derive(Clone)]
struct Level {
    scope: String,
    opener: Option<Target>,
}

#[derive(Clone)]
struct Activation {
    id: Id,
    parent_depth: usize,
}

#[derive(Clone, Default)]
struct State {
    targets: Vec<Target>,
    previous: Vec<Target>,
    levels: Vec<Level>,
    prefix: String,
    tips: bool,
    origin: Option<Id>,
    focus_first: bool,
    restore_at_end: bool,
    consume_enter_at_end: bool,
    consume_escape_at_end: bool,
    alt_down: bool,
    alt_used: bool,
    current_tab: String,
    queued_keys: Vec<Key>,
    activation: Option<Activation>,
    popup_was_open: bool,
    focused: Option<Id>,
    deferred_text_commit: Option<Id>,
    deferred_events: Vec<Event>,
    navigation_events: Vec<(usize, Event)>,
    navigation_tail: Vec<Event>,
}

const STATE: &str = "paint10_keytips";

fn read(ctx: &Context) -> State {
    ctx.data(|data| data.get_temp::<State>(Id::new(STATE)).unwrap_or_default())
}

fn write(ctx: &Context, state: State) {
    ctx.data_mut(|data| data.insert_temp(Id::new(STATE), state));
}

pub(super) fn register(
    ui: &Ui,
    response: &Response,
    scope: &str,
    group: &str,
    keys: impl AsRef<str>,
    kind: Kind,
) {
    let target = Target {
        id: response.id,
        owner: ui.id(),
        layer: response.layer_id,
        rect: response.rect,
        clip: ui.clip_rect(),
        scope: scope.into(),
        group: group.into(),
        keys: keys.as_ref().to_ascii_uppercase(),
        kind,
        enabled: response.enabled() && response.sense.is_focusable(),
        focusable: response.sense.is_focusable(),
        visible: ui.is_visible() && !ui.is_sizing_pass(),
        left_parent: None,
    };
    ui.ctx().data_mut(|data| {
        let state = data.get_temp_mut_or_default::<State>(Id::new(STATE));
        if let Some(old) = state.targets.iter_mut().find(|old| old.id == target.id) {
            *old = target;
        } else {
            state.targets.push(target);
        }
    });
    if response.has_focus() {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            0.0,
            Stroke::new(2.0_f32, Color32::from_rgb(0, 80, 160)),
            StrokeKind::Inside,
        );
    }
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
}

/// Associate a side-pane choice with the command that opened that pane.
pub(super) fn return_left_to(ui: &Ui, response: &Response, parent_keys: &str) {
    ui.ctx().data_mut(|data| {
        let state = data.get_temp_mut_or_default::<State>(Id::new(STATE));
        let Some(child) = state
            .targets
            .iter()
            .position(|target| target.id == response.id)
        else {
            return;
        };
        let parent = state
            .targets
            .iter()
            .find(|target| {
                target.scope == state.targets[child].scope
                    && target.group == state.targets[child].group
                    && target.keys == parent_keys
            })
            .map(|target| target.id);
        state.targets[child].left_parent = parent;
    });
}

pub(super) fn begin_frame(ctx: &Context) {
    let mut state = read(ctx);
    if !state.navigation_events.is_empty() {
        ctx.input_mut(|input| {
            for (index, event) in std::mem::take(&mut state.navigation_events) {
                input.events.insert(index.min(input.events.len()), event);
            }
        });
    }
    if !state.deferred_events.is_empty() {
        ctx.input_mut(|input| {
            input
                .events
                .splice(0..0, std::mem::take(&mut state.deferred_events))
                .for_each(drop)
        });
    }
    state.popup_was_open = actual_popup_open(ctx, &state);
    state.previous = std::mem::take(&mut state.targets);
    write(ctx, state);
}

/// Keep ribbon navigation out of egui's earlier automatic focus traversal.
/// Our controller handles these same events after the input pass has begun.
pub(super) fn raw_input(ctx: &Context, input: &mut RawInput) {
    let mut state = read(ctx);
    if !state.navigation_tail.is_empty() {
        input
            .events
            .splice(0..0, std::mem::take(&mut state.navigation_tail))
            .for_each(drop);
    }
    if !state.queued_keys.is_empty() {
        // Finish the earlier keytip activation before handling newer commands.
        // Its next key needs the widgets registered by the preceding pass.
        state.navigation_tail = std::mem::take(&mut input.events);
        ctx.request_repaint();
        write(ctx, state);
        return;
    }
    let focused = ctx.memory(|memory| memory.focused());
    let editing = state
        .targets
        .iter()
        .any(|target| Some(target.id) == focused && target.kind.is_text_input());
    let owns_focus = state
        .targets
        .iter()
        .any(|target| Some(target.id) == focused && !target.kind.is_text_input());
    let owns_menu = state.targets.iter().any(|target| {
        matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. })
            && egui::menu::BarState::load(ctx, target.owner).is_some()
    });
    let enters_ribbon = input.events.iter().any(|event| {
        matches!(event,
        Event::Key { key, pressed: true, modifiers, .. }
            if *key == Key::F10 || (modifiers.alt && key.name().len() == 1))
    });
    if state.levels.is_empty() && !owns_focus && !owns_menu && !enters_ribbon {
        write(ctx, state);
        return;
    }
    let navigation = |event: &Event| {
        matches!(
            event,
            Event::Key {
                key: Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Tab,
                pressed: true,
                ..
            }
        )
    };
    // Any ribbon key can open a new scope or focus a text field. Keep each key
    // with its associated text events, then release the following key only
    // after that activation has been rendered. Collecting all letter keys
    // first loses digits typed immediately after a numeric-field keytip.
    let boundary = input
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event, Event::Key { pressed: true, .. }))
        .nth(1)
        .map(|(index, _)| index);
    if let Some(index) = boundary.filter(|_| !editing) {
        state.navigation_tail = input.events.split_off(index);
        ctx.request_repaint();
    }
    let mut index = 0;
    input.events.retain(|event| {
        let retain = !navigation(event);
        if !retain {
            state.navigation_events.push((index, event.clone()));
        }
        index += 1;
        retain
    });
    write(ctx, state);
}

pub(super) fn active(ctx: &Context) -> bool {
    !read(ctx).levels.is_empty()
}

/// egui keeps menu-bar popups separately from ordinary popup memory.
pub(super) fn popup_open(ctx: &Context) -> bool {
    let state = read(ctx);
    state.popup_was_open || actual_popup_open(ctx, &state)
}

fn actual_popup_open(ctx: &Context, state: &State) -> bool {
    if ctx.memory(|memory| memory.any_popup_open()) || ctx.is_context_menu_open() {
        return true;
    }
    state.targets.iter().chain(&state.previous).any(|target| {
        matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. })
            && egui::menu::BarState::load(ctx, target.owner).is_some()
    })
}

pub(super) fn mark_alt_used(ctx: &Context) {
    let mut state = read(ctx);
    state.alt_down = ctx.input(|input| input.modifiers.alt);
    state.alt_used = true;
    write(ctx, state);
}

pub(super) fn enter_ribbon(ctx: &Context, origin: Id) {
    let mut state = read(ctx);
    enter(&mut state, origin, false);
    write(ctx, state);
}

pub(super) fn switch_tab(ctx: &Context, scope: &str) {
    let mut state = read(ctx);
    if !state.levels.is_empty() {
        for level in state.levels.iter().rev() {
            if level.opener.as_ref().is_some_and(|target| {
                matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. })
            }) {
                close_level(ctx, level);
            }
        }
        state.levels.truncate(1);
        let opener = state
            .previous
            .iter()
            .find(|target| matches!(target.kind, Kind::Tab { scope: child } if child == scope))
            .cloned();
        state.levels.push(Level {
            scope: scope.into(),
            opener,
        });
        state.focus_first = true;
        state.prefix.clear();
        state.queued_keys.clear();
    }
    write(ctx, state);
}

pub(super) fn cancel(ctx: &Context, restore: bool) {
    let mut state = read(ctx);
    close_all(ctx, &state);
    state.levels.clear();
    state.prefix.clear();
    state.queued_keys.clear();
    state.tips = false;
    state.restore_at_end = restore;
    write(ctx, state);
}

fn close_level(ctx: &Context, level: &Level) {
    if let Some(opener) = &level.opener {
        if matches!(opener.kind, Kind::Tab { .. }) {
            ctx.data_mut(|data| data.insert_temp(Id::new("paint10-ribbon-revealed"), false));
        }
        if matches!(opener.kind, Kind::Menu { .. } | Kind::PopupGroup { .. }) {
            egui::menu::BarState::default().store(ctx, opener.owner);
            ctx.memory_mut(|memory| memory.close_popup());
        }
    }
}

fn close_all(ctx: &Context, state: &State) {
    for level in state.levels.iter().rev() {
        close_level(ctx, level);
    }
}

fn enter(state: &mut State, origin: Id, tips: bool) {
    if state.levels.is_empty() {
        state.origin = Some(origin);
        state.levels.push(Level {
            scope: "tabs".into(),
            opener: None,
        });
        state.focus_first = true;
    }
    state.tips = tips;
    state.prefix.clear();
    state.queued_keys.clear();
}

fn focus(ctx: &Context, target: &Target) {
    ctx.memory_mut(|memory| memory.request_focus(target.id));
    ctx.request_repaint();
}

fn activate(ctx: &Context, state: &mut State, target: &Target, descend: bool) {
    state.prefix.clear();
    if !target.enabled && target.visible {
        return;
    }
    focus(ctx, target);
    if target.kind.is_text_input() {
        // An explicit field keytip supersedes the popup's pending initial
        // focus, including when it arrives immediately after its sizing pass.
        state.focus_first = false;
        state.tips = false;
        let mut editor = TextEdit::load_state(ctx, target.id).unwrap_or_default();
        editor
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(usize::MAX),
            )));
        editor.store(ctx, target.id);
        return;
    }
    state.activation = Some(Activation {
        id: target.id,
        parent_depth: state.levels.len(),
    });
    let already_open = matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. })
        && egui::menu::BarState::load(ctx, target.owner)
            .as_ref()
            .is_some_and(|menu| menu.id == target.id);
    if !already_open {
        ctx.input_mut(|input| {
            input.events.push(Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
        });
        // Some click responses observe Enter without consuming it. Remove our
        // activation event after the ribbon renders, before text regains focus.
        state.consume_enter_at_end = true;
    }
    match target.kind {
        Kind::Menu { scope } | Kind::PopupGroup { scope } | Kind::Tab { scope } if descend => {
            if matches!(target.kind, Kind::Tab { .. }) {
                state.levels.truncate(1);
            }
            state.levels.push(Level {
                scope: scope.into(),
                opener: Some(target.clone()),
            });
            state.focus_first = true;
        }
        Kind::Tab { .. } => {}
        _ => {
            // Keep focus on the command until its real widget consumes Enter.
            // Focus returns to the editor after the UI has executed that command.
            state.restore_at_end = true;
            state.tips = false;
        }
    }
}

fn back(ctx: &Context, state: &mut State) {
    state.prefix.clear();
    state.queued_keys.clear();
    if let Some(level) = state.levels.pop() {
        close_level(ctx, &level);
        if let Some(opener) = level.opener {
            focus(ctx, &opener);
        }
    }
    state.focus_first = false;
    if state.levels.is_empty() {
        state.restore_at_end = true;
        state.tips = false;
    }
}

fn consume(ctx: &Context, key: Key, modifiers: Modifiers) -> bool {
    ctx.input_mut(|input| super::shortcuts::consume_shortcut(input, modifiers, key))
}

fn levels_for_focus(ctx: &Context, state: &State, target: &Target) -> Vec<Level> {
    let mut scope = match target.kind {
        Kind::Menu { scope } | Kind::PopupGroup { scope }
            if egui::menu::BarState::load(ctx, target.owner)
                .as_ref()
                .is_some_and(|menu| menu.id == target.id) =>
        {
            scope.to_owned()
        }
        _ => target.scope.clone(),
    };
    let mut levels: Vec<Level> = Vec::new();
    while !levels.iter().any(|level| level.scope == scope) {
        let opener = state
            .previous
            .iter()
            .find(|candidate| {
                matches!(candidate.kind,
                Kind::Menu { scope: child }
                    | Kind::PopupGroup { scope: child }
                    | Kind::Tab { scope: child } if child == scope)
            })
            .cloned();
        let parent = opener.as_ref().map(|opener| opener.scope.clone());
        levels.push(Level { scope, opener });
        let Some(parent) = parent else { break };
        scope = parent;
    }
    levels.reverse();
    levels
}

fn clicked_input_with_pending_text(ctx: &Context, state: &State) -> Option<Target> {
    let point = ctx.input(|input| {
        input.events.iter().enumerate().find_map(|(index, event)| {
            let Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                ..
            } = event
            else {
                return None;
            };
            input.events[index + 1..]
                .iter()
                .any(|event| matches!(event, Event::Text(_) | Event::Paste(_)))
                .then_some(*pos)
        })
    })?;
    state.previous.iter().find(|target| {
        target.kind.is_text_input()
            && target.enabled
            && target.visible
            && target.rect.contains(point)
            && target.clip.contains(point)
            && ctx.layer_id_at(point) == Some(target.layer)
            && ctx.memory(|memory| memory.allows_interaction(target.layer))
            && (!state.popup_was_open || state.previous.iter().any(|opener| {
                matches!(opener.kind, Kind::Menu { scope } | Kind::PopupGroup { scope } if scope == target.scope)
                    && egui::menu::BarState::load(ctx, opener.owner).is_some()
            }))
    }).cloned()
}

/// Process previous-frame targets before painting the next ribbon frame.
pub(super) fn current_tab(ctx: &Context, scope: &str) {
    let mut state = read(ctx);
    state.current_tab = scope.into();
    write(ctx, state);
}

pub(super) fn keyboard(ctx: &Context, origin: Id) -> bool {
    let mut state = read(ctx);
    if let Some(id) = state.deferred_text_commit.take() {
        if ctx.memory(|memory| memory.has_focus(id))
            && !ctx.input(|input| input.pointer.any_pressed())
        {
            // A DragValue cannot report lost_focus in the same frame it first
            // gains focus. Submit its retained text on this next repaint.
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    if matches!(
                        event,
                        Event::Key { .. }
                            | Event::Text(_)
                            | Event::Copy
                            | Event::Cut
                            | Event::Paste(_)
                    ) {
                        state.deferred_events.push(event.clone());
                        false
                    } else {
                        true
                    }
                });
                input.events.push(Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            });
            state.restore_at_end = true;
            state.consume_enter_at_end = true;
            ctx.request_repaint();
            write(ctx, state);
            return true;
        }
    }
    let alt = ctx.input(|input| input.modifiers.alt);
    let other = ctx.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, Event::Key { pressed: true, .. }))
    });
    if alt && !state.alt_down {
        state.alt_used = false;
    }
    state.alt_used |= alt && other;
    let alt_released = state.alt_down && !alt && !state.alt_used;
    state.alt_down = alt;
    // The browser may translate a standalone Alt release into F10. Consume
    // that key even if an earlier pass already observed the Alt-down state.
    let f10 = consume(ctx, Key::F10, Modifiers::NONE);
    if alt_released || f10 {
        if state.levels.is_empty() {
            enter(&mut state, origin, true);
        } else {
            close_all(ctx, &state);
            state.levels.clear();
            state.restore_at_end = true;
            state.tips = false;
            write(ctx, state);
            return true;
        }
    }
    if ctx.input(|input| input.pointer.any_pressed()) {
        state.levels.clear();
        state.prefix.clear();
        state.tips = false;
        state.queued_keys.clear();
        if let Some(target) = clicked_input_with_pending_text(ctx, &state) {
            // A clicked DragValue normally becomes a TextEdit next frame. If
            // typing already arrived with that click, render the editor now.
            // A held pointer press alone keeps the normal drag-to-adjust path.
            focus(ctx, &target);
            if ctx.input(|input| input.key_pressed(Key::Enter)) {
                state.origin = Some(origin);
                state.deferred_text_commit = Some(target.id);
                consume(ctx, Key::Enter, Modifiers::NONE);
                ctx.request_repaint();
            }
        }
        write(ctx, state);
        return false;
    }
    // egui may clear ordinary button focus before our Escape handler runs.
    let focused = ctx.memory(|memory| memory.focused()).or(state.focused);
    if ctx.is_context_menu_open() && ctx.input(|input| input.key_pressed(Key::Escape)) {
        // Context menus own their close operation. Let their actual response
        // see Escape, then consume it before the document editor runs.
        state.levels.clear();
        state.prefix.clear();
        state.queued_keys.clear();
        state.tips = false;
        state.origin.get_or_insert(origin);
        state.restore_at_end = true;
        state.consume_escape_at_end = true;
        write(ctx, state);
        return true;
    }
    if state.levels.is_empty() {
        // egui mouse menu buttons deliberately do not take keyboard focus.
        // Recover the actual open menu when keyboard navigation begins there.
        let navigation = ctx.input(|input| {
            [
                Key::Tab,
                Key::ArrowLeft,
                Key::ArrowRight,
                Key::ArrowUp,
                Key::ArrowDown,
                Key::Escape,
            ]
            .iter()
            .any(|key| input.key_pressed(*key))
        });
        if navigation {
            let menu_levels = state
                .previous
                .iter()
                .filter(|target| {
                    matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. })
                        && egui::menu::BarState::load(ctx, target.owner)
                            .as_ref()
                            .is_some_and(|menu| menu.id == target.id)
                })
                .map(|target| levels_for_focus(ctx, &state, target))
                .max_by_key(Vec::len);
            if let Some(levels) = menu_levels {
                state.origin = Some(origin);
                state.levels = levels;
                state.tips = false;
            }
        }
    }
    let alt_command = ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(event,
            Event::Key { modifiers, pressed: true, key, .. }
                if modifiers.alt && !modifiers.ctrl && !modifiers.command && !modifiers.shift && key.name().len() == 1)
        })
    });
    if alt_command {
        // An explicit Alt+tab letter starts at the ribbon tabs even when a
        // previous keytip prefix or nested menu is still pending.
        close_all(ctx, &state);
        state.levels.clear();
        state.restore_at_end = false;
        enter(&mut state, origin, true);
    }
    if state.levels.is_empty() {
        if let Some(target) = state
            .previous
            .iter()
            .find(|target| Some(target.id) == focused)
        {
            let navigation = ctx.input(|input| {
                [
                    Key::Tab,
                    Key::ArrowLeft,
                    Key::ArrowRight,
                    Key::ArrowUp,
                    Key::ArrowDown,
                    Key::Escape,
                ]
                .iter()
                .any(|key| input.key_pressed(*key))
            });
            if !navigation {
                write(ctx, state);
                return false;
            }
            let levels = levels_for_focus(ctx, &state, target);
            state.origin = Some(origin);
            state.levels = levels;
            state.tips = false;
        } else {
            write(ctx, state);
            return false;
        }
    }
    if ctx.input(|input| input.key_pressed(Key::Escape))
        && state
            .previous
            .iter()
            .any(|target| Some(target.id) == focused && target.kind == Kind::NumericInput)
    {
        // Let DragValue see Escape while it loses focus, so it discards its
        // pending text instead of treating this as a normal committed blur.
        back(ctx, &mut state);
        state.consume_escape_at_end = true;
        write(ctx, state);
        return true;
    }
    if consume(ctx, Key::Escape, Modifiers::NONE) {
        back(ctx, &mut state);
        write(ctx, state);
        return true;
    }
    let scope = state.levels.last().unwrap().scope.clone();
    let targets: Vec<_> = state
        .previous
        .iter()
        .filter(|target| target.scope == scope)
        .cloned()
        .collect();
    let selected = targets.iter().find(|target| Some(target.id) == focused);
    let editing = selected.is_some_and(|target| target.kind.is_text_input()) && !state.tips;
    if ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(event,
        Event::Key { modifiers, pressed: true, .. } if modifiers.ctrl || modifiers.command)
        })
    }) {
        if editing {
            // Selection, clipboard and word navigation shortcuts belong to the
            // focused field. Closing its popup here would discard the editor
            // before it can process Ctrl+A (or Command+A on macOS).
            write(ctx, state);
            return true;
        }
        close_all(ctx, &state);
        state.levels.clear();
        state.prefix.clear();
        state.tips = false;
        state.popup_was_open = false;
        // Return global shortcuts to the editor before ribbon buttons render.
        // Otherwise an opener can treat Ctrl+Enter as another menu activation
        // and consume the text editor's commit command.
        ctx.memory_mut(|memory| memory.request_focus(state.origin.unwrap_or(origin)));
        write(ctx, state);
        return false;
    }
    if editing && consume(ctx, Key::ArrowDown, Modifiers::ALT) {
        if let Some(input) = selected {
            let dropdown = targets.iter().find(|target| {
                target.group == input.group
                    && target.enabled
                    && matches!(target.kind, Kind::Menu { .. })
                    && (target.rect.center().y - input.rect.center().y).abs()
                        < input.rect.height() / 2.0
            });
            if let Some(dropdown) = dropdown {
                activate(ctx, &mut state, dropdown, true);
            }
        }
        write(ctx, state);
        return true;
    }
    if editing && ctx.input(|input| input.events.iter().any(|event| matches!(event,
        Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.matches_exact(Modifiers::NONE)))) {
        // Let the real field validate its pending entry first, then return to
        // the document without delivering that Enter to the multiline editor.
        state.consume_enter_at_end = true;
        state.restore_at_end = true;
        write(ctx, state);
        return false;
    }
    let backward = consume(ctx, Key::Tab, Modifiers::SHIFT);
    if backward || consume(ctx, Key::Tab, Modifiers::NONE) {
        state.focus_first = false;
        let mut groups = Vec::new();
        for target in targets.iter().filter(|target| target.enabled) {
            if !groups.contains(&target.group.as_str()) {
                groups.push(target.group.as_str());
            }
        }
        if !groups.is_empty() {
            let current =
                selected.and_then(|target| groups.iter().position(|group| *group == target.group));
            let next = current.map_or(0, |index| {
                (index + if backward { groups.len() - 1 } else { 1 }) % groups.len()
            });
            if let Some(target) = targets
                .iter()
                .find(|target| target.enabled && target.group == groups[next])
            {
                focus(ctx, target);
            }
        }
        state.tips = false;
        write(ctx, state);
        return true;
    }
    if !editing {
        for key in [
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
            Key::ArrowDown,
        ] {
            if consume(ctx, key, Modifiers::NONE) {
                state.tips = false;
                state.focus_first = false;
                if key == Key::ArrowLeft
                    && state.levels.last().is_some_and(|level| {
                        level.opener.as_ref().is_some_and(|target| {
                            matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. })
                        })
                    })
                {
                    let left_pane =
                        selected
                            .filter(|target| target.scope == "file")
                            .and_then(|selected| {
                                selected
                                    .left_parent
                                    .and_then(|parent| {
                                        targets
                                            .iter()
                                            .find(|target| target.id == parent && target.enabled)
                                    })
                                    .or_else(|| {
                                        targets
                                            .iter()
                                            .filter(|target| {
                                                target.enabled
                                                    && target.group == selected.group
                                                    && target.rect.right()
                                                        <= selected.rect.left() + 1.0
                                                    && target.rect.center().x
                                                        < selected.rect.center().x
                                            })
                                            .min_by(|left, right| {
                                                left.rect
                                                    .center()
                                                    .distance_sq(selected.rect.center())
                                                    .total_cmp(
                                                        &right
                                                            .rect
                                                            .center()
                                                            .distance_sq(selected.rect.center()),
                                                    )
                                            })
                                    })
                            });
                    if let Some(target) = left_pane {
                        focus(ctx, target);
                    } else {
                        back(ctx, &mut state);
                    }
                } else if let Some(target) = selected.filter(|target| {
                    matches!(key, Key::ArrowRight | Key::ArrowDown)
                        && matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. })
                        || key == Key::ArrowDown && matches!(target.kind, Kind::Tab { .. })
                }) {
                    activate(ctx, &mut state, target, true);
                } else if let Some(target) = next_arrow(&targets, selected, key) {
                    if matches!(target.kind, Kind::Tab { .. }) {
                        activate(ctx, &mut state, target, false);
                    } else {
                        focus(ctx, target);
                    }
                }
                write(ctx, state);
                return true;
            }
        }
        if consume(ctx, Key::Enter, Modifiers::NONE) || consume(ctx, Key::Space, Modifiers::NONE) {
            let exact = targets.iter().find(|target| {
                state.tips && !state.prefix.is_empty() && target.keys == state.prefix
            });
            if let Some(target) = exact.or(selected) {
                activate(ctx, &mut state, target, true);
            }
            write(ctx, state);
            return true;
        }
    }
    if state.tips {
        let mut keys = std::mem::take(&mut state.queued_keys);
        keys.extend(ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    Event::Key {
                        key,
                        modifiers,
                        pressed: true,
                        ..
                    } if !modifiers.ctrl && !modifiers.command && key.name().len() == 1 => {
                        Some(*key)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        }));
        ctx.input_mut(|input| input.events.retain(|event| !matches!(event,
            Event::Key { key, pressed: true, modifiers, .. } if !modifiers.ctrl && !modifiers.command && key.name().len() == 1)));
        for (index, key) in keys.iter().copied().enumerate() {
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    !matches!(event,
                Event::Key { key: other, pressed: true, .. } if *other == key)
                })
            });
            state.prefix.push_str(key.name());
            let matches: Vec<_> = targets
                .iter()
                .filter(|target| target.keys.starts_with(&state.prefix))
                .collect();
            if matches.is_empty() {
                state.prefix.clear();
            } else if matches.len() == 1 && matches[0].keys == state.prefix {
                activate(ctx, &mut state, matches[0], true);
                if !state.restore_at_end && state.tips {
                    state.queued_keys.extend_from_slice(&keys[index + 1..]);
                    if !state.queued_keys.is_empty() {
                        ctx.request_repaint();
                    }
                }
                break;
            }
        }
        ctx.input_mut(|input| {
            input
                .events
                .retain(|event| !matches!(event, Event::Text(_)))
        });
    }
    let handled = state.tips;
    write(ctx, state);
    handled
}

fn next_arrow<'a>(
    targets: &'a [Target],
    selected: Option<&Target>,
    key: Key,
) -> Option<&'a Target> {
    let selected = selected.or_else(|| {
        targets
            .iter()
            .find(|target| target.enabled || !target.visible)
    })?;
    if key == Key::ArrowRight && selected.scope == "file" {
        if let Some(child) = targets.iter().find(|target| {
            target.left_parent == Some(selected.id) && target.enabled && target.visible
        }) {
            return Some(child);
        }
    }
    let candidates: Vec<_> = targets
        .iter()
        .filter(|target| {
            (target.enabled || !target.visible)
                && target.group == selected.group
                && (selected.scope != "file"
                    || key != Key::ArrowRight
                    || target.rect.left() >= selected.rect.right() - 1.0)
        })
        .collect();
    let axis = |target: &Target| {
        if matches!(key, Key::ArrowLeft | Key::ArrowRight) {
            target.rect.center().x
        } else {
            target.rect.center().y
        }
    };
    let direction = if matches!(key, Key::ArrowLeft | Key::ArrowUp) {
        -1.0
    } else {
        1.0
    };
    candidates
        .iter()
        .copied()
        .filter(|target| (axis(target) - axis(selected)) * direction > 1.0)
        .min_by(|a, b| {
            let distance =
                |target: &Target| target.rect.center().distance_sq(selected.rect.center());
            distance(a).total_cmp(&distance(b))
        })
        .or_else(|| {
            candidates
                .into_iter()
                .min_by(|a, b| (axis(a) * direction).total_cmp(&(axis(b) * direction)))
        })
}

pub(super) fn finish_frame(ctx: &Context) {
    let mut state = read(ctx);
    for previous in state
        .previous
        .iter()
        .filter(|target| matches!(target.kind, Kind::Menu { .. } | Kind::PopupGroup { .. }))
    {
        let still_present = state.targets.iter().any(|target| {
            target.id == previous.id
                && target.kind == previous.kind
                && target.scope == previous.scope
        });
        if !still_present
            && egui::menu::BarState::load(ctx, previous.owner)
                .as_ref()
                .is_some_and(|menu| menu.id == previous.id)
        {
            // Mouse-opened menus have no keytip level to unwind. Discard their
            // dormant state when adaptive layout removes the actual opener.
            egui::menu::BarState::default().store(ctx, previous.owner);
        }
    }
    if let Some(activation) = state.activation.take() {
        // A menu's previous sizing pass cannot tell us whether a command is
        // enabled. The actual interactive response is the final authority.
        if state
            .targets
            .iter()
            .find(|target| target.id == activation.id)
            .is_none_or(|target| !target.enabled || !target.visible)
        {
            state.levels.truncate(activation.parent_depth);
            state.restore_at_end = false;
            state.tips = true;
            state.focus_first = true;
            state.queued_keys.clear();
        }
    }
    if state.consume_enter_at_end {
        consume(ctx, Key::Enter, Modifiers::NONE);
        state.consume_enter_at_end = false;
    }
    if state.consume_escape_at_end {
        consume(ctx, Key::Escape, Modifiers::NONE);
        state.consume_escape_at_end = false;
    }
    if state.restore_at_end {
        close_all(ctx, &state);
        state.levels.clear();
        state.prefix.clear();
        state.queued_keys.clear();
        state.restore_at_end = false;
        ctx.memory_mut(|memory| {
            memory.request_focus(state.origin.unwrap_or_else(|| Id::new("canvas")))
        });
    }
    // Responsive layout may remove the opener of a compressed group. Unwind
    // only the vanished menu levels and focus its replacement in the parent.
    while state
        .levels
        .last()
        .and_then(|level| level.opener.as_ref())
        .is_some_and(|opener| {
            !state.targets.iter().any(|target| {
                target.id == opener.id && target.kind == opener.kind && target.scope == opener.scope
            })
        })
    {
        let level = state.levels.pop().unwrap();
        close_level(ctx, &level);
        if let (Some(opener), Some(parent)) = (level.opener, state.levels.last()) {
            if let Some(target) = state.targets.iter().find(|target| {
                target.scope == parent.scope
                    && target.group == opener.group
                    && target.enabled
                    && target.visible
            }) {
                focus(ctx, target);
            }
        }
        state.prefix.clear();
        state.queued_keys.clear();
        state.focus_first = false;
    }
    if let Some(level) = state.levels.last() {
        if state.focus_first {
            let preferred_tab = state.targets.iter().find(|target| {
                level.scope == "tabs"
                    && target.enabled
                    && target.visible
                    && matches!(target.kind, Kind::Tab { scope } if scope == state.current_tab)
            });
            if let Some(target) = preferred_tab.or_else(|| {
                state
                    .targets
                    .iter()
                    .find(|target| target.scope == level.scope && target.enabled && target.visible)
            }) {
                focus(ctx, target);
                state.focus_first = false;
            }
        }
        if state.tips {
            let painter = ctx.layer_painter(LayerId::new(
                Order::Tooltip,
                Id::new("paint10_keytip_overlays"),
            ));
            for target in state.targets.iter().filter(|target| {
                target.scope == level.scope
                    && target.visible
                    && target.focusable
                    && target.keys.starts_with(&state.prefix)
                    && !target.keys.is_empty()
            }) {
                let visible = target.clip.intersect(ctx.screen_rect());
                if !visible.contains_rect(target.rect.shrink(0.5)) {
                    continue;
                }
                let compact = target.rect.width() <= 36.0 || target.rect.height() <= 24.0;
                let text = painter.layout_no_wrap(
                    target.keys.clone(),
                    FontId::proportional(if compact { 9.0 } else { 10.5 }),
                    if target.enabled {
                        Color32::BLACK
                    } else {
                        Color32::GRAY
                    },
                );
                let size = text.size() + vec2(if compact { 4.0 } else { 6.0 }, 2.0);
                let center = if target.rect.width() <= 36.0 {
                    target.rect.center()
                } else if target.rect.height() <= 24.0 {
                    target.rect.right_center() - vec2(size.x / 2.0 + 2.0, 0.0)
                } else {
                    target.rect.center_bottom() - vec2(0.0, size.y / 2.0 + 2.0)
                };
                let rect = Rect::from_center_size(center, size);
                if !visible.contains_rect(rect) {
                    continue;
                }
                painter.rect(
                    rect,
                    2.0,
                    Color32::from_rgb(255, 255, 228),
                    Stroke::new(1.0_f32, Color32::from_gray(90)),
                    StrokeKind::Inside,
                );
                painter.galley(rect.center() - text.size() / 2.0, text, Color32::BLACK);
            }
        }
    }
    state.focused = ctx.memory(|memory| memory.focused());
    write(ctx, state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fixture {
        view: bool,
        pencil: bool,
        chosen: bool,
        text: String,
    }

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    fn frame(ctx: &Context, fixture: &mut Fixture, events: Vec<Event>) -> FullOutput {
        frame_with_modifiers(ctx, fixture, events, Modifiers::NONE)
    }

    fn frame_with_modifiers(
        ctx: &Context,
        fixture: &mut Fixture,
        events: Vec<Event>,
        modifiers: Modifiers,
    ) -> FullOutput {
        let mut input = RawInput {
            events,
            modifiers,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
            ..Default::default()
        };
        raw_input(ctx, &mut input);
        ctx.run(input, |ctx| {
            begin_frame(ctx);
            current_tab(ctx, if fixture.view { "view" } else { "home" });
            keyboard(ctx, Id::new("text_input"));
            TopBottomPanel::top("fixture_tabs").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let home = ui.selectable_label(!fixture.view, "Home");
                    register(ui, &home, "tabs", "Tabs", "H", Kind::Tab { scope: "home" });
                    if home.clicked() {
                        fixture.view = false;
                    }
                    let view = ui.selectable_label(fixture.view, "View");
                    register(ui, &view, "tabs", "Tabs", "V", Kind::Tab { scope: "view" });
                    if view.clicked() {
                        fixture.view = true;
                    }
                });
            });
            TopBottomPanel::top("fixture_ribbon").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if fixture.view {
                        let zoom = ui.button("Zoom in");
                        register(ui, &zoom, "view", "Zoom", "I", Kind::Button);
                        return;
                    }
                    let (rect, _) = ui.allocate_exact_size(vec2(30.0, 35.0), Sense::hover());
                    let pencil = icons::button(
                        ui,
                        "fixture_pencil",
                        rect,
                        Icon::Tool(Tool::Pencil),
                        "",
                        fixture.pencil,
                        true,
                    );
                    register(ui, &pencil, "home", "Tools", "P", Kind::Button);
                    fixture.pencil |= pencil.clicked();
                    pencil.context_menu(|ui| {
                        let item = ui.button("Tool settings");
                        register(ui, &item, "tool_context", "Tools", "S", Kind::Button);
                    });
                    let disabled = ui.add_enabled(false, Button::new("Disabled"));
                    register(ui, &disabled, "home", "Tools", "D", Kind::Button);
                    assert!(!disabled.clicked());
                    let menu = egui::menu::menu_custom_button(ui, Button::new("Options"), |ui| {
                        let first = ui.button("First option");
                        register(ui, &first, "options", "Options", "F", Kind::Button);
                        let next = ui.button("Second option");
                        register(ui, &next, "options", "Options", "S", Kind::Button);
                        if next.clicked() {
                            fixture.chosen = true;
                            ui.close_menu();
                        }
                    });
                    register(
                        ui,
                        &menu.response,
                        "home",
                        "Options",
                        "O",
                        Kind::Menu { scope: "options" },
                    );
                    let group = egui::menu::menu_custom_button(ui, Button::new("Colors"), |ui| {
                        let red = ui.button("Red");
                        register(ui, &red, "home_colors", "Colors", "R", Kind::Button);
                        let nested =
                            egui::menu::menu_custom_button(ui, Button::new("More colors"), |ui| {
                                let blue = ui.button("Blue");
                                register(ui, &blue, "more_colors", "Colors", "B", Kind::Button);
                                if blue.clicked() {
                                    fixture.chosen = true;
                                    ui.close_menu();
                                }
                            });
                        register(
                            ui,
                            &nested.response,
                            "home_colors",
                            "Colors",
                            "M",
                            Kind::Menu {
                                scope: "more_colors",
                            },
                        );
                    });
                    register(
                        ui,
                        &group.response,
                        "home",
                        "Colors",
                        "ZC",
                        Kind::PopupGroup {
                            scope: "home_colors",
                        },
                    );
                });
            });
            CentralPanel::default().show(ctx, |ui| {
                ui.add(TextEdit::singleline(&mut fixture.text).id(Id::new("text_input")));
            });
            finish_frame(ctx);
        })
    }

    fn warm(ctx: &Context, fixture: &mut Fixture) {
        for _ in 0..3 {
            frame(ctx, fixture, vec![]);
        }
    }

    fn visible(output: &FullOutput, text: &str) -> bool {
        output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(painted) if painted.galley.job.text == text))
    }

    #[test]
    fn anchored_tips_activate_real_icon_widgets_and_preserve_editor_text() {
        let ctx = Context::default();
        let mut fixture = Fixture {
            text: "Hello".into(),
            ..Default::default()
        };
        warm(&ctx, &mut fixture);
        ctx.memory_mut(|memory| memory.request_focus(Id::new("text_input")));
        let tabs = frame(&ctx, &mut fixture, vec![key(Key::F10)]);
        assert!(visible(&tabs, "H"));
        assert!(!visible(&tabs, "Ribbon · Key tips"));
        let home = frame(
            &ctx,
            &mut fixture,
            vec![key(Key::H), Event::Text("h".into())],
        );
        assert!(visible(&home, "P"));
        frame(&ctx, &mut fixture, vec![key(Key::D)]);
        assert!(active(&ctx));
        frame(
            &ctx,
            &mut fixture,
            vec![key(Key::P), Event::Text("p".into())],
        );
        assert!(fixture.pencil);
        assert_eq!(fixture.text, "Hello");
        assert!(!active(&ctx));
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );
    }

    #[test]
    fn releasing_alt_shows_anchored_tips_and_escape_restores_editor_focus() {
        let ctx = Context::default();
        let mut fixture = Fixture {
            text: "Keep this".into(),
            ..Default::default()
        };
        warm(&ctx, &mut fixture);
        ctx.memory_mut(|memory| memory.request_focus(Id::new("text_input")));
        frame_with_modifiers(&ctx, &mut fixture, vec![], Modifiers::ALT);
        let released = frame(&ctx, &mut fixture, vec![]);
        assert!(visible(&released, "H"));
        assert!(active(&ctx));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        assert!(!active(&ctx));
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );
        assert_eq!(fixture.text, "Keep this");
    }

    #[test]
    fn alt_release_with_browser_f10_consumes_the_toggle_once() {
        for alt_was_rendered in [false, true] {
            let ctx = Context::default();
            let mut fixture = Fixture {
                text: "Keep this".into(),
                ..Default::default()
            };
            warm(&ctx, &mut fixture);
            if alt_was_rendered {
                frame_with_modifiers(&ctx, &mut fixture, vec![], Modifiers::ALT);
            }
            let released = frame(&ctx, &mut fixture, vec![key(Key::F10)]);
            assert!(visible(&released, "H"));
            assert!(active(&ctx));
            assert!(
                !ctx.input(|input| input.key_pressed(Key::F10)),
                "The Alt release must consume its accompanying browser toggle"
            );
            let settled = frame(&ctx, &mut fixture, vec![]);
            assert!(visible(&settled, "H"));
            frame(&ctx, &mut fixture, vec![key(Key::H)]);
            frame(&ctx, &mut fixture, vec![key(Key::P)]);
            assert!(fixture.pencil);
            assert!(!active(&ctx));
            assert_eq!(fixture.text, "Keep this");
        }
    }

    #[test]
    fn menu_arrows_and_enter_use_actual_menu_contents() {
        let ctx = Context::default();
        let mut fixture = Fixture::default();
        warm(&ctx, &mut fixture);
        for code in [Key::F10, Key::H, Key::O] {
            frame(&ctx, &mut fixture, vec![key(code)]);
        }
        let output = frame(&ctx, &mut fixture, vec![key(Key::ArrowDown)]);
        assert!(visible(&output, "First option"));
        assert!(popup_open(&ctx));
        assert!(!ctx.memory(|memory| memory.any_popup_open()));
        frame(&ctx, &mut fixture, vec![key(Key::Enter)]);
        assert!(fixture.chosen);
        let output = frame(&ctx, &mut fixture, vec![]);
        assert!(!visible(&output, "First option"));
        assert!(!popup_open(&ctx));
    }

    #[test]
    fn mouse_opened_menu_accepts_arrow_navigation_and_escape_closes_it() {
        let ctx = Context::default();
        let mut fixture = Fixture::default();
        warm(&ctx, &mut fixture);
        let opener = read(&ctx)
            .targets
            .into_iter()
            .find(|target| target.keys == "O")
            .unwrap();
        let pos = opener.rect.center();
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut fixture,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        warm(&ctx, &mut fixture);
        assert!(popup_open(&ctx));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        warm(&ctx, &mut fixture);
        assert!(!popup_open(&ctx));
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut fixture,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        warm(&ctx, &mut fixture);
        assert!(popup_open(&ctx));
        frame(&ctx, &mut fixture, vec![key(Key::ArrowDown)]);
        assert!(popup_open(&ctx));
        let state = read(&ctx);
        let focused = ctx.memory(|memory| memory.focused());
        assert!(state
            .targets
            .iter()
            .any(|target| target.scope == "options" && Some(target.id) == focused));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        assert!(popup_open(&ctx));
        let output = frame(&ctx, &mut fixture, vec![]);
        assert!(!popup_open(&ctx));
        assert!(!visible(&output, "First option"));
    }

    #[test]
    fn escape_closes_a_mouse_context_menu_without_reaching_the_editor() {
        let ctx = Context::default();
        let mut fixture = Fixture {
            text: "Retain text".into(),
            ..Default::default()
        };
        warm(&ctx, &mut fixture);
        let opener = read(&ctx)
            .targets
            .into_iter()
            .find(|target| target.keys == "P")
            .unwrap();
        let pos = opener.rect.center();
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut fixture,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Secondary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        warm(&ctx, &mut fixture);
        assert!(ctx.is_context_menu_open());
        let item = read(&ctx)
            .targets
            .into_iter()
            .find(|target| target.scope == "tool_context")
            .unwrap();
        ctx.memory_mut(|memory| memory.request_focus(item.id));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        assert!(!ctx.is_context_menu_open());
        assert!(!ctx.input(|input| input.key_pressed(Key::Escape)));
        assert_eq!(fixture.text, "Retain text");
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );
        frame(&ctx, &mut fixture, vec![]);
        assert!(!popup_open(&ctx));
    }

    #[test]
    fn multi_key_popup_groups_and_escape_unwind_one_level_at_a_time() {
        let ctx = Context::default();
        let mut fixture = Fixture::default();
        warm(&ctx, &mut fixture);
        for code in [Key::F10, Key::H, Key::Z] {
            frame(&ctx, &mut fixture, vec![key(code)]);
        }
        let prefix = frame(&ctx, &mut fixture, vec![]);
        assert!(visible(&prefix, "ZC"));
        assert!(!visible(&prefix, "P"));
        frame(&ctx, &mut fixture, vec![key(Key::C)]);
        warm(&ctx, &mut fixture);
        frame(&ctx, &mut fixture, vec![key(Key::M)]);
        warm(&ctx, &mut fixture);
        let child = frame(&ctx, &mut fixture, vec![]);
        assert!(visible(&child, "Blue"));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        let parent = frame(&ctx, &mut fixture, vec![]);
        assert!(!visible(&parent, "Blue"));
        assert!(visible(&parent, "Red"));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        let ribbon = frame(&ctx, &mut fixture, vec![]);
        assert!(!visible(&ribbon, "Red"));
        assert!(visible(&ribbon, "ZC"));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        assert!(active(&ctx));
        frame(&ctx, &mut fixture, vec![key(Key::Escape)]);
        assert!(!active(&ctx));
    }

    #[test]
    fn tab_skips_between_groups_and_arrows_switch_real_tabs() {
        let ctx = Context::default();
        let mut fixture = Fixture::default();
        warm(&ctx, &mut fixture);
        frame(&ctx, &mut fixture, vec![key(Key::F10)]);
        frame(&ctx, &mut fixture, vec![key(Key::ArrowRight)]);
        assert!(fixture.view);
        frame(&ctx, &mut fixture, vec![key(Key::ArrowLeft)]);
        assert!(!fixture.view);
        frame(&ctx, &mut fixture, vec![key(Key::ArrowDown)]);
        frame(&ctx, &mut fixture, vec![key(Key::Tab)]);
        let state = read(&ctx);
        let focus = ctx.memory(|memory| memory.focused());
        let focused = state
            .targets
            .iter()
            .find(|target| Some(target.id) == focus)
            .unwrap();
        assert_eq!(focused.group, "Options");
        frame(&ctx, &mut fixture, vec![key(Key::ArrowRight)]);
        warm(&ctx, &mut fixture);
        let output = frame(&ctx, &mut fixture, vec![]);
        assert!(visible(&output, "First option"));
    }

    fn app_frame(app: &mut PaintApp, ctx: &Context, width: f32, events: Vec<Event>) -> FullOutput {
        let modifiers = events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let mut input = RawInput {
            events,
            modifiers,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(width, 500.0))),
            ..Default::default()
        };
        eframe::App::raw_input_hook(app, ctx, &mut input);
        ctx.run(input, |ctx| {
            if !app.ribbon_keyboard(ctx) {
                app.shortcut(ctx);
            }
            app.titlebar(ctx);
            app.ribbon(ctx);
            app.quick_access_below(ctx);
            app.status(ctx);
            app.canvas(ctx);
            app.keyboard_menu(ctx);
            app.dialogs(ctx);
        })
    }

    fn app_warm(app: &mut PaintApp, ctx: &Context, width: f32) {
        for _ in 0..3 {
            app_frame(app, ctx, width, vec![]);
        }
    }

    fn app_keys(app: &mut PaintApp, ctx: &Context, width: f32, keys: &[Key]) {
        for code in keys {
            app_frame(app, ctx, width, vec![key(*code)]);
        }
    }

    #[test]
    fn alt_home_restarts_a_pending_sequence_and_outline_number_activates() {
        for pending in [vec![], vec![Key::J], vec![Key::O]] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.tool = Tool::Rectangle;
            app.outline = PaintStyle::None;
            app_warm(&mut app, &ctx, 1200.0);
            if !pending.is_empty() {
                app_keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::H]);
                app_keys(&mut app, &ctx, 1200.0, &pending);
            }
            let mut alt_home = key(Key::H);
            if let Event::Key { modifiers, .. } = &mut alt_home {
                *modifiers = Modifiers::ALT;
            }
            app_frame(&mut app, &ctx, 1200.0, vec![alt_home]);
            app_warm(&mut app, &ctx, 1200.0);
            assert_eq!(
                read(&ctx).levels.last().map(|level| level.scope.as_str()),
                Some("home"),
                "pending sequence: {pending:?}"
            );
            assert!(read(&ctx).prefix.is_empty());
            assert_eq!(app.tool, Tool::Rectangle);
            app_keys(&mut app, &ctx, 1200.0, &[Key::O, Key::Num7]);
            app_warm(&mut app, &ctx, 1200.0);
            assert_eq!(app.outline, PaintStyle::Watercolor);
            assert!(!active(&ctx));
        }
    }

    #[test]
    fn rapid_file_arrows_move_once_per_press_and_preserve_batched_steps() {
        for batched in [false, true] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app_warm(&mut app, &ctx, 1200.0);
            app_keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::F]);
            app_warm(&mut app, &ctx, 1200.0);
            let focused_keys = |ctx: &Context| {
                let focused = ctx.memory(|memory| memory.focused());
                read(ctx)
                    .targets
                    .iter()
                    .find(|target| Some(target.id) == focused)
                    .map(|target| target.keys.clone())
            };
            assert_eq!(focused_keys(&ctx).as_deref(), Some("N"));
            if batched {
                app_frame(
                    &mut app,
                    &ctx,
                    1200.0,
                    vec![
                        key(Key::ArrowDown),
                        key(Key::ArrowDown),
                        key(Key::ArrowDown),
                    ],
                );
                assert_eq!(focused_keys(&ctx).as_deref(), Some("O"));
                app_frame(&mut app, &ctx, 1200.0, vec![]);
                assert_eq!(focused_keys(&ctx).as_deref(), Some("S"));
                app_frame(&mut app, &ctx, 1200.0, vec![]);
            } else {
                for expected in ["O", "S", "A"] {
                    app_keys(&mut app, &ctx, 1200.0, &[Key::ArrowDown]);
                    assert_eq!(focused_keys(&ctx).as_deref(), Some(expected));
                }
            }
            assert_eq!(focused_keys(&ctx).as_deref(), Some("A"));
            assert!(active(&ctx));
            assert!(app.file.is_none());
            assert!(!app.doc.dirty());
            app_keys(&mut app, &ctx, 1200.0, &[Key::ArrowRight, Key::ArrowLeft]);
            assert_eq!(focused_keys(&ctx).as_deref(), Some("A"));
            assert!(popup_open(&ctx));
        }
    }

    #[test]
    fn rapid_file_arrows_finish_using_only_accepted_repaint_callbacks() {
        for (batched, restarted) in [(false, false), (true, false), (true, true)] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.doc = Document::new(420, 560);
            app.recent = vec![PathBuf::from("artworks/mona-lisa.p10")];
            let callbacks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let recorded = callbacks.clone();
            ctx.set_request_repaint_callback(move |info| {
                recorded.lock().unwrap().push(info);
            });
            let mut trace = Vec::new();
            let mut focus_trace = Vec::new();
            let mut run_to_idle = |app: &mut PaintApp, events| {
                app_frame(app, &ctx, 1180.0, events);
                for _ in 0..20 {
                    let focused = ctx.memory(|memory| memory.focused());
                    if let Some(target) = read(&ctx)
                        .targets
                        .iter()
                        .find(|target| Some(target.id) == focused)
                    {
                        if focus_trace.last() != Some(&target.keys) {
                            focus_trace.push(target.keys.clone());
                        }
                    }
                    let pass = ctx.cumulative_pass_nr();
                    let requests = std::mem::take(&mut *callbacks.lock().unwrap());
                    let scheduled = requests.iter().any(|request| {
                        request.delay == std::time::Duration::ZERO
                            && (pass == request.current_cumulative_pass_nr
                                || pass == request.current_cumulative_pass_nr + 1)
                    });
                    trace.push((pass, requests));
                    if !scheduled {
                        return;
                    }
                    app_frame(app, &ctx, 1180.0, vec![]);
                }
                panic!("Menu must settle instead of continuously repainting; focus={:?}; causes={:?}; trace={trace:?}",
                    ctx.memory(|memory| memory.focused()), ctx.repaint_causes());
            };
            run_to_idle(&mut app, vec![]);
            let keystroke = |code| {
                let down = key(code);
                let mut up = down.clone();
                if let Event::Key { pressed, .. } = &mut up {
                    *pressed = false;
                }
                vec![down, up]
            };
            for code in [Key::F10, Key::F] {
                run_to_idle(&mut app, keystroke(code));
            }
            let arrows = [
                Key::ArrowDown,
                Key::ArrowDown,
                Key::ArrowDown,
                Key::ArrowRight,
            ];
            if restarted {
                let events = [Key::Escape, Key::Escape, Key::F10, Key::F]
                    .into_iter()
                    .chain(arrows)
                    .flat_map(keystroke)
                    .collect();
                run_to_idle(&mut app, events);
            } else if batched {
                run_to_idle(&mut app, arrows.into_iter().flat_map(keystroke).collect());
            } else {
                for code in arrows {
                    run_to_idle(&mut app, keystroke(code));
                }
            }
            let state = read(&ctx);
            let focused = ctx.memory(|memory| memory.focused());
            let target = state
                .targets
                .iter()
                .find(|target| Some(target.id) == focused);
            assert!(
                target.is_some_and(|target| target.left_parent.is_some()),
                "Three Down steps then Right must reach a save format; batched={batched}; focused={:?}; tail={:?}; {trace:?}",
                target.map(|target| &target.keys),
                state.navigation_tail,
            );
            assert!(state.navigation_tail.is_empty());
            assert_eq!(
                target.map(|target| target.keys.as_str()),
                Some("G"),
                "Right from Save as should enter the first format"
            );
            assert!(focus_trace.windows(3).any(|keys| keys == ["O", "S", "A"]),
                "File navigation must preserve Open, Save, Save as in order; restarted={restarted}; {focus_trace:?}");
            assert!(!app.doc.dirty());
        }
    }

    #[test]
    fn mouse_opened_file_menu_repaints_without_further_pointer_input() {
        for split_click in [false, true] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            let callbacks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let recorded = callbacks.clone();
            ctx.set_request_repaint_callback(move |info| {
                recorded.lock().unwrap().push(info);
            });
            app_warm(&mut app, &ctx, 1200.0);
            callbacks.lock().unwrap().clear();
            let file = read(&ctx)
                .targets
                .iter()
                .find(|target| target.scope == "tabs" && target.keys == "F")
                .unwrap()
                .rect
                .center();
            let button = |pressed| Event::PointerButton {
                pos: file,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            let mut events = vec![Event::PointerMoved(file), button(true)];
            let mut output = if split_click {
                app_frame(&mut app, &ctx, 1200.0, events);
                app_frame(&mut app, &ctx, 1200.0, vec![button(false)])
            } else {
                events.push(button(false));
                app_frame(&mut app, &ctx, 1200.0, events)
            };
            let mut trace = Vec::new();
            for _ in 0..12 {
                let pass = ctx.cumulative_pass_nr();
                let requests = std::mem::take(&mut *callbacks.lock().unwrap());
                // This is eframe0.31's native event-loop acceptance rule.
                let scheduled = requests.iter().any(|request| {
                    request.delay == std::time::Duration::ZERO
                        && (pass == request.current_cumulative_pass_nr
                            || pass == request.current_cumulative_pass_nr + 1)
                });
                trace.push((pass, requests));
                if visible(&output, "New") || !scheduled {
                    break;
                }
                output = app_frame(&mut app, &ctx, 1200.0, vec![]);
            }
            assert!(visible(&output, "New"),
                "File must become visible without another mouse event; split={split_click}; {trace:?}");
        }
    }

    #[test]
    fn actual_compressed_ribbon_keytips_reveal_colors_and_nested_rotate_menu() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(40, 20);
        app.refresh = true;
        app_warm(&mut app, &ctx, 500.0);
        app_keys(&mut app, &ctx, 500.0, &[Key::F10, Key::H, Key::Z, Key::K]);
        app_warm(&mut app, &ctx, 500.0);
        let colors = app_frame(&mut app, &ctx, 500.0, vec![]);
        assert!(visible(&colors, "AD"));
        app_keys(&mut app, &ctx, 500.0, &[Key::A, Key::D]);
        assert_eq!(app.colors[0], [237, 28, 36, 255]);
        assert!(!active(&ctx));
        app_keys(&mut app, &ctx, 500.0, &[Key::F10, Key::H, Key::Z, Key::I]);
        app_warm(&mut app, &ctx, 500.0);
        app_keys(&mut app, &ctx, 500.0, &[Key::R, Key::O]);
        app_warm(&mut app, &ctx, 500.0);
        let rotate = app_frame(&mut app, &ctx, 500.0, vec![]);
        assert!(visible(&rotate, "Rotate right 90°"));
        app_keys(&mut app, &ctx, 500.0, &[Key::Num1]);
        assert_eq!(app.doc.image.dimensions(), (20, 40));
        assert!(!active(&ctx));
    }

    #[test]
    fn actual_home_keytip_badges_fit_the_ribbon_without_overlapping() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_warm(&mut app, &ctx, 1200.0);
        app_keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::H]);
        let output = app_frame(&mut app, &ctx, 1200.0, vec![]);
        let badges: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Rect(rect)
                    if rect.fill == Color32::from_rgb(255, 255, 228) =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .collect();
        assert!(badges.len() > 25, "Only {} badges rendered", badges.len());
        for (index, badge) in badges.iter().enumerate() {
            assert!(
                badge.bottom() <= app.canvas_rect.top(),
                "Badge escaped the ribbon: {badge:?}"
            );
            assert!(ctx.screen_rect().contains_rect(*badge));
            for other in &badges[index + 1..] {
                assert!(
                    !badge.intersects(*other),
                    "Overlapping badges: {badge:?} and {other:?}"
                );
            }
        }
    }

    #[test]
    fn keytip_sequences_batched_in_one_frame_open_and_activate_actual_menus() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.doc = Document::new(40, 20);
        app_warm(&mut app, &ctx, 1200.0);
        app_frame(
            &mut app,
            &ctx,
            1200.0,
            [Key::F10, Key::H, Key::R, Key::O, Key::Num1]
                .into_iter()
                .map(key)
                .collect(),
        );
        for _ in 0..16 {
            if !ctx.has_requested_repaint() {
                break;
            }
            app_frame(&mut app, &ctx, 1200.0, vec![]);
        }
        assert_eq!(app.doc.image.dimensions(), (20, 40));
        assert!(!active(&ctx));
    }

    #[test]
    fn resizing_an_open_group_restores_its_expanded_controls_and_keyboard_focus() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_warm(&mut app, &ctx, 500.0);
        app_keys(&mut app, &ctx, 500.0, &[Key::F10, Key::H, Key::Z, Key::K]);
        app_warm(&mut app, &ctx, 500.0);
        app_warm(&mut app, &ctx, 1200.0);
        let state = read(&ctx);
        assert_eq!(state.levels.last().unwrap().scope, "home");
        let focus = ctx.memory(|memory| memory.focused());
        assert!(state.targets.iter().any(|target| Some(target.id) == focus
            && target.scope == "home"
            && target.group == "Colors"));
        app_keys(&mut app, &ctx, 1200.0, &[Key::A, Key::D]);
        assert_eq!(app.colors[0], [237, 28, 36, 255]);
        assert!(!active(&ctx));
    }

    #[test]
    fn mouse_opened_menus_do_not_reappear_after_ribbon_resizing() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app_warm(&mut app, &ctx, 1200.0);
        let opener = read(&ctx)
            .targets
            .into_iter()
            .find(|target| target.scope == "home" && target.kind == Kind::Menu { scope: "rotate" })
            .unwrap();
        let pos = opener.rect.center();
        for pressed in [true, false] {
            app_frame(
                &mut app,
                &ctx,
                1200.0,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        app_warm(&mut app, &ctx, 1200.0);
        assert!(popup_open(&ctx));
        assert!(!active(&ctx));
        app_warm(&mut app, &ctx, 500.0);
        assert!(!popup_open(&ctx));
        app_warm(&mut app, &ctx, 1200.0);
        let output = app_frame(&mut app, &ctx, 1200.0, vec![]);
        assert!(!visible(&output, "Rotate right 90°"));
        assert!(!popup_open(&ctx));
    }

    #[test]
    fn escape_closes_a_temporary_minimized_ribbon_and_restores_canvas_focus() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.collapsed = true;
        app_warm(&mut app, &ctx, 500.0);
        app_keys(&mut app, &ctx, 500.0, &[Key::F10, Key::H]);
        app_warm(&mut app, &ctx, 500.0);
        assert!(ctx
            .data(|data| data.get_temp::<bool>(Id::new("paint10-ribbon-revealed")))
            .unwrap_or(false));
        app_keys(&mut app, &ctx, 500.0, &[Key::Escape]);
        assert!(!ctx
            .data(|data| data.get_temp::<bool>(Id::new("paint10-ribbon-revealed")))
            .unwrap_or(false));
        app_keys(&mut app, &ctx, 500.0, &[Key::Escape]);
        assert!(app.collapsed);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("canvas"))
        );
        assert!(!active(&ctx));
    }

    #[test]
    fn f6_and_editable_font_keytips_preserve_the_live_text_editor() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.text_tab = true;
        app.text_edit = Some(TextEditState {
            index: None,
            origin: (10, 10),
            text: "Keep editing".into(),
            format: crate::text::TextFormat::default(),
            focus: true,
            selection: 0..4,
            insertion_style: None,
            history: text_editing::TextHistory::default(),
            palette_colors: app.colors,
        });
        app_warm(&mut app, &ctx, 1200.0);
        app_keys(&mut app, &ctx, 1200.0, &[Key::F6]);
        assert!(active(&ctx));
        assert_ne!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );
        app_keys(&mut app, &ctx, 1200.0, &[Key::F6]);
        assert!(!active(&ctx));
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );

        app_keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::T, Key::F, Key::S]);
        app_frame(&mut app, &ctx, 1200.0, vec![Event::Text("4".into())]);
        app_frame(&mut app, &ctx, 1200.0, vec![Event::Text("6".into())]);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Keep editing");
        app_keys(&mut app, &ctx, 1200.0, &[Key::Enter]);
        assert_eq!(
            crate::text::pixels_to_points(app.text_edit.as_ref().unwrap().format.style_at(0).size),
            46.0
        );
        assert!(!active(&ctx));
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );

        app_keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::T, Key::F, Key::S]);
        app_frame(&mut app, &ctx, 1200.0, vec![Event::Text("72".into())]);
        app_keys(&mut app, &ctx, 1200.0, &[Key::Escape]);
        assert_eq!(
            crate::text::pixels_to_points(app.text_edit.as_ref().unwrap().format.style_at(0).size),
            46.0,
            "Escape should discard an unsubmitted numeric draft"
        );
        app_keys(&mut app, &ctx, 1200.0, &[Key::Escape]);

        app_keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::T, Key::F, Key::F]);
        app_frame(
            &mut app,
            &ctx,
            1200.0,
            vec![Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::ALT,
            }],
        );
        app_warm(&mut app, &ctx, 1200.0);
        assert_eq!(read(&ctx).levels.last().unwrap().scope, "fonts");
        let list = app_frame(&mut app, &ctx, 1200.0, vec![]);
        assert!(visible(&list, "Sans serif"));
        app_keys(
            &mut app,
            &ctx,
            1200.0,
            &[Key::Escape, Key::Escape, Key::Escape],
        );
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Keep editing");
        assert!(!active(&ctx));
    }

    #[test]
    fn escape_then_control_enter_commits_text_without_reopening_font_menu() {
        for batched in [false, true] {
            let ctx = Context::default();
            let mut app = PaintApp::new_with_context(&ctx, false);
            app.text_tab = true;
            app.text_edit = Some(TextEditState {
                index: None,
                origin: (10, 10),
                text: "Keep this caption".into(),
                format: crate::text::TextFormat::default(),
                focus: true,
                selection: 0..4,
                insertion_style: None,
                history: text_editing::TextHistory::default(),
                palette_colors: app.colors,
            });
            app_warm(&mut app, &ctx, 1200.0);
            let position = read(&ctx)
                .targets
                .iter()
                .find(|target| target.kind == Kind::Menu { scope: "fonts" })
                .unwrap()
                .rect
                .center();
            app_frame(
                &mut app,
                &ctx,
                1200.0,
                vec![
                    Event::PointerMoved(position),
                    Event::PointerButton {
                        pos: position,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Modifiers::NONE,
                    },
                    Event::PointerButton {
                        pos: position,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
            app_warm(&mut app, &ctx, 1200.0);
            assert!(popup_open(&ctx));
            let mut commit = key(Key::Enter);
            if let Event::Key { modifiers, .. } = &mut commit {
                *modifiers = Modifiers::CTRL;
            }
            if batched {
                app_frame(&mut app, &ctx, 1200.0, vec![key(Key::Escape), commit]);
            } else {
                app_keys(&mut app, &ctx, 1200.0, &[Key::Escape]);
                app_frame(&mut app, &ctx, 1200.0, vec![commit]);
            }
            app_warm(&mut app, &ctx, 1200.0);
            assert!(
                app.text_edit.is_none(),
                "Text commit was lost; batched={batched}"
            );
            assert!(!popup_open(&ctx), "Font menu reopened; batched={batched}");
            let selected = app.object.expect("Committed caption should stay selected");
            let ObjectKind::Text { text, .. } = &app.doc.objects[selected].kind else {
                panic!("Caption must remain text");
            };
            assert_eq!(text, "Keep this caption", "batched={batched}");
        }
    }

    #[test]
    fn font_size_accepts_a_click_and_typing_batched_in_one_frame() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.text_tab = true;
        app.text_edit = Some(TextEditState {
            index: None,
            origin: (10, 10),
            text: "Keep editing".into(),
            format: crate::text::TextFormat::default(),
            focus: true,
            selection: 0..4,
            insertion_style: None,
            history: text_editing::TextHistory::default(),
            palette_colors: app.colors,
        });
        app_warm(&mut app, &ctx, 1200.0);
        let input = read(&ctx)
            .targets
            .into_iter()
            .find(|target| target.keys == "FS")
            .unwrap();
        let pos = input.rect.center();
        app_frame(
            &mut app,
            &ctx,
            1200.0,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
                Event::Key {
                    key: Key::A,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers {
                        ctrl: true,
                        command: true,
                        ..Default::default()
                    },
                },
                Event::Text("54".into()),
                key(Key::Enter),
            ],
        );
        app_warm(&mut app, &ctx, 1200.0);
        let state = app.text_edit.as_ref().unwrap();
        assert_eq!(state.text, "Keep editing");
        assert_eq!(
            crate::text::pixels_to_points(state.format.style_at(0).size),
            54.0
        );
        assert_eq!(
            crate::text::pixels_to_points(state.format.style_at(5).size),
            18.0
        );
    }

    #[test]
    fn actual_text_font_keytip_focuses_the_field_and_restores_text_after_escape() {
        let ctx = Context::default();
        let mut app = PaintApp::new_with_context(&ctx, false);
        app.text_tab = true;
        app.text_edit = Some(TextEditState {
            index: None,
            origin: (10, 10),
            text: "Keep editing".into(),
            format: crate::text::TextFormat::default(),
            focus: true,
            selection: 0..4,
            insertion_style: None,
            history: text_editing::TextHistory::default(),
            palette_colors: app.colors,
        });
        app_warm(&mut app, &ctx, 1200.0);
        app_keys(&mut app, &ctx, 1200.0, &[Key::F10, Key::T, Key::F, Key::F]);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("paint10_font_name"))
        );
        app_frame(&mut app, &ctx, 1200.0, vec![Event::Text("sans".into())]);
        assert_eq!(app.text_edit.as_ref().unwrap().text, "Keep editing");
        app_keys(
            &mut app,
            &ctx,
            1200.0,
            &[Key::Enter, Key::F10, Key::T, Key::F, Key::L],
        );
        app_warm(&mut app, &ctx, 1200.0);
        let list = app_frame(&mut app, &ctx, 1200.0, vec![]);
        assert!(visible(&list, "Sans serif"));
        let scope = read(&ctx).levels.last().unwrap().scope.clone();
        assert_eq!(scope, "fonts");
        app_keys(
            &mut app,
            &ctx,
            1200.0,
            &[Key::Escape, Key::Escape, Key::Escape],
        );
        assert!(!active(&ctx));
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(Id::new("text_input"))
        );
        app_frame(&mut app, &ctx, 1200.0, vec![Event::Text("Next".into())]);
        assert!(app.text_edit.as_ref().unwrap().text.contains("Next"));
    }
}
