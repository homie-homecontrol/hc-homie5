#[cfg(test)]
mod tests {
    use hc_homie5::*;

    // Test that a literal condition uses the default literal matching.
    #[test]
    fn test_evaluate_literal_string() {
        let condition = ValueCondition::Value("test".to_string());
        let value = "test".to_string();
        assert!(condition.evaluate(&value));

        let value2 = "not test".to_string();
        assert!(!condition.evaluate(&value2));
    }

    // Test that a pattern condition uses regex matching.
    #[test]
    fn test_evaluate_pattern_string() {
        let condition = ValueCondition::Pattern(Pattern {
            pattern: "^te.*".to_string(),
        });
        let value = "test".to_string();
        assert!(condition.evaluate(&value));

        let value2 = "atest".to_string();
        // "^te.*" does not match "atest"
        assert!(!condition.evaluate(&value2));
    }

    // Test an operator condition using the Equal operator.
    #[test]
    fn test_evaluate_operator_equal_string() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::Equal,
            value: Some(ValueSet::Single("equal".to_string())),
        };
        let condition = ValueCondition::Operator(operator_condition);

        let value = "equal".to_string();
        assert!(condition.evaluate(&value));

        let value2 = "different".to_string();
        assert!(!condition.evaluate(&value2));
    }

    // Test an operator condition using a lexicographical comparison (Greater).
    #[test]
    fn test_evaluate_operator_greater_string() {
        // Assuming the default `matches` implementation for String performs lexicographical comparison.
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::Greater,
            value: Some(ValueSet::Single("apple".to_string())),
        };
        let condition = ValueCondition::Operator(operator_condition);

        let value = "banana".to_string(); // "banana" > "apple"
        assert!(condition.evaluate(&value));

        let value2 = "aardvark".to_string(); // "aardvark" < "apple"
        assert!(!condition.evaluate(&value2));
    }

    // Test evaluate_option for a literal condition.
    #[test]
    fn test_evaluate_option_literal_string() {
        let condition = ValueCondition::Value("option".to_string());
        let value = Some("option".to_string());
        assert!(condition.evaluate_option(value.as_ref()));

        let none_value: Option<&String> = None;
        assert!(!condition.evaluate_option(none_value));
    }

    // Test the value() helper method for all variants.
    #[test]
    fn test_value_method_string() {
        // For a literal condition, the inner value is returned.
        let literal_condition = ValueCondition::Value("literal".to_string());
        assert_eq!(literal_condition.value(), Some(&"literal".to_string()));

        // For an operator condition wrapping a single value,
        // value() should recursively return that inner value.
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::Equal,
            value: Some(ValueSet::Single("op_value".to_string())),
        };
        let condition = ValueCondition::Operator(operator_condition);
        assert_eq!(condition.value(), Some(&"op_value".to_string()));

        // For a pattern condition, value() should return None.
        let pattern_condition = ValueCondition::<String>::Pattern(Pattern {
            pattern: ".*".to_string(),
        });
        assert!(pattern_condition.value().is_none());
    }

    // Test deserialization of a literal condition.
    #[test]
    fn test_deserialize_literal_condition_string() {
        // A literal condition is untagged so that a JSON string deserializes into ValueCondition::Value.
        let yaml = "\"literal_value\"";
        let condition: ValueCondition<String> = serde_yml::from_str(yaml).unwrap();
        if let ValueCondition::Value(val) = condition {
            assert_eq!(val, "literal_value".to_string());
        } else {
            panic!("Expected literal variant");
        }
    }

    // Test deserialization of an operator condition.
    #[test]
    fn test_deserialize_operator_condition_string() {
        let yaml = r#"{
            "operator": "=",
            "value": "test"
        }"#;
        let condition: ValueCondition<String> = serde_yml::from_str(yaml).unwrap();
        if let ValueCondition::Operator(op_condition) = condition {
            assert_eq!(op_condition.operator, ConditionOperator::Equal);
            if let Some(ValueSet::Single(val)) = op_condition.value {
                assert_eq!(val, "test".to_string());
            } else {
                panic!("Expected a single value in operator condition");
            }
        } else {
            panic!("Expected operator variant");
        }
    }

    // Test deserialization of a pattern condition.
    #[test]
    fn test_deserialize_pattern_condition_string() {
        let yaml = r#"{
            "pattern": "regex_pattern"
        }"#;
        let condition: ValueCondition<String> = serde_yml::from_str(yaml).unwrap();
        if let ValueCondition::Pattern(pattern) = condition {
            assert_eq!(pattern.pattern, "regex_pattern".to_string());
        } else {
            panic!("Expected pattern variant");
        }
    }

    // --- String-based tests ---

    #[test]
    fn test_evaluate_operator_not_equal_string() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::NotEqual,
            value: Some(ValueSet::Single("abc".to_string())),
        };
        let condition = ValueCondition::Operator(operator_condition);

        let value = "abc".to_string();
        assert!(!condition.evaluate(&value)); // equal, so NotEqual should be false

        let value2 = "def".to_string();
        assert!(condition.evaluate(&value2)); // not equal, so should be true
    }

    #[test]
    fn test_evaluate_operator_includes_any_string() {
        // For IncludesAny assume the implementation checks if the tested string contains any of the substrings.
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::IncludesAny,
            value: Some(ValueSet::Multiple(vec![
                "world".to_string(),
                "foo".to_string(),
            ])),
        };
        let condition = ValueCondition::Operator(operator_condition);

        let value_contains = "world".to_string();
        let value_not_contains = "hello".to_string();

        assert!(condition.evaluate(&value_contains));
        assert!(!condition.evaluate(&value_not_contains));
    }

    #[test]
    fn test_evaluate_operator_includes_none_string() {
        // For IncludesNone assume the implementation checks if the tested string does not contain any of the substrings.
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::IncludesNone,
            value: Some(ValueSet::Multiple(vec![
                "world".to_string(),
                "foo".to_string(),
            ])),
        };
        let condition = ValueCondition::Operator(operator_condition);

        let value_without = "hello".to_string();
        let value_with = "world".to_string();

        assert!(condition.evaluate(&value_without));
        assert!(!condition.evaluate(&value_with));
    }

    #[test]
    fn test_evaluate_operator_match_always_string() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::MatchAlways,
            value: None, // Value is irrelevant for MatchAlways.
        };
        let condition = ValueCondition::Operator(operator_condition);

        let any_value = "anything".to_string();
        assert!(condition.evaluate(&any_value));
    }

    #[test]
    fn test_evaluate_operator_is_empty_string() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::IsEmpty,
            value: None,
        };
        let condition = ValueCondition::Operator(operator_condition);

        // For evaluate_option: when no value is provided, IsEmpty should yield true.
        assert!(condition.evaluate_option(None));

        // When a value is provided, IsEmpty should yield false.
        let some_value = "non-empty".to_string();
        assert!(!condition.evaluate_option(Some(&some_value)));
    }

    #[test]
    fn test_evaluate_operator_exists_string() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::Exists,
            value: None,
        };
        let condition = ValueCondition::Operator(operator_condition);

        // When a value is provided, Exists should yield true.
        let some_value = "non-empty".to_string();
        assert!(condition.evaluate_option(Some(&some_value)));

        // When no value is provided, Exists should yield false.
        assert!(!condition.evaluate_option(None));
    }

    // --- i64-based operator tests ---

    #[test]
    fn test_operator_greater_i64() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::Greater,
            value: Some(ValueSet::Single(5)),
        };
        let condition = ValueCondition::Operator(operator_condition);
        // 10 is greater than 5.
        assert!(condition.evaluate(&10));
        // 3 is not greater than 5.
        assert!(!condition.evaluate(&3));
    }

    #[test]
    fn test_operator_less_i64() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::Less,
            value: Some(ValueSet::Single(10)),
        };
        let condition = ValueCondition::Operator(operator_condition);
        // 5 is less than 10.
        assert!(condition.evaluate(&5));
        // 15 is not less than 10.
        assert!(!condition.evaluate(&15));
    }

    #[test]
    fn test_operator_greater_or_equal_i64() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::GreaterOrEqual,
            value: Some(ValueSet::Single(5)),
        };
        let condition = ValueCondition::Operator(operator_condition);
        // 5 is equal to 5.
        assert!(condition.evaluate(&5));
        // 10 is greater than 5.
        assert!(condition.evaluate(&10));
        // 3 is less than 5.
        assert!(!condition.evaluate(&3));
    }

    #[test]
    fn test_operator_less_or_equal_i64() {
        let operator_condition = ValueOperatorCondition {
            operator: ConditionOperator::LessOrEqual,
            value: Some(ValueSet::Single(10)),
        };
        let condition = ValueCondition::Operator(operator_condition);
        // 10 is equal to 10.
        assert!(condition.evaluate(&10));
        // 5 is less than 10.
        assert!(condition.evaluate(&5));
        // 15 is greater than 10.
        assert!(!condition.evaluate(&15));
    }
}
