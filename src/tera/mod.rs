use std::sync::RwLock;

use tera_crate::{Tera, TesterFn, FilterFn, GlobalFn};
use serde::Serialize;

use ::{RenderEngineBase, RenderEngine, AdditionalCIds};

use self::error::TeraError;

pub mod error;

pub struct TeraRenderEngine {
    tera: RwLock<Tera>
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

        Ok(TeraRenderEngine { tera: RwLock::new(tera) })
    }

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
        Ok(self.tera.get_mut().unwrap().full_reload()?)
    }

    /// expose `Tera::register_filter`
    pub fn register_filter(&mut self, name: &str, filter: FilterFn) {
        self.tera.get_mut().unwrap().register_filter(name, filter);
    }

    /// exposes `Tera::register_tester`
    pub fn register_tester(&mut self, name: &str, tester: TesterFn) {
        self.tera.get_mut().unwrap().register_tester(name, tester);
    }

    /// exposes `Tera::register_global_function`
    pub fn register_global_function(&mut self, name: &str, function: GlobalFn) {
        self.tera.get_mut().unwrap().register_global_function(name, function)
    }

    /// exposes `Tera::autoescape_on`
    pub fn set_autoescape_file_suffixes(&mut self, suffixes: Vec<&'static str>) {
        self.tera.get_mut().unwrap().autoescape_on(suffixes)
    }

    /// preloads a `RenderTemplateEngine` template, templates loaded this
    /// way will be discarded once `reload_base_only` is called.
    pub fn preload_rte_template(&mut self, id: &str) -> Result<(), TeraError> {
        Ok(self.tera.get_mut().unwrap().add_template_file(id, None)?)
    }
}

impl RenderEngineBase for TeraRenderEngine {
    // nothing gurantees that the templates use \r\n, so by default fix newlines
    // but it can be disabled
    const PRODUCES_VALID_NEWLINES: bool = false;
}

#[derive(Serialize)]
struct DataWrapper<'a,D: Serialize + 'a> {
    data: &'a D,
    cids: AdditionalCIds<'a>
}

impl<D> RenderEngine<D> for TeraRenderEngine
    where D: Serialize
{
    type Error = TeraError;

    fn render(&self, id: &str, data: &D, cids: AdditionalCIds) -> Result<String, Self::Error> {
        let data = &DataWrapper { data, cids };
        {
            let tera = self.tera.read().unwrap();
            if tera.templates.contains_key(id) {
                return Ok(tera.render(id, data)?);
            }
        }
        {
            let mut tera = self.tera.write().unwrap();
            if !tera.templates.contains_key(id) {
                tera.add_template_file(id, None)?;
            }
            Ok(tera.render(id, data)?)
        }

    }
}

