use crate::engine::parts as ep;
use crate::gui::action::{DropTarget, MouseAction};
use crate::gui::audio_widgets::Module;
use crate::gui::constants::*;
use crate::gui::graphics::{GrahpicsWrapper, HAlign, VAlign};
use crate::gui::{Gui, MouseMods};
use crate::util::*;
use std::f32::consts::PI;

pub enum FeedbackDataRequirement {
    None,
    Control { control_index: usize },
    Custom { code_name: String, size: usize },
}

impl FeedbackDataRequirement {
    pub fn size(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Control { .. } => 1,
            Self::Custom { size, .. } => *size,
        }
    }
}

#[derive(Debug)]
pub enum WidgetOutline {
    Knob {
        control_index: usize,
        grid_pos: (i32, i32),
        label: String,
    },
    EnvelopeGraph {
        grid_pos: (i32, i32),
        grid_size: (i32, i32),
        feedback_name: String,
    },
}

impl WidgetOutline {
    pub fn get_feedback_data_requirement(&self) -> FeedbackDataRequirement {
        match self {
            Self::Knob { control_index, .. } => FeedbackDataRequirement::Control {
                control_index: *control_index,
            },
            Self::EnvelopeGraph { feedback_name, .. } => FeedbackDataRequirement::Custom {
                code_name: feedback_name.clone(),
                size: 6,
            },
        }
    }
}

pub fn widget_from_outline(
    controls: &Vec<Rcrc<ep::Control>>,
    outline: &WidgetOutline,
    // usize is the amount of feedback data the widget uses.
) -> (Box<dyn ModuleWidget>, usize) {
    fn convert_grid_pos(grid_pos: (i32, i32)) -> (i32, i32) {
        (
            MODULE_IO_WIDTH + JACK_SIZE + coord(grid_pos.0),
            coord(grid_pos.1),
        )
    }
    fn convert_grid_size(grid_size: (i32, i32)) -> (i32, i32) {
        (grid(grid_size.0), grid(grid_size.1))
    }

    let widget: Box<dyn ModuleWidget> = match outline {
        WidgetOutline::Knob {
            control_index,
            grid_pos,
            label,
        } => Box::new(Knob::create(
            Rc::clone(&controls[*control_index]),
            convert_grid_pos(*grid_pos),
            label.clone(),
        )),
        WidgetOutline::EnvelopeGraph {
            grid_pos,
            grid_size,
            ..
        } => Box::new(EnvelopeGraph::create(
            convert_grid_pos(*grid_pos),
            convert_grid_size(*grid_size),
        )),
    };
    let feedback_data_len = outline.get_feedback_data_requirement().size();
    (widget, feedback_data_len)
}

pub trait ModuleWidget {
    fn respond_to_mouse_press(
        &self,
        mouse_pos: (i32, i32),
        mods: &MouseMods,
        parent_pos: (i32, i32),
    ) -> MouseAction;
    fn get_drop_target_at(&self, mouse_pos: (i32, i32)) -> DropTarget;
    fn draw(
        &self,
        g: &mut GrahpicsWrapper,
        highlight: bool,
        parent_pos: (i32, i32),
        feedback_data: &[f32],
    );
}

#[derive(Clone)]
pub struct Knob {
    control: Rcrc<ep::Control>,
    pos: (i32, i32),
    label: String,
}

impl Knob {
    fn create(control: Rcrc<ep::Control>, pos: (i32, i32), label: String) -> Knob {
        Knob {
            control,
            pos,
            label,
        }
    }
}

impl ModuleWidget for Knob {
    fn respond_to_mouse_press(
        &self,
        mouse_pos: (i32, i32),
        mods: &MouseMods,
        parent_pos: (i32, i32),
    ) -> MouseAction {
        let mouse_pos = (mouse_pos.0 - self.pos.0, mouse_pos.1 - self.pos.1);
        if mouse_pos.inside((grid(2), grid(1))) {
            if mods.right_click {
                let pos = (
                    self.pos.0 + parent_pos.0 + grid(2) / 2,
                    self.pos.1 + parent_pos.1 + grid(2) / 2,
                );
                MouseAction::OpenMenu(Box::new(KnobEditor::create(
                    Rc::clone(&self.control),
                    pos,
                    self.label.clone(),
                )))
            } else {
                MouseAction::ManipulateControl(Rc::clone(&self.control))
            }
        } else {
            MouseAction::None
        }
    }

    fn get_drop_target_at(&self, mouse_pos: (i32, i32)) -> DropTarget {
        let mouse_pos = (mouse_pos.0 - self.pos.0, mouse_pos.1 - self.pos.1);
        if mouse_pos.inside((grid(2), grid(1))) {
            DropTarget::Control(Rc::clone(&self.control))
        } else {
            DropTarget::None
        }
    }

    fn draw(
        &self,
        g: &mut GrahpicsWrapper,
        highlight: bool,
        parent_pos: (i32, i32),
        feedback_data: &[f32],
    ) {
        g.push_state();

        let control = &*self.control.borrow();
        fn value_to_angle(range: (f32, f32), value: f32) -> f32 {
            value.from_range_to_range(range.0, range.1, PI, 0.0)
        }

        g.set_color(&COLOR_TEXT);
        let (cx, cy) = (self.pos.0 + grid(2) / 2, self.pos.1 + grid(2) / 2);
        for lane in self.control.borrow().automation.iter() {
            let (module, output_index) = &lane.connection;
            let output_index = *output_index as i32;
            let module_ref = module.borrow();
            let (ox, oy) = Module::output_position(&*module_ref, output_index);
            let (ox, oy) = (ox - parent_pos.0, oy - parent_pos.1);
            g.stroke_line(cx, cy, ox, oy, 2.0);
        }

        // Applying the offset later makes connections easier to render.
        g.apply_offset(self.pos.0, self.pos.1);

        if highlight {
            g.set_color(&COLOR_TEXT);
        } else {
            g.set_color(&COLOR_BG);
        }
        g.fill_pie(0, 0, grid(2), KNOB_INSIDE_SPACE * 2, 0.0, PI);
        g.set_color(&COLOR_KNOB);
        if highlight {
            g.set_alpha(0.5);
        }
        let zero_angle = value_to_angle(control.range, 0.0);
        // If manual, show the manual value. If automated, show the most recent value recorded
        // from when a note was actually playing.
        let value = if control.automation.len() > 0 {
            feedback_data[0]
        } else {
            control.value
        };
        let value_angle = value_to_angle(control.range, value);
        g.fill_pie(
            0,
            0,
            grid(2),
            KNOB_INSIDE_SPACE * 2,
            zero_angle,
            value_angle,
        );
        g.set_alpha(1.0);
        g.set_color(&COLOR_TEXT);
        g.write_label(0, grid(1) + GRID_P, grid(2), &self.label);

        if control.automation.len() > 0 {
            let num_lanes = control.automation.len() as i32;
            let lane_size = KNOB_AUTOMATION_SPACE / num_lanes;
            let lane_size = lane_size.min(KNOB_MAX_LANE_SIZE).max(2);
            for (index, lane) in control.automation.iter().enumerate() {
                g.set_color(&COLOR_AUTOMATION);
                let index = index as i32;
                let outer_diameter = grid(2) - (KNOB_OUTSIDE_SPACE * 2) - lane_size * index * 2;
                let inner_diameter = outer_diameter - (lane_size - KNOB_LANE_GAP) * 2;
                let inset = (grid(2) - outer_diameter) / 2;
                let min_angle = value_to_angle(control.range, lane.range.0);
                let max_angle = value_to_angle(control.range, lane.range.1);
                g.fill_pie(
                    inset,
                    inset,
                    outer_diameter,
                    inner_diameter,
                    min_angle,
                    max_angle,
                );
            }
        }

        g.pop_state();
    }
}

#[derive(Clone)]
pub struct EnvelopeGraph {
    pos: (i32, i32),
    size: (i32, i32),
}

impl EnvelopeGraph {
    fn create(pos: (i32, i32), size: (i32, i32)) -> Self {
        Self { pos, size }
    }
}

impl ModuleWidget for EnvelopeGraph {
    fn respond_to_mouse_press(
        &self,
        mouse_pos: (i32, i32),
        mods: &MouseMods,
        parent_pos: (i32, i32),
    ) -> MouseAction {
        MouseAction::None
    }

    fn get_drop_target_at(&self, mouse_pos: (i32, i32)) -> DropTarget {
        DropTarget::None
    }

    fn draw(
        &self,
        g: &mut GrahpicsWrapper,
        highlight: bool,
        parent_pos: (i32, i32),
        feedback_data: &[f32],
    ) {
        g.push_state();

        const CS: i32 = CORNER_SIZE;
        g.apply_offset(self.pos.0, self.pos.1);
        g.set_color(&COLOR_BG);
        g.fill_rounded_rect(0, 0, self.size.0, self.size.1, CS);
        g.apply_offset(0, CS);

        g.set_color(&COLOR_TEXT);
        let (a, d, s, r) = (
            feedback_data[0],
            feedback_data[1],
            feedback_data[2],
            feedback_data[3],
        );
        let total_duration = (a + d + r).max(0.2); // to prevent div0
        let w = self.size.0;
        let h = self.size.1 - CS * 2;
        let decay_x = (w as f32 * (a / total_duration)) as i32;
        let sustain_y = ((1.0 - s) * h as f32) as i32;
        let release_x = (w as f32 * ((a + d) / total_duration)) as i32;
        let silence_x = (w as f32 * ((a + d + r) / total_duration)) as i32;
        g.stroke_line(0, h, decay_x, 0, 2.0);
        g.stroke_line(decay_x, 0, release_x, sustain_y, 2.0);
        g.stroke_line(release_x, sustain_y, silence_x, h, 2.0);

        g.set_alpha(0.5);
        g.stroke_line(decay_x, -CS, decay_x, h + CS, 1.0);
        g.stroke_line(release_x, -CS, release_x, h + CS, 1.0);
        let (cx, cy) = (feedback_data[4], feedback_data[5]);
        let cx = (cx / total_duration * w as f32) as i32;
        let cy = ((-cy * 0.5 + 0.5) * h as f32) as i32;
        g.stroke_line(cx, 0, cx, h, 1.0);
        g.stroke_line(0, cy, w, cy, 1.0);
        g.set_alpha(1.0);
        const DOT_SIZE: i32 = 8;
        const DR: i32 = DOT_SIZE / 2;
        g.fill_pie(cx - DR, cy - DR, DR * 2, 0, 0.0, PI * 2.0);

        let ms = (total_duration * 1000.0) as i32;
        let ms_text = if ms > 999 {
            format!("{},{:03}ms", ms / 1000, ms % 1000)
        } else {
            format!("{}ms", ms)
        };
        g.write_text(12, 0, 0, w, h, HAlign::Right, VAlign::Top, 1, &ms_text);

        g.pop_state();
    }
}

#[derive(Clone)]
pub struct KnobEditor {
    control: Rcrc<ep::Control>,
    pos: (i32, i32),
    size: (i32, i32),
    label: String,
}

impl KnobEditor {
    fn create(control: Rcrc<ep::Control>, center_pos: (i32, i32), label: String) -> Self {
        let num_channels = control.borrow().automation.len().max(2) as i32;
        let required_radius =
            (KNOB_MENU_LANE_SIZE + KNOB_MENU_LANE_GAP) * num_channels + KNOB_MENU_KNOB_OR + GRID_P;
        let size = (required_radius * 2, required_radius + fatgrid(1));
        Self {
            control,
            pos: (center_pos.0 - size.0 / 2, center_pos.1 - size.1 / 2),
            size,
            label,
        }
    }

    pub(in crate::gui) fn respond_to_mouse_press(
        &self,
        mouse_pos: (i32, i32),
        mods: &MouseMods,
    ) -> Option<MouseAction> {
        let mouse_pos = (mouse_pos.0 - self.pos.0, mouse_pos.1 - self.pos.1);
        if mouse_pos.inside(self.size) {
            // Yes, the last 0 is intentional. The center of the knob is not vertically centered.
            let (cx, cy) = (mouse_pos.0 - self.size.0 / 2, mouse_pos.1 - self.size.0 / 2);
            // y coordinate is inverted from how it appears on screen.
            let (fcx, fcy) = (cx as f32, -cy as f32);
            let (angle, radius) = (fcy.atan2(fcx), (fcy * fcy + fcx * fcx).sqrt());
            let control = &*self.control.borrow();
            let auto_lanes = control.automation.len();
            // Clicked somewhere in the top "half" where the main knob and automation lanes are.
            if angle >= 0.0 && angle <= PI {
                let radius = radius as i32;
                if radius < KNOB_MENU_KNOB_IR {
                    // Nothing interacjackle inside the knob.
                } else if radius < KNOB_MENU_KNOB_OR {
                    return Some(MouseAction::ManipulateControl(Rc::clone(&self.control)));
                } else {
                    let radius = radius - KNOB_MENU_KNOB_OR;
                    let lane = (radius / (KNOB_MENU_LANE_SIZE + KNOB_MENU_LANE_GAP)) as usize;
                    if lane < auto_lanes {
                        // It's rendered backwards so we need to flip the index to make it visually
                        // match up.
                        let lane = auto_lanes - lane - 1;
                        let range = control.range;
                        let lane_range = control.automation[lane].range;
                        let min_angle = lane_range.0.from_range_to_range(range.0, range.1, PI, 0.0);
                        let max_angle = lane_range.1.from_range_to_range(range.0, range.1, PI, 0.0);
                        // TODO: Handle inverted lanes.
                        return Some(if angle > min_angle {
                            MouseAction::ManipulateLaneStart(Rc::clone(&self.control), lane)
                        } else if angle < max_angle {
                            MouseAction::ManipulateLaneEnd(Rc::clone(&self.control), lane)
                        } else {
                            MouseAction::ManipulateLane(Rc::clone(&self.control), lane)
                        });
                    }
                }
            }
            Some(MouseAction::None)
        } else {
            None
        }
    }

    pub(in crate::gui) fn draw(&self, g: &mut GrahpicsWrapper) {
        g.push_state();

        g.apply_offset(self.pos.0, self.pos.1);
        g.set_color(&COLOR_SURFACE);
        g.fill_rounded_rect(0, 0, self.size.0, self.size.1, CORNER_SIZE);

        let control = &*self.control.borrow();
        fn value_to_angle(range: (f32, f32), value: f32) -> f32 {
            value.from_range_to_range(range.0, range.1, PI, 0.0)
        }
        g.apply_offset(self.size.0 / 2, self.size.1 - fatgrid(1));

        const KOR: i32 = KNOB_MENU_KNOB_OR;
        const KIR: i32 = KNOB_MENU_KNOB_IR;
        g.set_color(&COLOR_BG);
        g.fill_pie(-KOR, -KOR, KOR * 2, KIR * 2, PI, 0.0);
        g.set_color(&COLOR_KNOB);
        let zero_angle = value_to_angle(control.range, 0.0);
        let value_angle = value_to_angle(control.range, control.value);
        g.fill_pie(-KOR, -KOR, KOR * 2, KIR * 2, zero_angle, value_angle);

        const GAP: i32 = KNOB_MENU_LANE_GAP;
        const LS: i32 = KNOB_MENU_LANE_SIZE;
        // TODO: Handle inverted lanes.
        for (index, lane) in control.automation.iter().rev().enumerate() {
            let ir = KOR + GAP + (GAP + LS) * index as i32;
            let or = ir + LS;
            g.set_color(&COLOR_BG);
            g.fill_pie(-or, -or, or * 2, ir * 2, PI, 0.0);
            g.set_color(&COLOR_AUTOMATION);
            let min_angle = value_to_angle(control.range, lane.range.0);
            let max_angle = value_to_angle(control.range, lane.range.1);
            g.fill_pie(-or, -or, or * 2, ir * 2, min_angle, max_angle);
        }

        g.set_color(&COLOR_TEXT);
        let value_text = format_decimal(control.value, 3);
        g.write_label(-KIR, -12, KIR * 2, &value_text);
        g.write_label(-KOR, GRID_P, KOR * 2, &self.label);

        g.pop_state();
    }
}