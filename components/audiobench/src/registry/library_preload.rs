use crate::{config::*, registry::yaml};
use shared_util::Version;
use std::{
    fs::{self, File},
    io::{Read, Seek},
    path::{Path, PathBuf},
};

pub struct LibraryInfo {
    pub internal_name: String,
    pub pretty_name: String,
    pub description: String,
    pub version: Version,
    pub dependencies: Vec<(String, Version)>,
}

pub(super) struct PreloadedLibrary {
    pub content: Box<dyn LibraryContentProvider>,
    pub info: LibraryInfo,
}

pub(super) trait LibraryContentProvider {
    fn get_num_files(&self) -> usize;
    fn get_file_name(&mut self, index: usize) -> String;
    fn get_full_path(&mut self, index: usize) -> Option<PathBuf>;
    fn read_file_contents(&mut self, index: usize) -> Result<Vec<u8>, String>;
}

// Allows loading a library from a plain directory.
struct DirectoryLibraryContentProvider {
    root_path: PathBuf,
    file_paths: Vec<PathBuf>,
}

impl DirectoryLibraryContentProvider {
    fn new(root_path: PathBuf) -> Result<Self, String> {
        let mut file_paths = Vec::new();
        let mut unvisited_paths = vec![PathBuf::new()];
        while let Some(visiting) = unvisited_paths.pop() {
            let reader_path = root_path.join(&visiting);
            let reader = fs::read_dir(&reader_path).map_err(|err| {
                format!(
                    "ERROR: Failed to list files in {}, caused by:\nERROR: {}",
                    reader_path.to_string_lossy(),
                    err
                )
            })?;
            for entry in reader {
                let entry = if let Ok(entry) = entry {
                    entry
                } else {
                    continue;
                };
                let path = entry.path();
                let local_path = visiting.join(entry.file_name());
                if path.is_dir() {
                    unvisited_paths.push(local_path);
                } else {
                    file_paths.push(local_path);
                }
            }
        }
        Ok(Self {
            root_path,
            file_paths,
        })
    }
}

impl LibraryContentProvider for DirectoryLibraryContentProvider {
    fn get_num_files(&self) -> usize {
        self.file_paths.len()
    }

    fn get_file_name(&mut self, index: usize) -> String {
        let value: String = self.file_paths[index].to_string_lossy().into();
        if cfg!(windows) {
            value.replace("\\", "/")
        } else {
            value
        }
    }

    fn get_full_path(&mut self, index: usize) -> Option<PathBuf> {
        Some(self.root_path.join(&self.file_paths[index]))
    }

    fn read_file_contents(&mut self, index: usize) -> Result<Vec<u8>, String> {
        fs::read(self.root_path.join(&self.file_paths[index]))
            .map_err(|err| format!("ERROR: {}", err))
    }
}

// Allows loading a library from a zip file.
pub(super) struct ZippedLibraryContentProvider<R: Read + Seek> {
    archive: zip::ZipArchive<R>,
    non_directory_files: Vec<usize>,
}

impl<R: Read + Seek> ZippedLibraryContentProvider<R> {
    pub(super) fn new(reader: R) -> Result<Self, String> {
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| {
            format!(
                "ERROR: File is not a valid ZIP archive, caused by:\nERROR: {}",
                e
            )
        })?;
        let non_directory_files = (0..archive.len())
            .filter(|element| !archive.by_index(*element).unwrap().name().ends_with("/"))
            .collect();
        Ok(Self {
            archive,
            non_directory_files,
        })
    }
}

impl<R: Read + Seek> LibraryContentProvider for ZippedLibraryContentProvider<R> {
    fn get_num_files(&self) -> usize {
        self.non_directory_files.len()
    }

    fn get_file_name(&mut self, index: usize) -> String {
        let value = self
            .archive
            .by_index(self.non_directory_files[index])
            .unwrap()
            .name()
            .to_owned();
        if cfg!(windows) {
            value.replace("\\", "/")
        } else {
            value
        }
    }

    fn get_full_path(&mut self, _index: usize) -> Option<PathBuf> {
        None
    }

    fn read_file_contents(&mut self, index: usize) -> Result<Vec<u8>, String> {
        let mut file = self
            .archive
            .by_index(self.non_directory_files[index])
            .unwrap();
        let mut buffer = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buffer).map_err(|err| {
            format!(
                "ERROR: Failed to read zipped file {}, caused by:\nERROR: {}",
                file.name(),
                err
            )
        })?;
        Ok(buffer)
    }
}

fn parse_library_info(name: &str, buffer: Vec<u8>) -> Result<LibraryInfo, String> {
    assert!(
        ENGINE_VERSION != Version::new(0, 0, 0),
        "ERROR: Engine version not provided during compilation."
    );
    let buffer_as_text = String::from_utf8(buffer).map_err(|e| {
        format!(
            "ERROR: Not a valid UTF-8 text document, caused by:\nERROR: {}",
            e
        )
    })?;
    let yaml = yaml::parse_yaml(&buffer_as_text, name)?;
    let internal_name = yaml.unique_child("internal_name")?.value.clone();
    let pretty_name = yaml.unique_child("pretty_name")?.value.clone();
    let description = yaml.unique_child("description")?.value.clone();
    let version = yaml.unique_child("version")?.parse()?;
    let mut dependencies = Vec::new();
    if let Ok(child) = yaml.unique_child("dependencies") {
        for child in &child.children {
            dependencies.push((child.name.clone(), child.parse()?));
        }
    }
    if !dependencies.iter().any(|(name, _)| name == "Factory")
        && internal_name != "User"
        && internal_name != "Factory"
    {
        return Err(format!(
            concat!(
                "ERROR: The library {} is missing a dependency on the Factory library. ",
                "Try adding the following lines in its library_info.yaml:\n",
                "dependencies:\n",
                "  Factory: {}"
            ),
            internal_name, ENGINE_VERSION
        ));
    }
    Ok(LibraryInfo {
        internal_name,
        pretty_name,
        description,
        version,
        dependencies,
    })
}

pub(super) fn preload_library(
    mut content: Box<dyn LibraryContentProvider>,
) -> Result<PreloadedLibrary, String> {
    for index in 0..content.get_num_files() {
        if &content.get_file_name(index) == "library_info.yaml" {
            let lib_info_name = format!("library_info.yaml");
            let buffer = content.read_file_contents(index).map_err(|err| {
                format!(
                    "ERROR: Failed to read file {}, caused by:\n{}",
                    &lib_info_name, err
                )
            })?;
            let lib_info = parse_library_info(&lib_info_name, buffer).map_err(|err| {
                format!(
                    "ERROR: Failed to parse {}, caused by:\n{}",
                    &lib_info_name, err
                )
            })?;
            return Ok(PreloadedLibrary {
                info: lib_info,
                content,
            });
        }
    }
    Err(format!("ERROR: could not find a library_info.yaml file",))
}

pub(super) fn preload_library_from_path(path: &Path) -> Result<PreloadedLibrary, String> {
    let lib_name: String = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into();
    if path.is_dir() {
        let content = DirectoryLibraryContentProvider::new(path.to_owned())?;
        preload_library(Box::new(content))
    } else {
        let extension_index = lib_name.rfind(".").unwrap_or(lib_name.len());
        if &lib_name[extension_index..] != ".ablib" {
            return Err(format!(
                concat!("ERROR: The file has an invalid extension \"{}\" (should be .ablib)"),
                &lib_name[extension_index..]
            ));
        }
        let lib_name = (&lib_name[..extension_index]).to_owned();
        let file = File::open(path)
            .map_err(|e| format!("ERROR: Failed to open file, caused by:\nERROR: {}", e))?;
        let content = ZippedLibraryContentProvider::new(file)?;
        preload_library(Box::new(content))
    }
}
