use crate::engine::parts::{ComplexControl, Control, IOJack, JackType, Module, ModuleTemplate};
use crate::engine::yaml::{self, YamlNode};
use crate::gui::module_widgets::WidgetOutline;
use crate::util::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

fn create_control_from_yaml(yaml: &YamlNode) -> Result<Rcrc<Control>, String> {
    let min = yaml.unique_child("min")?.f32()?;
    let max = yaml.unique_child("max")?.f32()?;
    let default = yaml.unique_child("default")?.f32()?;
    let suffix = if let Ok(node) = yaml.unique_child("suffix") {
        node.value.clone()
    } else {
        "".to_owned()
    };
    Ok(rcrc(Control::create(
        yaml.name.clone(),
        min,
        max,
        default,
        suffix,
    )))
}

fn create_widget_outline_from_yaml(
    yaml: &YamlNode,
    controls: &Vec<Rcrc<Control>>,
    complex_controls: &mut Vec<Rcrc<ComplexControl>>,
) -> Result<WidgetOutline, String> {
    let x = yaml.unique_child("x")?.i32()?;
    let y = yaml.unique_child("y")?.i32()?;
    let grid_pos = (x, y);
    let tooltip_node = yaml.unique_child("tooltip");
    let find_control_index = |name: &str| {
        controls
            .iter()
            .position(|item| &item.borrow().code_name == name)
            .ok_or_else(|| {
                format!(
                    "ERROR: Invalid widget {}, caused by:\nERROR: No control named {}.",
                    &yaml.full_name, name
                )
            })
    };
    let find_complex_control_index = |name: &str| {
        complex_controls
            .iter()
            .position(|item| &item.borrow().code_name == name)
            .ok_or_else(|| {
                format!(
                    "ERROR: Invalid widget {}, caused by:\nERROR: No complex control named {}.",
                    &yaml.full_name, name
                )
            })
    };
    let mut set_default = None;
    let outline = match &yaml.name[..] {
        "knob" => {
            let control_name = &yaml.unique_child("control")?.value;
            let control_index = find_control_index(control_name)?;
            let label = yaml.unique_child("label")?.value.clone();
            WidgetOutline::Knob {
                tooltip: tooltip_node?.value.clone(),
                control_index,
                grid_pos,
                label,
            }
        }
        "envelope_graph" => {
            let grid_size = (
                yaml.unique_child("w")?.i32()?,
                yaml.unique_child("h")?.i32()?,
            );
            let feedback_name = yaml.unique_child("feedback_name")?.value.clone();
            WidgetOutline::EnvelopeGraph {
                grid_pos,
                grid_size,
                feedback_name,
            }
        }
        "waveform_graph" => {
            let grid_size = (
                yaml.unique_child("w")?.i32()?,
                yaml.unique_child("h")?.i32()?,
            );
            let feedback_name = yaml.unique_child("feedback_name")?.value.clone();
            WidgetOutline::WaveformGraph {
                grid_pos,
                grid_size,
                feedback_name,
            }
        }
        "int_box" => {
            let ccontrol_name = &yaml.unique_child("control")?.value;
            let ccontrol_index = find_complex_control_index(ccontrol_name)?;
            let min = yaml.unique_child("min")?.i32()?;
            let max = yaml.unique_child("max")?.i32()?;
            let default = if let Ok(child) = yaml.unique_child("default") {
                child.i32()?
            } else {
                min
            };
            let label = yaml.unique_child("label")?.value.clone();
            set_default = Some((ccontrol_index, format!("{}", default)));
            WidgetOutline::IntBox {
                tooltip: tooltip_node?.value.clone(),
                ccontrol_index,
                grid_pos,
                range: (min, max),
                label,
            }
        }
        _ => {
            return Err(format!(
                "ERROR: Invalid widget {}, caused by:\nERROR: {} is not a valid widget type.",
                &yaml.full_name, &yaml.name
            ))
        }
    };
    if let Some((index, value)) = set_default {
        if complex_controls[index].borrow().value != "" {
            return Err(format!(
                "ERROR: Multiple widgets controlling the same complex control {}.",
                complex_controls[index].borrow().code_name
            ));
        }
        complex_controls[index].borrow_mut().default = value.clone();
        complex_controls[index].borrow_mut().value = value;
    }
    Ok(outline)
}

fn create_module_prototype_from_yaml(
    icon_indexes: &HashMap<String, usize>,
    resource_name: String,
    yaml: &YamlNode,
) -> Result<Module, String> {
    let mut controls = Vec::new();
    let mut existing_controls = HashSet::new();
    for control_description in &yaml.unique_child("controls")?.children {
        if existing_controls.contains(&control_description.name) {
            return Err(format!(
                "ERROR: Duplicate entry for {}",
                control_description.full_name
            ));
        }
        existing_controls.insert(control_description.name.clone());
        controls.push(create_control_from_yaml(&control_description)?);
    }

    let mut complex_controls = Vec::new();
    if let Ok(child) = &yaml.unique_child("complex_controls") {
        for description in &child.children {
            // TODO: Error for duplicate control
            complex_controls.push(rcrc(ComplexControl {
                code_name: description.name.clone(),
                value: "".to_owned(),
                default: "".to_owned(),
            }));
        }
    }

    let gui_description = yaml.unique_child("gui")?;
    let widgets_description = gui_description.unique_child("widgets")?;
    let label = gui_description.unique_child("label")?.value.clone();
    let category = gui_description.unique_child("category")?.value.clone();
    let tooltip = gui_description.unique_child("tooltip")?.value.clone();
    let width = gui_description.unique_child("width")?.i32()?;
    let height = gui_description.unique_child("height")?.i32()?;
    let mut widgets = Vec::new();
    for widget_description in &widgets_description.children {
        widgets.push(create_widget_outline_from_yaml(
            widget_description,
            &controls,
            &mut complex_controls,
        )?);
    }

    for control in &complex_controls {
        if control.borrow().value == "" {
            return Err(format!(
                "ERROR: No widget was created for the complex control {}",
                control.borrow().code_name
            ));
        }
    }

    let mut inputs = Vec::new();
    let mut default_inputs = Vec::new();
    for input_description in &yaml.unique_child("inputs")?.children {
        let type_name = &input_description.unique_child("type")?.value;
        let typ = JackType::from_str(type_name)
            .map_err(|_| format!("ERROR: {} is not a valid input type.", type_name))?;
        // The base library should always come with these icons.
        let icon = *icon_indexes.get(typ.icon_name()).unwrap();
        let custom_icon = if let Ok(node) = input_description.unique_child("icon") {
            Some(
                *icon_indexes
                    .get(&node.value)
                    .ok_or_else(|| format!("ERROR: {} is not a valid icon name.", &node.value))?,
            )
        } else {
            None
        };
        let label = input_description.unique_child("label")?.value.clone();
        let tooltip = input_description.unique_child("tooltip")?.value.clone();
        default_inputs.push(
            if let Ok(node) = input_description.unique_child("default") {
                let index = node.i32()? as usize;
                if index >= typ.get_num_defaults() {
                    0
                } else {
                    index
                }
            } else {
                0
            },
        );
        inputs.push(IOJack::create(
            icon_indexes,
            typ,
            icon,
            custom_icon,
            input_description.name.clone(),
            label,
            tooltip,
        ));
    }
    let mut outputs = Vec::new();
    for output_description in &yaml.unique_child("outputs")?.children {
        let type_name = &output_description.unique_child("type")?.value;
        let typ = JackType::from_str(type_name)
            .map_err(|_| format!("ERROR: {} is not a valid output type.", type_name))?;
        // The base library should always come with these icons.
        let icon = *icon_indexes.get(typ.icon_name()).unwrap();
        let custom_icon = if let Ok(node) = output_description.unique_child("icon") {
            Some(
                *icon_indexes
                    .get(&node.value)
                    .ok_or_else(|| format!("ERROR: {} is not a valid icon name.", &node.value))?,
            )
        } else {
            None
        };
        let label = output_description.unique_child("label")?.value.clone();
        let tooltip = output_description.unique_child("tooltip")?.value.clone();
        outputs.push(IOJack::create(
            icon_indexes,
            typ,
            icon,
            custom_icon,
            output_description.name.clone(),
            label,
            tooltip,
        ));
    }

    let feedback_data_len = widgets.iter().fold(0, |counter, item| {
        counter + item.get_feedback_data_requirement().size()
    });

    let template = ModuleTemplate {
        resource_name,
        label,
        category,
        tooltip,
        code_resource: yaml.name.replace(".module.yaml", ".module.ns"),
        size: (width, height),
        widget_outlines: widgets,
        inputs,
        outputs,
        feedback_data_len,
    };

    Ok(Module::create(
        rcrc(template),
        controls,
        complex_controls,
        default_inputs,
    ))
}

pub struct Registry {
    modules: HashMap<String, Module>,
    scripts: HashMap<String, String>,
    icon_indexes: HashMap<String, usize>,
    icons: Vec<Vec<u8>>,
}

impl Registry {
    fn load_module_resource(
        &mut self,
        name: &str,
        module_id: &str,
        buffer: Vec<u8>,
    ) -> Result<(), String> {
        let buffer_as_text = String::from_utf8(buffer).map_err(|e| {
            format!(
                "ERROR: The file {} is not a valid UTF-8 text document, caused by:\nERROR: {}",
                name, e
            )
        })?;
        let yaml = yaml::parse_yaml(&buffer_as_text, name)?;
        let module =
            create_module_prototype_from_yaml(&self.icon_indexes, module_id.to_owned(), &yaml)?;
        self.modules.insert(module_id.to_owned(), module);
        Ok(())
    }

    fn load_script_resource(&mut self, name: &str, buffer: Vec<u8>) -> Result<(), String> {
        let buffer_as_text = String::from_utf8(buffer).map_err(|e| {
            format!(
                "ERROR: The file {} is not a valid UTF-8 text document, caused by:\nERROR: {}",
                name, e
            )
        })?;
        self.scripts.insert(name.to_owned(), buffer_as_text);
        Ok(())
    }

    fn strip_path_and_extension<'a>(full_path: &'a str, extension: &str) -> &'a str {
        let last_slash = full_path.rfind("/").unwrap_or(0);
        let extension_start = full_path.rfind(extension).unwrap_or(full_path.len());
        &full_path[last_slash + 1..extension_start]
    }

    fn load_library_impl(
        &mut self,
        lib_name: &str,
        lib_reader: impl Read + Seek,
    ) -> Result<(), String> {
        let mut reader = zip::ZipArchive::new(lib_reader).map_err(|e| format!("ERROR: {}", e))?;
        // Modules can refer to icons, so load all the icons before all the modules.
        for index in 0..reader.len() {
            let mut file = reader.by_index(index).unwrap();
            let name = format!("{}:{}", lib_name, file.name());
            if name.ends_with("/") {
                // We don't do anything special with directories.
                continue;
            }
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer).map_err(|e| {
                format!(
                    "ERROR: Failed to read resource {}, caused by:\nERROR: {}",
                    file.name(),
                    e
                )
            })?;
            if name.ends_with(".icon.svg") {
                let file_name = Self::strip_path_and_extension(file.name(), ".icon.svg");
                let icon_id = format!("{}:{}", lib_name, file_name);
                self.icon_indexes.insert(icon_id, self.icons.len());
                self.icons.push(buffer);
            } else {
                // Don't error here, we'll wait to the second loop to check if a file really is
                // unrecognized. That way we only have to maintain one set of conditions.
            }
        }
        // Now load the modules and other files.
        for index in 0..reader.len() {
            let mut file = reader.by_index(index).unwrap();
            let name = format!("{}:{}", lib_name, file.name());
            if name.ends_with("/") {
                // We don't do anything special with directories.
                continue;
            }
            let mut buffer = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buffer).map_err(|e| {
                format!(
                    "ERROR: Failed to read resource {}, caused by:\nERROR: {}",
                    file.name(),
                    e
                )
            })?;
            if name.ends_with(".module.yaml") {
                let file_name = Self::strip_path_and_extension(file.name(), ".module.yaml");
                let module_id = format!("{}:{}", lib_name, file_name);
                self.load_module_resource(&name, &module_id, buffer)?;
            } else if name.ends_with(".ns") {
                self.load_script_resource(&name, buffer)?;
            } else if name.ends_with(".md") {
                // Ignore, probably just readme / license type stuff.
            } else if name.ends_with(".icon.svg") {
                // Already loaded earlier.
            } else {
                return Err(format!(
                    "ERROR: Not sure what to do with the file {}.",
                    name
                ));
            }
        }
        Ok(())
    }

    fn load_library_from_file(&mut self, path: &Path) -> Result<(), String> {
        let lib_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_owned();
        let extension_index = lib_name.rfind(".").unwrap_or(lib_name.len());
        let lib_name = (&lib_name[..extension_index]).to_owned();
        let file = File::open(path).map_err(|e| {
            format!(
                "ERROR: Failed to load library from {}, caused by:\nERROR: {}",
                path.to_string_lossy(),
                e
            )
        })?;
        self.load_library_impl(&lib_name, file).map_err(|e| {
            format!(
                "ERROR: Failed to load library from {}, caused by:\n{}",
                path.to_string_lossy(),
                e
            )
        })
    }

    pub fn new() -> (Self, Result<(), String>) {
        let mut registry = Self {
            modules: HashMap::new(),
            scripts: HashMap::new(),
            icon_indexes: HashMap::new(),
            icons: Vec::new(),
        };

        let base_library = std::include_bytes!(concat!(env!("OUT_DIR"), "/base.ablib"));
        let reader = std::io::Cursor::new(base_library as &[u8]);
        let result = registry
            .load_library_impl("base", reader)
            .map_err(|e| format!("ERROR: Failed to load base library, caused by:\n{}", e));

        (registry, result)
    }

    pub fn borrow_module(&self, id: &str) -> Option<&Module> {
        self.modules.get(id)
    }

    pub fn iterate_over_modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.values()
    }

    pub fn borrow_scripts(&self) -> &HashMap<String, String> {
        &self.scripts
    }

    pub fn lookup_icon(&self, name: &str) -> Option<usize> {
        self.icon_indexes.get(name).cloned()
    }

    pub fn get_num_icons(&self) -> usize {
        self.icons.len()
    }

    pub fn borrow_icon_data(&self, index: usize) -> &[u8] {
        &self.icons[index][..]
    }
}
