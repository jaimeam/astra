use super::*;

#[test]
fn test_assert_eq() {
    assert!(assert::assert_eq(1, 1).is_ok());
    assert!(assert::assert_eq(1, 2).is_err());
}

#[test]
fn test_property_basic() {
    let gen = property::IntGenerator { min: 0, max: 100 };

    let result = property::run_property(&gen, 100, 42, |x| {
        if *x >= 0 && *x <= 100 {
            Ok(())
        } else {
            Err(format!("out of range: {}", x))
        }
    });

    assert!(result.is_ok());
}
