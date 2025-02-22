#[cfg(test)]
mod tests {
    use hc_homie5::*;

    // For these tests we use String for both FROM and TO.
    // It is assumed that String implements ValueMatcher appropriately.

    #[test]
    fn test_value_mapping_no_condition() {
        // When no condition is provided, the mapping should always apply.
        let mapping: ValueMapping<String, String> = ValueMapping {
            from: None,
            to: "mapped".to_string(),
        };
        let input = "anything".to_string();
        let result = mapping.map_to(&input);
        // Should always be mapped, regardless of input.
        assert!(result.is_mapped());
        if let MappingResult::Mapped(mapped) = result {
            assert_eq!(mapped, "mapped");
        }
    }

    #[test]
    fn test_value_mapping_with_condition() {
        // Create a mapping that only applies if the input equals "match".
        let mapping: ValueMapping<String, String> = ValueMapping {
            from: Some(ValueCondition::Value("match".to_string())),
            to: "mapped".to_string(),
        };

        let vm = "match".to_string();
        let result_match = mapping.map_to(&vm);
        assert!(result_match.is_mapped());
        if let MappingResult::Mapped(mapped) = result_match {
            assert_eq!(mapped, "mapped");
        }

        let vm = "no match".to_string();
        let result_no_match = mapping.map_to(&vm);
        assert!(!result_no_match.is_mapped());
        if let MappingResult::Unmapped(unmapped) = result_no_match {
            assert_eq!(unmapped, "no match");
        }
    }

    #[test]
    fn test_value_mapping_list() {
        // Create two mappings:
        //  - First mapping applies if the input equals "a" and maps to "first".
        //  - Second mapping applies if the input equals "b" and maps to "second".
        let mapping1: ValueMapping<String, String> = ValueMapping {
            from: Some(ValueCondition::Value("a".to_string())),
            to: "first".to_string(),
        };
        let mapping2: ValueMapping<String, String> = ValueMapping {
            from: Some(ValueCondition::Value("b".to_string())),
            to: "second".to_string(),
        };
        let mapping_list = ValueMappingList(vec![mapping1, mapping2]);

        // For input "a", the first mapping should match.
        let vm = "a".to_string();
        let result_a = mapping_list.map_to(&vm);
        assert!(result_a.is_mapped());
        if let MappingResult::Mapped(mapped) = result_a {
            assert_eq!(mapped, "first");
        }

        // For input "b", the second mapping should match.
        let vm = "b".to_string();
        let result_b = mapping_list.map_to(&vm);
        assert!(result_b.is_mapped());
        if let MappingResult::Mapped(mapped) = result_b {
            assert_eq!(mapped, "second");
        }

        // For input "c", no mapping applies.
        let vm = "c".to_string();
        let result_c = mapping_list.map_to(&vm);
        assert!(!result_c.is_mapped());
        if let MappingResult::Unmapped(unmapped) = result_c {
            assert_eq!(unmapped, "c");
        }
    }

    #[test]
    fn test_value_mapping_io_output() {
        // In the output mapping, the type parameters are:
        //   IN: the input type to be mapped (here, String)
        //   OUT: the mapped (output) type (also String)
        // We create an output mapping that maps "hello" to "world".
        let output_mapping: ValueMapping<String, String> = ValueMapping {
            from: Some(ValueCondition::Value("hello".to_string())),
            to: "world".to_string(),
        };
        let mapping_list = ValueMappingList(vec![output_mapping]);
        let mapping_io: ValueMappingIO<String, String> = ValueMappingIO {
            input: ValueMappingList::default(), // empty input mapping list
            output: mapping_list,
        };

        // When the input ("hello") satisfies the mapping condition, we should get "world".
        let vm = "hello".to_string();
        let result_match = mapping_io.map_ouput(&vm);
        assert!(result_match.is_mapped());
        if let MappingResult::Mapped(mapped) = result_match {
            assert_eq!(mapped, "world");
        }

        // For a non-matching value, we expect an unmapped result.
        let vm = "not hello".to_string();
        let result_no_match = mapping_io.map_ouput(&vm);
        assert!(!result_no_match.is_mapped());
        if let MappingResult::Unmapped(unmapped) = result_no_match {
            assert_eq!(unmapped, "not hello");
        }
    }

    #[test]
    fn test_value_mapping_io_input() {
        // In the input mapping, the type parameters are:
        //   OUT: the output type from mapping (here, String)
        //   IN: the input type (also String)
        // We create an input mapping that maps "foo" to "bar".
        let input_mapping: ValueMapping<String, String> = ValueMapping {
            from: Some(ValueCondition::Value("foo".to_string())),
            to: "bar".to_string(),
        };
        let mapping_list = ValueMappingList(vec![input_mapping]);
        let mapping_io: ValueMappingIO<String, String> = ValueMappingIO {
            input: mapping_list,
            output: ValueMappingList::default(), // empty output mapping list
        };

        // When the condition is satisfied, mapping should occur.
        let vm = "foo".to_string();
        let result_match = mapping_io.map_input(&vm);
        assert!(result_match.is_mapped());
        if let MappingResult::Mapped(mapped) = result_match {
            assert_eq!(mapped, "bar");
        }

        // When not satisfied, the result is unmapped.
        let vm = "baz".to_string();
        let result_no_match = mapping_io.map_input(&vm);
        assert!(!result_no_match.is_mapped());
        if let MappingResult::Unmapped(unmapped) = result_no_match {
            assert_eq!(unmapped, "baz");
        }
    }
}
