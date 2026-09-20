use aec_ast::{Literal, SecretValue, TopLevelItem};
use aec_parser::parse;
use pretty_assertions::assert_eq;

#[test]
fn parses_minimal_agent() {
    let source = "agent Test\n";
    let program = parse(source).expect("should parse");
    assert_eq!(program.header.name.name, "Test");
    assert_eq!(program.items.len(), 0);
}

#[test]
fn parses_agent_with_underscore_name() {
    let source = "agent My_Agent_123\n";
    let program = parse(source).expect("should parse");
    assert_eq!(program.header.name.name, "My_Agent_123");
}

#[test]
fn parses_simple_import() {
    let source = "agent Test\nimport \"./models.aec\"\n";
    let program = parse(source).expect("should parse");
    assert_eq!(program.items.len(), 1);
    match &program.items[0] {
        TopLevelItem::Import(imp) => {
            assert_eq!(imp.path, "./models.aec");
            assert!(imp.alias.is_none());
        }
        _ => panic!("expected import"),
    }
}

#[test]
fn parses_import_with_alias() {
    let source = "agent Test\nimport \"./models.aec\" as models\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Import(imp) => {
            assert_eq!(imp.path, "./models.aec");
            assert_eq!(imp.alias.as_ref().unwrap().name, "models");
        }
        _ => panic!("expected import"),
    }
}

#[test]
fn parses_secrets_block() {
    let source = "agent Test\nsecrets {\n    openai_key: env(\"OPENAI_KEY\")\n    db_url: \"postgres://localhost\"\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Secrets(secrets) => {
            assert_eq!(secrets.entries.len(), 2);
            match &secrets.entries[0].value {
                SecretValue::Env(var) => assert_eq!(var, "OPENAI_KEY"),
                _ => panic!("expected env"),
            }
        }
        _ => panic!("expected secrets"),
    }
}

#[test]
fn parses_simple_model() {
    let source = "agent Test\nmodel primary {\n    provider: \"openai\"\n    name: \"gpt-4o\"\n    temperature: 0.1\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Model(model) => {
            assert_eq!(model.name.name, "primary");
            match &model.fields[0] {
                aec_ast::ModelField::Property(p) => {
                    assert_eq!(p.value, Literal::String("openai".to_string()));
                }
                _ => panic!("expected property"),
            }
        }
        _ => panic!("expected model"),
    }
}

#[test]
fn parses_model_with_retry() {
    let source = "agent Test\nmodel primary {\n    provider: \"openai\"\n    retry {\n        attempts: 3\n    }\n}\n";
    let program = parse(source).expect("should parse");
    match &program.items[0] {
        TopLevelItem::Model(model) => {
            let retry = model.fields.iter().find_map(|f| {
                if let aec_ast::ModelField::Retry(r) = f { Some(r) } else { None }
            }).expect("should have retry");
            assert_eq!(retry.fields.len(), 1);
        }
        _ => panic!("expected model"),
    }
}

#[test]
fn parses_full_sprint1_program() {
    let source = r#"agent SupportAgent

import "./models.aec" as models

secrets {
    openai_key: env("OPENAI_API_KEY")
    db_url: env("DATABASE_URL")
}

model primary {
    provider: "openai"
    name: "gpt-4o"
    temperature: 0.3
    max_tokens: 2000
}
"#;
    let program = parse(source).expect("should parse full");
    assert_eq!(program.header.name.name, "SupportAgent");
    assert_eq!(program.items.len(), 3);
}

#[test]
fn ignores_comments() {
    let source = "# comment\nagent Test\n# another\nsecrets {\n    # inner\n    key: env(\"KEY\")\n}\n";
    let program = parse(source).expect("should parse with comments");
    assert_eq!(program.header.name.name, "Test");
}

#[test]
fn fails_without_agent_header() {
    let source = "secrets {\n    key: env(\"KEY\")\n}\n";
    let result = parse(source);
    assert!(result.is_err(), "should fail without agent header");
}
