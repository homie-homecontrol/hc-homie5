use std::cell::Cell;

use hc_homie5::value::ConditionTree;
use serde::{Deserialize, Serialize};

/// A strict map leaf: `{ is: true }` holds when its flag is set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Is {
    is: bool,
}

/// An untagged leaf like automation's `WhileCondition`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Either {
    Flag { flag: bool },
    Number { number: i64 },
}

fn parse<L: for<'de> Deserialize<'de>>(yaml: &str) -> ConditionTree<L> {
    serde_yaml_ng::from_str(yaml).unwrap_or_else(|e| panic!("{yaml}: {e}"))
}

fn holds(tree: &ConditionTree<Is>) -> bool {
    tree.evaluate(|leaf| leaf.is)
}

#[test]
fn a_plain_leaf_is_a_leaf() {
    let tree: ConditionTree<Is> = parse("is: true");
    assert_eq!(tree, ConditionTree::Leaf(Is { is: true }));
    assert!(holds(&tree));
}

#[test]
fn all_any_and_not_combine_leaves() {
    assert!(holds(&parse("all: [{ is: true }, { is: true }]")));
    assert!(!holds(&parse("all: [{ is: true }, { is: false }]")));
    assert!(holds(&parse("any: [{ is: false }, { is: true }]")));
    assert!(!holds(&parse("any: [{ is: false }, { is: false }]")));
    assert!(holds(&parse("not: { is: false }")));
    assert!(!holds(&parse("not: { is: true }")));
}

#[test]
fn empty_all_holds_and_empty_any_does_not() {
    assert!(holds(&parse("all: []")));
    assert!(!holds(&parse("any: []")));
}

#[test]
fn a_list_is_shorthand_for_all() {
    let tree: ConditionTree<Is> = parse("[{ is: true }, { is: false }]");
    assert_eq!(
        tree,
        ConditionTree::All(vec![Is { is: true }.into(), Is { is: false }.into()])
    );
}

#[test]
fn trees_nest() {
    let yaml = r#"
all:
  - is: true
  - any:
      - is: false
      - not:
          is: false
"#;
    assert!(holds(&parse(yaml)));
}

#[test]
fn evaluation_stops_once_the_result_is_known() {
    let tree: ConditionTree<Is> = parse("any: [{ is: true }, { is: false }]");
    let calls = Cell::new(0);
    assert!(tree.evaluate(|leaf| {
        calls.set(calls.get() + 1);
        leaf.is
    }));
    assert_eq!(calls.get(), 1);
}

#[test]
fn untagged_leaves_are_parsed_after_the_first_key_was_read() {
    let tree: ConditionTree<Either> = parse("any: [{ flag: true }, { number: 3 }]");
    assert_eq!(
        tree.leaves(),
        vec![&Either::Flag { flag: true }, &Either::Number { number: 3 }]
    );
}

#[test]
fn a_typo_in_a_leaf_reports_the_leaf_error() {
    let err = serde_yaml_ng::from_str::<ConditionTree<Is>>("all: [{ iss: true }]")
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown field `iss`"), "{err}");
}

#[test]
fn a_combinator_cannot_carry_other_fields() {
    let err = serde_yaml_ng::from_str::<ConditionTree<Is>>("not: { is: true }\nis: true")
        .unwrap_err()
        .to_string();
    assert!(err.contains("`not` cannot be combined"), "{err}");
}

#[test]
fn trees_round_trip_through_json() {
    let tree: ConditionTree<Is> = parse("all: [{ is: true }, { not: { any: [{ is: false }] } }]");
    let json = serde_json::to_string(&tree).unwrap();
    assert_eq!(
        json,
        r#"{"all":[{"is":true},{"not":{"any":[{"is":false}]}}]}"#
    );
    assert_eq!(
        serde_json::from_str::<ConditionTree<Is>>(&json).unwrap(),
        tree
    );
}

#[test]
fn map_and_try_map_convert_leaves() {
    let tree: ConditionTree<Is> = parse("any: [{ is: true }, { not: { is: false } }]");
    let flags = tree.clone().map(|leaf| leaf.is);
    assert!(flags.evaluate(|flag| *flag));

    let failed: Result<ConditionTree<bool>, String> = tree.try_map(|leaf| {
        if leaf.is {
            Ok(true)
        } else {
            Err("false leaf".into())
        }
    });
    assert_eq!(failed, Err("false leaf".to_string()));
}

#[test]
fn the_schema_is_recursive() {
    #[derive(schemars::JsonSchema)]
    #[allow(dead_code)]
    struct Leaf {
        is: bool,
    }
    let schema = schemars::schema_for!(ConditionTree<Leaf>);
    let json = serde_json::to_string(&schema).unwrap();
    assert!(json.contains("ConditionTree_Leaf"), "{json}");
    assert!(json.contains("\"all\""), "{json}");
}
