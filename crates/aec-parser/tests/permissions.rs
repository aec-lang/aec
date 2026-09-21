use aec_ast::{Literal, PermissionsEntry, TopLevelItem};
use aec_parser::parse;

const SOURCE: &str = r##"agent Sandbox

permissions {
    network: ["api.openai.com", "api.telegram.org"]
    filesystem {
        read:  ["/var/log/sysstat"]
        write: ["cache"]
    }
    system {
        metrics: true
        restart: false
    }
}

limits {
    concurrency: 8
    timeout: 10s
}

fn main() -> int {
    return 0
}
"##;

fn permissions_block(
    program: &aec_ast::Program,
) -> &aec_ast::PermissionsBlock {
    program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Permissions(p) => Some(p),
            _ => None,
        })
        .expect("a permissions block must be present")
}

fn limits_block(program: &aec_ast::Program) -> &aec_ast::LimitsBlock {
    program
        .items
        .iter()
        .find_map(|item| match item {
            TopLevelItem::Limits(l) => Some(l),
            _ => None,
        })
        .expect("a limits block must be present")
}

#[test]
fn parses_full_permissions_and_limits_block() {
    let program = parse(SOURCE).expect("parse should succeed");

    let perms = permissions_block(&program);
    assert_eq!(perms.entries.len(), 3);

    let network = perms
        .entries
        .iter()
        .find_map(|e| match e {
            PermissionsEntry::Network(n) => Some(n),
            _ => None,
        })
        .expect("network rule");
    assert_eq!(network.hosts, vec!["api.openai.com", "api.telegram.org"]);

    let fs = perms
        .entries
        .iter()
        .find_map(|e| match e {
            PermissionsEntry::Filesystem(f) => Some(f),
            _ => None,
        })
        .expect("filesystem rule");
    assert_eq!(fs.read, vec!["/var/log/sysstat"]);
    assert_eq!(fs.write, vec!["cache"]);

    let system = perms
        .entries
        .iter()
        .find_map(|e| match e {
            PermissionsEntry::System(s) => Some(s),
            _ => None,
        })
        .expect("system rule");
    assert_eq!(system.metrics, Some(true));
    assert_eq!(system.restart, Some(false));

    let limits = limits_block(&program);
    assert_eq!(limits.entries.len(), 2);

    let concurrency = limits
        .entries
        .iter()
        .find(|e| e.name.name == "concurrency")
        .expect("concurrency");
    assert_eq!(concurrency.value, Literal::Int(8));

    let timeout = limits
        .entries
        .iter()
        .find(|e| e.name.name == "timeout")
        .expect("timeout");
    assert!(matches!(timeout.value, Literal::Duration(_)));
}

#[test]
fn permissions_block_order_does_not_matter() {
    let src = r##"agent T

permissions {
    system {
        restart: true
    }
    network: ["a.com"]
    filesystem {
        write: ["/tmp"]
    }
}

fn main() -> int { return 0 }
"##;
    let program = parse(src).expect("parse should succeed");
    let perms = permissions_block(&program);
    assert_eq!(perms.entries.len(), 3);

    let system = perms
        .entries
        .iter()
        .find_map(|e| match e {
            PermissionsEntry::System(s) => Some(s),
            _ => None,
        })
        .unwrap();
    // Only restart was declared, so metrics must stay None (not a fabricated false)
    assert_eq!(system.metrics, None);
    assert_eq!(system.restart, Some(true));
}

#[test]
fn empty_permissions_block_is_allowed() {
    let src = "agent T\n\npermissions {\n}\n\nfn main() -> int { return 0 }\n";
    let program = parse(src).expect("parse should succeed");
    let perms = permissions_block(&program);
    assert!(perms.entries.is_empty());
}

#[test]
fn network_rule_rejects_non_string_elements() {
    let src = "agent T\n\npermissions {\n    network: [123]\n}\n\nfn main() -> int { return 0 }\n";
    let err = parse(src).expect_err("a number in the allowlist must be rejected");
    assert!(
        err.to_string().contains("expected a string literal"),
        "the error message should explain the problem, got: {}",
        err
    );
}

#[test]
fn filesystem_rule_rejects_unknown_field() {
    let src = "agent T\n\npermissions {\n    filesystem {\n        execute: [\"x\"]\n    }\n}\n\nfn main() -> int { return 0 }\n";
    let err = parse(src).expect_err("an unknown field must be rejected");
    assert!(
        err.to_string().contains("unknown filesystem field"),
        "the error message should explain the problem, got: {}",
        err
    );
}

#[test]
fn limits_accepts_duration_and_int_values() {
    let src = r##"agent T

limits {
    timeout: 250ms
    concurrency: 4
}

fn main() -> int { return 0 }
"##;
    let program = parse(src).expect("parse should succeed");
    let limits = limits_block(&program);
    let timeout = limits
        .entries
        .iter()
        .find(|e| e.name.name == "timeout")
        .unwrap();
    match &timeout.value {
        Literal::Duration(d) => {
            assert_eq!(d.value, 250);
            assert_eq!(d.value * d.unit.to_ms(), 250);
        }
        other => panic!("expected Duration, got {:?}", other),
    }
}

#[test]
fn program_without_sandbox_blocks_still_parses() {
    // regression: the absence of these blocks must change nothing
    let src = "agent T\n\nfn main() -> int { return 0 }\n";
    let program = parse(src).expect("parse should succeed");
    assert!(program
        .items
        .iter()
        .all(|i| !matches!(i, TopLevelItem::Permissions(_) | TopLevelItem::Limits(_))));
}
