use tera_crate::{Tera, TesterFn, FilterFn, GlobalFn};
use serde::Serialize;

use ::traits::{RenderEngine, RenderEngineBase, AdditionalCIds};
use ::spec::{TemplateSpec, SubTemplateSpec, TemplateSource};

use self::error::TeraError;

pub mod error;

pub struct TeraRenderEngine {
    tera: Tera
}

impl TeraRenderEngine {

    /// create a new TeraRenderEngine given a base_templates_dir
    ///
    /// The `base_templates_glob` contains a number of tera templates which can be used to
    /// inherit (or include) from e.g. a `base_mail.html` which is then used in all
    /// `mail.html` templates through `{% extends "base_mail.html" %}`.
    ///
    /// The `base_templates_glob` _is separate from template dirs used by
    /// the `RenderTemplateEngine`_. It contains only tera templates to be reused at
    /// other places.
    ///
    pub fn new(base_templats_glob: &str) -> Result<Self, TeraError> {
        let tera = Tera::new(base_templats_glob)?;

        Ok(TeraRenderEngine { tera })
    }

    //TODO chang it
    /// Reloads all base templates, but no `RenderTemplateEngine` specific templates.
    /// After a reload `RenderTemplateEngine` specific templates will be loaded when
    /// they are used the next time.
    ///
    pub fn reload_base_only(&mut self) -> Result<(), TeraError> {
        //full_reload doe NOT a full reload what it does is
        // 1. discard all templates which are from a Tera::extend call
        //    (yes you can't reload them at all)
        // 2. load all templates from a glob
        //
        // No template path is used at all, even through all templates do have path's assigned
        // them if they where added through a path, well this actually happens to be exactly what
        // we want even through it's not what it says it is.
        Ok(self.tera.full_reload()?)
    }

    /// expose `Tera::register_filter`
    pub fn register_filter(&mut self, name: &str, filter: FilterFn) {
        self.tera.register_filter(name, filter);
    }

    /// exposes `Tera::register_tester`
    pub fn register_tester(&mut self, name: &str, tester: TesterFn) {
        self.tera.register_tester(name, tester);
    }

    /// exposes `Tera::register_global_function`
    pub fn register_global_function(&mut self, name: &str, function: GlobalFn) {
        self.tera.register_global_function(name, function)
    }

    /// exposes `Tera::autoescape_on`
    pub fn set_autoescape_file_suffixes(&mut self, suffixes: Vec<&'static str>) {
        self.tera.autoescape_on(suffixes)
    }

    //TODO chang it
    /// preloads a `RenderTemplateEngine` template, templates loaded this
    /// way will be discarded once `reload_base_only` is called.
    pub fn preload_rte_template(&mut self, id: &str) -> Result<(), TeraError> {
        Ok(self.tera.add_template_file(id, None)?)
    }

}

impl RenderEngineBase for TeraRenderEngine {
    // nothing gurantees that the templates use \r\n, so by default fix newlines
    // but it can be disabled
    const PRODUCES_VALID_NEWLINES: bool = false;

    type RenderError = TeraError;
    type LoadingError = TeraError;

    fn load_templates(&mut self, spec: &TemplateSpec) -> Result<(), Self::LoadingError> {
        let mut loaded = Vec::new();

        for sub_spec in spec.sub_specs() {
            match *sub_spec.source() {
                TemplateSource::Path(ref path) => {
                    try_add_sub_template(
                        &mut self.tera,
                        path,
                        &mut loaded,
                        |tera| Ok(tera.add_template_file(path, None)?)
                    )?;
                },
                TemplateSource::Source { ref id, ref content } => {
                    try_add_sub_template(
                        &mut self.tera,
                        id,
                        &mut loaded,
                        |tera| Ok(tera.add_raw_template(id, content)?)
                    )?;
                }
            }
        }
        Ok(())
    }


    /// This can be used to reload a templates.
    fn unload_templates(&mut self, spec: &TemplateSpec) {
        for sub_spec in spec.sub_specs() {
            let id = sub_spec.source().id();
            self.tera.templates.remove(id);
        }
    }


    fn unknown_template_id_error(id: &str) -> Self::RenderError {
        TeraError::UnknowTemplateId { id: id.to_owned() }
    }
}


fn try_add_sub_template<'s, 'l: 's>(
    tera: &'s mut Tera,
    id: &'l str,
    loaded: &'s mut Vec<&'l str>,
    add_op: impl FnOnce(&mut Tera) -> Result<(), TeraError>
) -> Result<(), TeraError> {
    if tera.templates.contains_key(id) {
        error_cleanup(tera, loaded);
        return Err(TeraError::TemplateIdCollision { id: id.to_owned() });
    }
    if let Err(error) = add_op(tera) {
        error_cleanup(tera, loaded);
        return Err(error);
    }
    loaded.push(id);
    Ok(())
}

fn error_cleanup(tera: &mut Tera, added_names: &Vec<&str>) {
    for name in added_names {
        tera.templates.remove(*name);
    }
}

#[derive(Serialize)]
struct DataWrapper<'a,D: Serialize + 'a> {
    data: &'a D,
    cids: AdditionalCIds<'a>
}

impl<D> RenderEngine<D> for TeraRenderEngine
    where D: Serialize
{
    fn render(
        &self,
        spec: &SubTemplateSpec,
        data: &D,
        cids: AdditionalCIds
    ) -> Result<String, Self::RenderError> {
        let data = &DataWrapper { data, cids };
        let id = spec.source().id();
        Ok(self.tera.render(id, data)?)
    }
}

