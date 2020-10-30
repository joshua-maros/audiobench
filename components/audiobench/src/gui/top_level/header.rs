use crate::gui::constants::*;
use crate::scui_config::Renderer;
use scui::{MaybeMouseBehavior, MouseBehavior, MouseMods, OnClickBehavior, Vec2D, WidgetImpl};
use shared_util::prelude::*;

scui::widget! {
    pub Header
}

const TAB_SIZE: Vec2D = Vec2D::new(grid(4), grid(1));
const TAB_PADDING: f32 = GRID_P * 0.5;

impl Header {
    pub fn new(parent: &impl HeaderParent) -> Rc<Self> {
        let state = HeaderState { pos: Vec2D::zero() };
        let this = Rc::new(Self::create(parent, state));
        this
    }
}

impl WidgetImpl<Renderer> for Header {
    fn get_size(self: &Rc<Self>) -> Vec2D {
        (ROOT_WIDTH, HEADER_HEIGHT).into()
    }

    fn get_mouse_behavior(self: &Rc<Self>, pos: Vec2D, _mods: &MouseMods) -> MaybeMouseBehavior {
        let tab_index = (pos.x / (TAB_SIZE.x + TAB_PADDING)) as usize;
        let this = Rc::clone(self);
        OnClickBehavior::wrap(move || {
            this.with_gui_state_mut(|state| {
                state.focus_tab_by_index(tab_index);
            });
        })
    }

    fn draw(self: &Rc<Self>, renderer: &mut Renderer) {
        const BFS: f32 = BIG_FONT_SIZE;
        const CS: f32 = CORNER_SIZE;
        const GP: f32 = GRID_P;
        const FS: f32 = FONT_SIZE;

        renderer.set_color(&COLOR_BG2);
        renderer.draw_rect(0, (ROOT_WIDTH, HEADER_HEIGHT - grid(1)));
        renderer.set_color(&COLOR_BG0);
        let tab_bar_start = Vec2D::new(0.0, HEADER_HEIGHT - grid(1));
        renderer.draw_rect(tab_bar_start, (ROOT_WIDTH, grid(1)));

        let tooltip_size: Vec2D = (ROOT_WIDTH - GP * 2.0, TOOLTIP_HEIGHT).into();
        renderer.set_color(&COLOR_BG0);
        renderer.draw_rounded_rect(GP, tooltip_size, CS);
        let textbox_size = tooltip_size - GP * 2.0;
        self.with_gui_state(|state| {
            let tooltip = &state.tooltip;
            renderer.set_color(&COLOR_FG1);
            renderer.draw_text(BFS, GP * 2.0, textbox_size, (-1, -1), 1, &tooltip.text);

            let mut pos = tab_bar_start;
            let mut index = 0;
            let active_index = state.get_current_tab_index();
            for tab in state.all_tabs() {
                if index == active_index {
                    renderer.set_color(&COLOR_BG2);
                } else {
                    renderer.set_color(&COLOR_BG1);
                }
                renderer.draw_rect(pos, TAB_SIZE);
                renderer.set_color(&COLOR_FG1);
                renderer.draw_text(FS, pos, TAB_SIZE, (0, 0), 1, &tab.get_name());
                pos.x += TAB_SIZE.x + TAB_PADDING;
                index += 1;
            }
        });
    }
}
