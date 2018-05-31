extern crate mail_template as compos;
extern crate mail_types as mail;
extern crate mail_render_template_engine;

use std::path::Path;

use mail_render_template_engine::{TemplateSpec, DEFAULT_SETTINGS};


#[test]
fn load_template_a() {
    let settings = &*DEFAULT_SETTINGS;
    let a_spec = TemplateSpec::from_dir("./test_resources/templates/template_a", settings).unwrap();

    // 1. test base_path
    assert_eq!(a_spec.base_path().unwrap(), Path::new("./test_resources/templates/template_a"));

    // 2. test embeddings
    let embeddings = a_spec.embeddings();
    assert_eq!(embeddings.len(), 1);
    let emb_0 = embeddings.get("portfolio").unwrap();
    assert_eq!(
        emb_0.source().unwrap().iri.as_str(),
        "path:./test_resources/templates/template_a/portfolio.pdf"
    );

    // 3. test subtemplate spec
    let sub_specs = a_spec.sub_specs();
    assert_eq!(sub_specs.len(), 2);
    let text = &sub_specs[0];
    let html = &sub_specs[1];
    // 3.1. test text subtempalte
    assert_eq!(text.path(), Path::new("./test_resources/templates/template_a/text/mail.txt"));
    assert_eq!(text.media_type().as_str_repr(), "text/plain; charset=utf-8");
    assert!(text.embeddings().is_empty());
    assert!(text.attachments().is_empty());
    // 3.2. test html subtemplate
    assert_eq!(html.path(), Path::new("./test_resources/templates/template_a/html/mail.html"));
    assert_eq!(html.media_type().as_str_repr(), "text/html; charset=utf-8");
    let embeddings = html.embeddings();
    assert_eq!(embeddings.len(), 1);
    let logo = embeddings.get("logo").unwrap();
    assert_eq!(
        logo.source().unwrap().iri.as_str(),
        "path:./test_resources/templates/template_a/html/logo.png"
    );
    assert!(text.attachments().is_empty());
}

