use std::path::{Path, PathBuf};
use std::collections::HashMap;

use vec1::Vec1;

use mail::context::Source;
use mail::{Resource, IRI};

use ::error::{LoadingSpecError, LoadingSpecErrorVariant};
use ::utils::{new_string_path, new_str_path};
use ::{TemplateSpec, SubTemplateSpec};
use ::settings::{LoadSpecSettings, Type};

//TODO missing global template level embeddings
//TODO missing caching (of Resources)


pub(crate) fn from_dir(base_path: &Path, settings: &LoadSpecSettings) -> Result<TemplateSpec, LoadingSpecError> {
    let mut glob_embeddings = HashMap::new();
    let mut sub_template_dirs = Vec::new();
    for folder in base_path.read_dir()? {
        let entry = folder?;
        if entry.file_type()?.is_dir() {
            let type_name = entry.file_name()
                .into_string().map_err(|_| LoadingSpecErrorVariant::NonStringPath(entry.path().into()))?;
            let (prio, type_) = settings.get_type_with_priority(&*type_name)
                .ok_or_else(|| LoadingSpecErrorVariant::MissingTypeInfo { type_name: type_name.clone() })?;
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
        .map_err(|_| LoadingSpecErrorVariant::NoSubTemplatesFound { dir: base_path.into() })?;
    TemplateSpec::new_with_embeddings_and_base_path(
        sub_specs, glob_embeddings, base_path.to_owned())
}


//NOTE: if this is provided as a pub utility provide a wrapper function instead which
// only accepts dir_path + settings and gets the rest from it
fn sub_template_from_dir(dir: &Path, type_: &Type, settings: &LoadSpecSettings)
    -> Result<SubTemplateSpec, LoadingSpecError>
{
    let template_file = find_template_file(dir, type_)?;
    let media_type = type_.to_media_type_for(&*template_file)?;
    let embeddings = find_embeddings(dir, &*template_file, settings)?;

    SubTemplateSpec::new(template_file, media_type, embeddings, Vec::new())
}

fn find_template_file(dir: &Path, type_: &Type) -> Result<PathBuf, LoadingSpecError> {
    let base_name = type_.template_base_name();
    let file = type_.suffixes()
        .iter()
        .map(|suffix| dir.join(base_name.to_owned() + suffix))
        .find(|path| path.exists())
        .ok_or_else(|| LoadingSpecErrorVariant::TemplateFileMissing { dir: dir.into() })?;

    Ok(file)
}


fn find_embeddings(target_path: &Path, template_file: &Path, settings: &LoadSpecSettings)
    -> Result<HashMap<String, Resource>, LoadingSpecError>
{
    use std::collections::hash_map::Entry::*;

    let mut embeddings = HashMap::new();
    for entry in target_path.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        if path != template_file {
            let (key, value) = embedding_from_path(path, settings)?;
            match embeddings.entry(key) {
                Occupied(oe) => {
                    return Err(LoadingSpecErrorVariant::DuplicateEmbeddingName { name: oe.key().clone() }.into());
                },
                Vacant(ve) => {ve.insert(value);}
            }
        }
    }
    Ok(embeddings)
}

fn embedding_from_path(path: PathBuf, settings: &LoadSpecSettings)
                       -> Result<(String, Resource), LoadingSpecError>
{
    if !path.is_file() {
        return Err(LoadingSpecErrorVariant::NotAFile(path.into()).into());
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

fn iri_from_path<IP: AsRef<Path> + Into<PathBuf>>(path: IP) -> Result<IRI, LoadingSpecError> {
    {
        let path_ref = path.as_ref();
        if let Ok(strfy) = new_str_path(&path_ref) {
            if let Ok(iri) = IRI::from_parts("path", strfy) {
                return Ok(iri)
            }
        }
    }
    Err(LoadingSpecErrorVariant::IRIConstructionFailed {
        scheme: "path",
        tail: path.into().into()
    }.into())
}