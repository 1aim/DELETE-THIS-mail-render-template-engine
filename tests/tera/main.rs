extern crate mail_common as common;
extern crate mail_headers as headers;
extern crate mail_types as mail;
#[macro_use]
extern crate mail_template as template;
extern crate mail_render_template_engine as render_template_engine;
extern crate soft_ascii_string;
extern crate futures;
extern crate regex;
#[macro_use]
extern crate serde_derive;

//TODO use custom integration test target for this
#[cfg(not(feature = "tera-engine"))]
compile_error!("need feature \"tera-engine\" to run tera integration tests");


use std::result::{Result as StdResult};
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::collections::HashMap;
use std::borrow::Cow;

use regex::Regex;
use futures::Future;
use soft_ascii_string::SoftAsciiString;

use common::MailType;
use common::encoder::EncodingBuffer;
use mail::{Mail, Context};
use mail::default_impl::simple_context;
use headers::components::{Email, Domain};
use headers::HeaderTryFrom;
use template::{MailSendData, InspectEmbeddedResources, Embedded};

use render_template_engine::{
    RenderTemplateEngine, DEFAULT_SETTINGS,
    TemplateSpec
};
use render_template_engine::tera::TeraRenderEngine;


#[derive(Serialize, InspectEmbeddedResources)]
struct UserData {
    name: &'static str
}


fn setup_context() -> simple_context::Context {
    let msg_id_domain = Domain::try_from("company_a.test").unwrap();
    let unique_part = SoftAsciiString::from_string("r73rc20").unwrap();
    simple_context::new(msg_id_domain, unique_part).unwrap()
}

fn setup_template_engine() -> RenderTemplateEngine<TeraRenderEngine> {
    let tera = TeraRenderEngine::new("./test_resources/tera_base/**/*").unwrap();
    let mut rte = RenderTemplateEngine::new(tera);
    let specs = TemplateSpec
        ::from_dirs("./test_resources/templates",  &*DEFAULT_SETTINGS)
        .unwrap();

    for (id, spec) in specs {
        rte.insert_spec(id, spec).unwrap();
    }

    rte
}

fn send_mail_to_string(mail: Mail, ctx: impl Context) -> String {
    let mut encoder = EncodingBuffer::new( MailType::Ascii );
    let encodable_mail = mail.into_encodeable_mail(ctx).wait().unwrap();
    encodable_mail.encode( &mut encoder ).unwrap();
    encoder.to_string().unwrap()
}

#[test]
fn use_tera_template_a() {
    let context = setup_context();
    let engine = setup_template_engine();

    let from        = Email::try_from("a@b.c").unwrap().into();
    let to          = Email::try_from("d@e.f").unwrap().into();
    let subject     = "Dear randomness";
    let template_id = Cow::Borrowed("template_a");
    let data        = UserData { name: "Liz" };

    let send_data = MailSendData::simple_new(
        from, to, subject,
        template_id, data
    );

    let mail = send_data.compose(&context, &engine).unwrap();

    // context's are meant to be cheaply cloneable,
    // e.g. in this case it just cloning a `Arc`
    let out_string = send_mail_to_string(mail, context.clone());

    assert_mail_out_is_as_expected(out_string);
}

fn assert_mail_out_is_as_expected(mail_out: String) {
    let mut line_iter = mail_out.lines();
    let mut capture_map = HashMap::new();

    let fd = File::open("./test_resources/template_a.out.regex").unwrap();
    let fd_line_iter = BufReader::new(fd).lines().map(StdResult::unwrap).enumerate();
    for (line_nr, mut template_line) in fd_line_iter {
        template_line.insert(0, '^');
        template_line.push('$');
        let mut line_regex = Regex::new(&*template_line).unwrap();
        let res_line = line_iter.next().unwrap();
        let captures = line_regex.captures(res_line).unwrap_or_else(|| {
            panic!("[{}] no match, regex: {:?}, line: {:?}", line_nr, line_regex, res_line);
        });
        for name in line_regex.capture_names().filter_map(|e|e){
            let value = captures.name(name).unwrap().as_str();
            let value2 = capture_map.entry(name.to_owned()).or_insert(value);
            assert_eq!(value, *value2)
        }
    }
    assert_eq!(line_iter.next(), None);
}
