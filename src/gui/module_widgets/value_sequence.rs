use super::ModuleWidget;
use super::{IntBoxBase, IntBoxImpl};
use crate::engine::parts as ep;
use crate::gui::action::MouseAction;
use crate::gui::constants::*;
use crate::gui::graphics::{GrahpicsWrapper, HAlign, VAlign};
use crate::gui::{InteractionHint, MouseMods, Tooltip};
use crate::registry::yaml::YamlNode;
use crate::registry::Registry;
use crate::util::*;

yaml_widget_boilerplate::make_widget_outline! {
    widget_struct: ValueSequence,
    constructor: create(
        pos: GridPos,
        size: GridSize,
        sequence_control: ComplexControlRef,
        ramping_control: ControlRef,
        tooltip: String,
    ),
    // Feedback for playhead and ramping amount.
    feedback: custom(2),
}

#[derive(Clone)]
pub struct ValueSequence {
    tooltip: String,
    sequence_control: Rcrc<ep::ComplexControl>,
    ramping_control: Rcrc<ep::Control>,
    pos: (f32, f32),
    width: f32,
}

impl ValueSequence {
    const HEIGHT: f32 = grid(2);
    const HEADER_SPACE: f32 = CORNER_SIZE * 2.0;
    const VALUE_DECIMALS: usize = 5;
    const VALUE_LENGTH: usize = Self::VALUE_DECIMALS + 4; // 2 for 0., 1 for sign, 1 for comma

    pub fn create(
        pos: (f32, f32),
        size: (f32, f32),
        sequence_control: Rcrc<ep::ComplexControl>,
        ramping_control: Rcrc<ep::Control>,
        tooltip: String,
    ) -> ValueSequence {
        ValueSequence {
            tooltip,
            sequence_control,
            ramping_control,
            pos,
            width: size.0,
        }
    }
}

impl ModuleWidget for ValueSequence {
    fn get_position(&self) -> (f32, f32) {
        self.pos
    }

    fn get_bounds(&self) -> (f32, f32) {
        (self.width, ValueSequence::HEIGHT)
    }

    fn respond_to_mouse_press(
        &self,
        local_pos: (f32, f32),
        mods: &MouseMods,
        parent_pos: (f32, f32),
    ) -> MouseAction {
        let num_steps = parse_sequence_length(&self.sequence_control);
        let step_width = self.width / num_steps as f32;
        let clicked_step = (local_pos.0 / step_width) as usize;
        let value_start = clicked_step * ValueSequence::VALUE_LENGTH + 1;
        let value_end = value_start + ValueSequence::VALUE_LENGTH;
        let float_value: f32 = (&self.sequence_control.borrow().value[value_start..value_end - 1])
            .trim()
            .parse()
            .unwrap();
        MouseAction::ManipulateSequencedValue {
            cref: Rc::clone(&self.sequence_control),
            value_start,
            value_end,
            float_value,
            value_formatter: format_value,
        }
    }

    fn get_tooltip_at(&self, local_pos: (f32, f32)) -> Option<Tooltip> {
        Some(Tooltip {
            text: self.tooltip.clone(),
            interaction: InteractionHint::LeftClickAndDrag.into(),
        })
    }

    fn draw(
        &self,
        g: &mut GrahpicsWrapper,
        highlight: bool,
        parent_pos: (f32, f32),
        feedback_data: &[f32],
    ) {
        assert_eq!(feedback_data.len(), 2);
        g.push_state();
        g.apply_offset(self.pos.0, self.pos.1);

        const H: f32 = ValueSequence::HEIGHT;
        const CS: f32 = CORNER_SIZE;
        const HEAD: f32 = ValueSequence::HEADER_SPACE;
        g.set_color(&COLOR_BG);
        g.fill_rounded_rect(0.0, HEAD, self.width, H - HEAD, CS);

        let num_steps = parse_sequence_length(&self.sequence_control);
        let step_width = self.width / num_steps as f32;
        let borrowed = self.sequence_control.borrow();
        let ramping = feedback_data[1];
        let mut current_value = &borrowed.value[1..];
        const VALUE_LEN: usize = ValueSequence::VALUE_LENGTH;
        const MIDPOINT: f32 = HEAD + (H - HEAD) * 0.5;
        let first_value: f32 = (&current_value[..VALUE_LEN - 1]).trim().parse().unwrap();
        let mut value = first_value;
        for step_index in 0..num_steps {
            let x = step_index as f32 * step_width;
            if step_index != num_steps - 1 {
                g.set_color(&COLOR_TEXT);
                // g.set_alpha(0.5);
                g.stroke_line(x + step_width, HEAD, x + step_width, H, 1.0);
            }
            g.set_color(&COLOR_KNOB);
            let y = (0.5 - value * 0.5) * (H - HEAD) + HEAD;
            g.set_alpha(0.3);
            g.fill_rect(x, MIDPOINT.min(y), step_width, (MIDPOINT - y).abs());
            g.set_alpha(1.0);
            g.stroke_line(x, y, x + step_width * (1.0 - ramping), y, 2.0);
            current_value = &current_value[VALUE_LEN..];
            let next_value = if step_index < num_steps - 1 {
                (&current_value[..VALUE_LEN - 1]).trim().parse().unwrap()
            } else {
                first_value
            };
            value = next_value;
            let next_y = (0.5 - next_value * 0.5) * (H - HEAD) + HEAD;
            g.stroke_line(
                x + step_width * (1.0 - ramping),
                y,
                x + step_width,
                next_y,
                2.0,
            );
        }

        g.set_color(&COLOR_TEXT);
        g.fill_pie(
            feedback_data[0] * step_width - HEAD,
            0.0,
            HEAD * 2.0,
            0.0,
            std::f32::consts::PI * 0.75,
            std::f32::consts::PI * 0.25,
        );

        g.pop_state();
    }
}

fn parse_sequence_length(control: &Rcrc<ep::ComplexControl>) -> usize {
    (control.borrow().value.len() - 2) / ValueSequence::VALUE_LENGTH
}

fn format_value(value: f32) -> String {
    format!("{: >8.*},", ValueSequence::VALUE_DECIMALS, value,)
}

yaml_widget_boilerplate::make_widget_outline! {
    widget_struct: ValueSequenceLength,
    constructor: create(
        registry: RegistryRef,
        pos: GridPos,
        sequence_control: ComplexControlRef,
        label: String,
        tooltip: String,
    ),
    complex_control_default_provider: get_defaults,
}

pub struct ValueSequenceLength {
    base: IntBoxBase,
    sequence_control: Rcrc<ep::ComplexControl>,
}

impl ValueSequenceLength {
    pub fn create(
        registry: &Registry,
        pos: (f32, f32),
        sequence_control: Rcrc<ep::ComplexControl>,
        label: String,
        tooltip: String,
    ) -> Self {
        Self {
            base: IntBoxBase::create(tooltip, registry, pos, (1, 99), label),
            sequence_control,
        }
    }

    fn get_defaults(
        outline: &GeneratedValueSequenceLengthOutline,
        yaml: &YamlNode,
    ) -> Result<Vec<(usize, String)>, String> {
        let p = format_value(1.0);
        let n = format_value(-1.0);
        Ok(vec![(
            outline.sequence_control_index,
            format!("[{}{}{}{}]", p, p, n, n),
        )])
    }
}

impl IntBoxImpl for ValueSequenceLength {
    fn get_base(&self) -> &IntBoxBase {
        &self.base
    }

    fn get_current_value(&self) -> i32 {
        parse_sequence_length(&self.sequence_control) as i32
    }

    fn make_callback(&self) -> Box<dyn Fn(i32)> {
        let sequence_control = Rc::clone(&self.sequence_control);
        Box::new(move |new_length| {
            assert!(new_length >= 1);
            let new_length = new_length as usize;
            let current_length = parse_sequence_length(&sequence_control);
            let mut borrowed = sequence_control.borrow_mut();
            let current_value = &borrowed.value;
            const VALUE_LEN: usize = ValueSequence::VALUE_LENGTH;
            if new_length < current_length {
                let new_value = format!("{}]", &current_value[..1 + VALUE_LEN * new_length]);
                borrowed.value = new_value;
            } else if new_length > current_length {
                let mut new_value = String::from(&current_value[..current_value.len() - 1]);
                for _ in current_length..new_length {
                    new_value.push_str(&format_value(0.0));
                }
                new_value.push_str("]");
                borrowed.value = new_value;
            }
        })
    }
}
