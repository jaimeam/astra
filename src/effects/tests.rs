use super::*;

#[test]
fn test_pure_effect_set() {
    let set = EffectSet::new();
    assert!(set.is_pure());
}

#[test]
fn test_effect_set_from_names() {
    let set = EffectSet::from_names(&["Net".to_string(), "Console".to_string()]);
    assert!(!set.is_pure());
    assert!(set.has(&Effect::Net));
    assert!(set.has(&Effect::Console));
    assert!(!set.has(&Effect::Fs));
}

#[test]
fn test_effect_subset() {
    let mut small = EffectSet::new();
    small.add(Effect::Net);

    let mut large = EffectSet::new();
    large.add(Effect::Net);
    large.add(Effect::Console);

    assert!(small.is_subset_of(&large));
    assert!(!large.is_subset_of(&small));
}
