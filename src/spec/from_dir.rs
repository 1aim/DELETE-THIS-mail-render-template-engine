use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs::DirEntry;

use vec1::Vec1;

use mail::context::Source;
use mail::{Resource, IRI};

use ::error::{CreatingSpecError, CreatingSpecErrorVariant};
use ::utils::{new_string_path, new_str_path};
use ::{TemplateSpec, SubTemplateSpec};
use ::settings::{LoadSpecSettings, Type};

//TODO missing global template level embeddings

pub(crate) fn from_dirs(
    templates_dir: &Path,
    settings: &LoadSpecSettings
) -> Result<Vec<(String, TemplateSpec)>, CreatingSpecError>
{
    let mut specs = Vec::new();
    for entry in templates_dir.read_dir()? {
        let entry = entry?;
        if entry.metadata()?.is_dir() {
            let id = entry.file_name()
                .into_string()
                .map_err(|file_name| CreatingSpecErrorVariant::NonStringPath(file_name.into()))?;

            specs.push((id, TemplateSpec::from_dir(entry.path(), settings)?));
        }
    }
    Ok(specs)
}

pub(crate) fn from_dir(base_path: &Path, settings: &LoadSpecSettings) -> Result<TemplateSpec, CreatingSpecError> {
    let mut glob_embeddings = HashMap::new();
    let mut sub_template_dirs = Vec::new();
    for folder in base_path.read_dir()? {
        let entry = folder?;
        if entry.file_type()?.is_dir() {
            let type_name = entry.file_name()
                .into_string().map_err(|_| CreatingSpecErrorVariant::NonStringPath(entry.path().into()))?;
            let (prio, type_) = settings.get_type_with_priority(&*type_name)
                .ok_or_else(|| CreatingSpecErrorVariant::MissingTypeInfo { type_name: type_name.clone() })?;
            sub_template_dirs.push((prio, entry.path(), type_));
        } else {
            let (name, resource_spec) = embedding_from_path(entry.path(), settings)?;
            glob_embeddings.insert(name, resource_spec);
        }
    }

    sub_template_dirs.sort_by_key(|data| data.0);

    let mut sub_specs = Vec::with_capacity(sub_template_dirs.len());
    for (_, dir_path, type_) in sub_template_dirs {
        sub_specs.push(sub_template_from_dir(&*dir_path, type_, settings)?);
    }

    let sub_specs = Vec1::from_vec(sub_specs)
        .map_err(|_| CreatingSpecErrorVariant::NoSubTemplatesFound { dir: base_path.into() })?;
    TemplateSpec::new_with_embeddings_and_base_path(
        sub_specs, glob_embeddings, base_path.to_owned())
}


fn sub_template_from_dir(dir: &Path, type_: &Type, settings: &LoadSpecSettings)
    -> Result<SubTemplateSpec, CreatingSpecError>
{
    let FindResult { template_file, other_files:embeddings } = find_files(dir, settings)?;
    let media_type = type_.to_media_type_for(&template_file)?;

    SubTemplateSpec::new(template_file, media_type, embeddings)
}


fn is_template_file(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|name| name.starts_with("mail."))
        .unwrap_or(false)
}

struct FindResult {
    template_file: PathBuf,
    other_files: HashMap<String, Resource>,

}

fn find_files(in_dir: &Path, settings: &LoadSpecSettings) -> Result<FindResult, CreatingSpecError> {
    use std::collections::hash_map::Entry::*;

    let mut template_file = None;
    let mut other_files = HashMap::new();
    for entry in in_dir.read_dir()? {
        let entry = entry?;
        if is_template_file(&entry) {
            if template_file.is_none() {
                template_file = Some(entry.path())
            } else {
                return Err(CreatingSpecErrorVariant::MultipleTemplateFiles { dir: in_dir.into() }.into());
            }
        } else {
            let (key, value) = embedding_from_path(entry.path(), settings)?;
             match other_files.entry(key) {
                Occupied(oe) => {
                    return Err(CreatingSpecErrorVariant::DuplicateEmbeddingName { name: oe.key().clone() }.into());
                },
                Vacant(ve) => {ve.insert(value);}
            }
        }
    }

    if let Some(template_file) = template_file {
        Ok(FindResult {
            template_file,
            other_files
        })
    } else {
        Err(CreatingSpecErrorVariant::TemplateFileMissing { dir: in_dir.into() }.into())
    }
}

fn embedding_from_path(path: PathBuf, settings: &LoadSpecSettings)
                       -> Result<(String, Resource), CreatingSpecError>
{
    if !path.is_file() {
        return Err(CreatingSpecErrorVariant::NotAFile(path.into()).into());
    }

    let file_name = new_string_path(
        path.file_name()
        // UNWRAP_SAFE: file_name returns the file (,dir,symlink) name which
        // has to exist for a dir_entry
        .unwrap())?;

    let name = file_name.split(".")
        .next()
        //UNWRAP_SAFE: Split iterator has always at last one element
        .unwrap()
        .to_owned();

    //TODO we can remove the media type sniffing from here
    let media_type = settings.determine_media_type(&path)?;

    let source = Source {
        iri: iri_from_path(path)?,
        use_name: None,
        use_media_type: Some(media_type)
    };

    let resource = Resource::new(source);

    Ok((name, resource))
}

fn iri_from_path<IP: AsRef<Path> + Into<PathBuf>>(path: IP) -> Result<IRI, CreatingSpecError> {
    {
        let path_ref = path.as_ref();
        if let Ok(strfy) = new_str_path(&path_ref) {
            if let Ok(iri) = IRI::from_parts("path", strfy) {
                return Ok(iri)
            }
        }
    }
    Err(CreatingSpecErrorVariant::IRIConstructionFailed {
        scheme: "path",
        tail: path.into().into()
    }.into())
}