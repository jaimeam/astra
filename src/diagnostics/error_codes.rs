//! Error code definitions and documentation

/// Syntax/parsing errors (E0xxx)
pub mod syntax {
    pub const UNEXPECTED_TOKEN: &str = "E0001";
    pub const UNTERMINATED_STRING: &str = "E0002";
    pub const INVALID_NUMBER: &str = "E0003";
    pub const MISSING_DELIMITER: &str = "E0004";
    pub const INVALID_IDENTIFIER: &str = "E0005";
    pub const RESERVED_KEYWORD: &str = "E0006";
    pub const INVALID_ESCAPE: &str = "E0007";
    pub const UNEXPECTED_EOF: &str = "E0008";
    pub const INVALID_MODULE: &str = "E0009";
    pub const DUPLICATE_MODULE: &str = "E0010";
}

/// Type errors (E1xxx)
pub mod types {
    pub const TYPE_MISMATCH: &str = "E1001";
    pub const UNKNOWN_IDENTIFIER: &str = "E1002";
    pub const MISSING_TYPE_ANNOTATION: &str = "E1003";
    pub const NON_EXHAUSTIVE_MATCH: &str = "E1004";
    pub const DUPLICATE_FIELD: &str = "E1005";
    pub const UNKNOWN_FIELD: &str = "E1006";
    pub const WRONG_ARGUMENT_COUNT: &str = "E1007";
    pub const CANNOT_INFER_TYPE: &str = "E1008";
    pub const RECURSIVE_TYPE: &str = "E1009";
    pub const INVALID_TYPE_APPLICATION: &str = "E1010";
    pub const DUPLICATE_TYPE: &str = "E1011";
    pub const UNKNOWN_TYPE: &str = "E1012";
    pub const EXPECTED_FUNCTION: &str = "E1013";
    pub const EXPECTED_RECORD: &str = "E1014";
    pub const EXPECTED_ENUM: &str = "E1015";
}

/// Effect errors (E2xxx)
pub mod effects {
    pub const EFFECT_NOT_DECLARED: &str = "E2001";
    pub const UNKNOWN_EFFECT: &str = "E2002";
    pub const CAPABILITY_NOT_AVAILABLE: &str = "E2003";
    pub const EFFECTFUL_IN_PURE: &str = "E2004";
    pub const EFFECT_MISMATCH: &str = "E2005";
    pub const EFFECT_NOT_MOCKABLE: &str = "E2006";
    pub const INVALID_CAPABILITY_INJECTION: &str = "E2007";
}

/// Contract errors (E3xxx)
pub mod contracts {
    pub const PRECONDITION_VIOLATION: &str = "E3001";
    pub const POSTCONDITION_VIOLATION: &str = "E3002";
    pub const INVARIANT_VIOLATION: &str = "E3003";
    pub const INVALID_CONTRACT_EXPR: &str = "E3004";
    pub const CONTRACT_BINDING_UNAVAILABLE: &str = "E3005";
}

/// Runtime errors (E4xxx)
pub mod runtime {
    pub const DIVISION_BY_ZERO: &str = "E4001";
    pub const INDEX_OUT_OF_BOUNDS: &str = "E4002";
    pub const CONTRACT_VIOLATION: &str = "E4003";
    pub const RESOURCE_LIMIT_EXCEEDED: &str = "E4004";
    pub const CAPABILITY_DENIED: &str = "E4005";
    pub const INTEGER_OVERFLOW: &str = "E4006";
    pub const STACK_OVERFLOW: &str = "E4007";
    pub const ASSERTION_FAILED: &str = "E4008";
}

/// Warnings (W0xxx)
pub mod warnings {
    pub const UNUSED_VARIABLE: &str = "W0001";
    pub const UNUSED_IMPORT: &str = "W0002";
    pub const UNREACHABLE_CODE: &str = "W0003";
    pub const DEPRECATED: &str = "W0004";
    pub const WILDCARD_MATCH: &str = "W0005";
    pub const SHADOWED_BINDING: &str = "W0006";
    pub const REDUNDANT_TYPE_ANNOTATION: &str = "W0007";
}
