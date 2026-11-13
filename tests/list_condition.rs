use hc_homie5::value::{ListCondition, ValueCondition};

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn parse(yaml: &str) -> ListCondition<String> {
    serde_yaml_ng::from_str(yaml).unwrap()
}

#[test]
fn any_matches_if_one_element_matches() {
    let cond = parse("any: b");
    assert!(cond.evaluate(&strings(&["a", "b"])));
    assert!(!cond.evaluate(&strings(&["a", "c"])));
    assert!(!cond.evaluate(&[]));

    let cond = parse("any: { operator: \"=\", value: [b, c] }");
    assert!(cond.evaluate(&strings(&["a", "c"])));
    assert!(!cond.evaluate(&strings(&["a", "d"])));
}

#[test]
fn all_matches_if_every_element_matches_and_holds_for_empty() {
    let cond = parse("all: { pattern: '^x-' }");
    assert!(cond.evaluate(&strings(&["x-1", "x-2"])));
    assert!(!cond.evaluate(&strings(&["x-1", "y-2"])));
    assert!(cond.evaluate(&[]));
}

#[test]
fn none_matches_if_no_element_matches_and_holds_for_empty() {
    let cond = parse("none: { operator: includesAny, value: [legacy, deprecated] }");
    assert!(cond.evaluate(&strings(&["mqtt"])));
    assert!(!cond.evaluate(&strings(&["mqtt", "legacy"])));
    assert!(cond.evaluate(&[]));
}

#[test]
fn first_matches_the_first_element_only() {
    let cond = parse("first: a");
    assert!(cond.evaluate(&strings(&["a", "b"])));
    assert!(!cond.evaluate(&strings(&["b", "a"])));
    assert!(!cond.evaluate(&[]));
}

#[test]
fn empty_checks_list_length() {
    assert!(parse("empty: true").evaluate(&[]));
    assert!(!parse("empty: true").evaluate(&strings(&["a"])));
    assert!(parse("empty: false").evaluate(&strings(&["a"])));
    assert!(!parse("empty: false").evaluate(&[]));
}

#[test]
fn missing_list_counts_as_empty() {
    assert!(!parse("any: a").evaluate_option(None));
    assert!(!parse("first: a").evaluate_option(None));
    assert!(parse("all: a").evaluate_option(None));
    assert!(parse("none: a").evaluate_option(None));
    assert!(parse("empty: true").evaluate_option(None));
    assert!(parse("any: a").evaluate_option(Some(&strings(&["a"]))));
}

#[test]
fn works_for_non_string_elements() {
    let cond: ListCondition<i64> =
        serde_yaml_ng::from_str("all: { operator: between, value: [1, 5] }").unwrap();
    assert!(cond.evaluate(&[1, 3, 5]));
    assert!(!cond.evaluate(&[1, 6]));
}

#[test]
fn deserializes_from_text_and_from_yaml_tree() {
    for yaml in [
        "any: { operator: \">=\", value: b }",
        "all: { pattern: '^[a-z]$' }",
        "none: z",
        "first: { operator: \"=\", value: [a, q] }",
        "empty: false",
    ] {
        let from_text = parse(yaml);
        let tree: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
        let from_tree: ListCondition<String> = serde_yaml_ng::from_value(tree).unwrap();
        let values = strings(&["a", "c"]);
        assert!(from_text.evaluate(&values), "{yaml}");
        assert!(from_tree.evaluate(&values), "{yaml}");
    }
}

#[test]
fn serializes_as_single_key_mapping() {
    let cond: ListCondition<String> = ListCondition::Any(ValueCondition::Value("a".to_string()));
    assert_eq!(
        serde_json::to_value(&cond).unwrap(),
        serde_json::json!({ "any": "a" })
    );
    assert_eq!(
        serde_json::to_value(ListCondition::<String>::Empty(true)).unwrap(),
        serde_json::json!({ "empty": true })
    );
    let yaml = serde_yaml_ng::to_string(&ListCondition::<String>::First(ValueCondition::Value(
        "a".into(),
    )))
    .unwrap();
    assert_eq!(yaml, "first: a\n");
    assert!(parse(&yaml).evaluate(&strings(&["a"])));
}

#[test]
fn rejects_bare_conditions_unknown_keys_and_multiple_keys() {
    for yaml in [
        "a",
        "{ operator: \"=\", value: a }",
        "{ pattern: '^a' }",
        "{ some: a }",
        "{ any: a, all: b }",
        "empty: maybe",
    ] {
        let err = serde_yaml_ng::from_str::<ListCondition<String>>(yaml)
            .unwrap_err()
            .to_string();
        let tree: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(
            serde_yaml_ng::from_value::<ListCondition<String>>(tree).is_err(),
            "{yaml}"
        );
        println!("{yaml:?}: {err}");
    }
    let err = serde_yaml_ng::from_str::<ListCondition<String>>("{ operator: \"=\", value: a }")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("bare value condition") && err.contains("any"),
        "{err}"
    );
}

#[test]
fn schema_is_one_of_single_key_objects() {
    let schema = serde_json::to_value(schemars::schema_for!(ListCondition<String>)).unwrap();
    let variants = schema["oneOf"].as_array().expect("oneOf");
    assert_eq!(variants.len(), 5);
    for v in variants {
        assert_eq!(v["additionalProperties"], serde_json::json!(false), "{v}");
        assert_eq!(v["required"].as_array().map(Vec::len), Some(1), "{v}");
    }
}
