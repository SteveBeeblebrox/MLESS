use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{
    CharacterTokens, EndTag, NullCharacterToken, StartTag, TagToken, DoctypeToken, CommentToken, EOFToken,
    ParseError, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts, BufferQueue, Tag
};

use std::convert::TryFrom;
use std::default::Default;
use std::sync::Once;

use v8;

static LESS: &str = include_str!(r"less.js");

#[derive(Clone)]
pub struct CompileOptions {
}

static V8_INIT: Once = Once::new();

#[allow(dead_code)]
pub fn compile_less(text: &str, _options: CompileOptions) -> Option<String> {
    V8_INIT.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
    
    let isolate = &mut v8::Isolate::new(Default::default());

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);

    let dummy_object = v8::String::new(scope, r"
        const __v8_dummy_object__ = new Proxy(new Function(), {
            get(_,p) {return p===Symbol.toPrimitive?()=>'':__v8_dummy_object__},
            apply() {return __v8_dummy_object__}
        });
    ")?;
    
    let script = v8::Script::compile(scope, dummy_object, None)?;
    script.run(scope)?;

    let less_compiler = v8::String::new(scope, LESS)?;
    
    let script = v8::Script::compile(scope, less_compiler, None)?;
    script.run(scope)?;

    let less_obj_name = v8::String::new(scope, "less")?.into();
    let less_obj = context.global(scope).get(scope, less_obj_name)?;
    
    let render_func_name = v8::String::new(scope, "render")?.into();
    let render_function = less_obj.to_object(scope)?.get(scope, render_func_name)?.to_object(scope)?;
    let render_function = v8::Local::<v8::Function>::try_from(render_function).ok()?;

    let text = v8::String::new(scope, text)?.into();

    let args = v8::Object::new(scope);



    let result = render_function.call(scope, less_obj, &[text, args.into()])?;

    if result.is_promise() {
        let promise = v8::Local::<v8::Promise>::try_from(result).ok()?;

        while promise.state() == v8::PromiseState::Pending {
            scope.perform_microtask_checkpoint();
        }
        if promise.state() == v8::PromiseState::Rejected {
            panic!("Promise rejected");
        } else {
            let css_name = v8::String::new(scope, "css")?.into();
            let resolved = promise.result(scope).to_object(scope)?.get(scope, css_name)?;
            return Some(resolved.to_string(scope)?.to_rust_string_lossy(scope))
        }
    } else {
        panic!("Value is not a promise");
    }
}


#[derive(PartialEq)]
enum TargetType {
    None, Classic
}

struct Document {
    options: CompileOptions,
    less_mode: TargetType,
    inner_html: String,
    script_buffer: String
}

impl Document {
    fn write_text<S: AsRef<str>>(&mut self, html: S) {
        if self.less_mode == TargetType::None {
            self.inner_html.push_str(html.as_ref());
        } else {
            self.script_buffer.push_str(html.as_ref());
        }
    }
    fn new(options: CompileOptions) -> Self {
        Document {
            options,
            less_mode: TargetType::None,
            inner_html: String::new(),
            script_buffer: String::new()
        }
    }
}

impl TokenSink for &mut Document {
    type Handle = ();
    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        true
    }
    fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        match token {
            CharacterTokens(str_tendril) => {
                self.write_text(str_tendril);
            },
            DoctypeToken(doctype) => {
                self.write_text("<!DOCTYPE ");
                if let Some(name) = doctype.name {
                    self.write_text(name.as_ref());
                }
                if let Some(public_id) = doctype.public_id {
                    self.write_text(format!(" PUBLIC \"{}\"", public_id));
                }
                if let Some(system_id) = doctype.system_id {
                    self.write_text(format!(" \"{}\"", system_id));
                }
                self.write_text(">");
            },
            TagToken(mut tag) => {
                fn get_tag_str(tag: Tag) -> String {
                    let mut attrs = String::new();
                    
                    for attr in tag.attrs.iter() {
                        if attr.value.len() > 0 {
                            let value = attr.value.to_string();
                            
                            if value.contains("\"") && !value.contains("'") {
                                attrs.push_str(&format!(" {}='{}'", attr.name.local, value));
                            } else if value.contains("'") && !value.contains("\"") {
                                attrs.push_str(&format!(r#" {}="{}""#, attr.name.local, value));
                            }
                            else {
                                attrs.push_str(&format!(r#" {}="{}""#, attr.name.local, attr.value.as_ref().replace("\"", "&quot;")));
                            }
                        } else {
                            attrs.push_str(&format!(" {}", attr.name.local));
                        }
                    }
                    
                    return match tag.kind {
                        _ if tag.self_closing => format!("<{}{}/>", tag.name, attrs),
                        StartTag => format!("<{}{}>", tag.name, attrs),
                        EndTag => format!("</{}>", tag.name),
                    }
                }

                if tag.name.to_lowercase() == "style" {
                    match tag.kind {
                        StartTag => {
                            if let Some(attr) = tag.attrs.iter_mut().find(|attr| attr.name.local.as_ref() == "type") {
                                match attr.value.as_ref() {
                                    "text/less" => {
                                        tag.attrs.retain(|attr| attr.name.local.as_ref() != "type");
                                        self.write_text(get_tag_str(tag));
                                        self.less_mode = TargetType::Classic
                                    },
                                    _ => self.write_text(get_tag_str(tag))
                                }
                            } else {
                                self.write_text(get_tag_str(tag));
                            }
                            return TokenSinkResult::RawData(html5ever::tokenizer::states::RawKind::ScriptData);
                        },
                        EndTag => {
                            if self.less_mode != TargetType::None {
                                self.less_mode = TargetType::None;
                                
                                #[allow(unused_mut)]
                                let mut options = self.options.clone();
                                

                                let script_buffer = self.script_buffer.clone();
                                
                                let mut lines: Vec<&str> = script_buffer.lines().collect::<Vec<&str>>();
                                lines.retain(|line| !line.trim().is_empty());
                                let mut indentation = String::new();

                                if lines.len() > 0 {
                                    for i in 0..lines[0].len() {
                                        if let Some(char) = lines[0].chars().nth(i) {
                                            if char.is_whitespace() && lines.iter().all(move |line| line.chars().nth(i) == Some(char)) {
                                                indentation.push(char);
                                            } else {
                                                break;
                                            }
                                        } else {
                                            break;
                                        }
                                    }
                                }

                                self.write_text(format!("\n{}",
                                    compile_less(
                                            &script_buffer.lines().map(|line| line.strip_prefix(indentation.as_str()).unwrap_or(line).to_string()).collect::<Vec<String>>().join("\n"),
                                            options
                                        ).expect("Error compiling Less within HTML")
                                    .lines().map(|line| format!("{}{}", indentation, line)).collect::<Vec<String>>().join("\n")
                                ));
                                
                                let last = script_buffer.lines().last().unwrap_or("");
                                if last.chars().all(|char| char.is_whitespace()) {
                                    self.write_text(format!("\n{}", last));
                                }

                                self.script_buffer = String::new();
                            }
                            self.write_text(get_tag_str(tag));
                            return TokenSinkResult::Continue
                        }
                    }
                }
                else {
                    self.write_text(get_tag_str(tag));
                }
            },
            CommentToken(comment) => {
                self.write_text(format!("<!--{}-->", comment));
            },
            NullCharacterToken => self.write_text("\0"),
            ParseError(_error) => (),
            EOFToken => ()
        }
        TokenSinkResult::Continue
    }
}

#[allow(dead_code)]
pub fn compile_html(text: &str, options: CompileOptions) -> Option<String> {
    let mut document = Document::new(options);
    
    let mut input = BufferQueue::new();
    input.push_back(StrTendril::from(text));

    let mut tokenizer = Tokenizer::new(&mut document, TokenizerOpts {
        ..Default::default()
    });

    let _ = tokenizer.feed(&mut input);
    tokenizer.end();

    return Some(document.inner_html);
}