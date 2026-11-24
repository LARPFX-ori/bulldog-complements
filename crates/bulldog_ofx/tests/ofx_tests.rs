use bulldog_core::{EffectContext, EffectDescriptor, ParamDescriptor};
use bulldog_ofx::{validate_plugin, validate_suite};

fn valid_filter(id: &str) -> EffectDescriptor {
    EffectDescriptor {
        identifier: id.to_owned(),
        label: "Sample Filter".to_owned(),
        grouping: "Bulldog Tests".to_owned(),
        description: "Clean-room validation test filter.".to_owned(),
        version_major: 1,
        version_minor: 0,
        context: EffectContext::Filter,
        params: vec![ParamDescriptor::double(
            "amount",
            "Amount",
            "Effect strength",
            0.5,
            0.0,
            1.0,
        )],
    }
}

#[test]
fn validator_accepts_clean_bulldog_descriptor() {
    let desc = valid_filter("br.com.bulldog.complements.continuus.sample");
    assert!(validate_plugin(&desc).is_ok());
    assert!(validate_suite(&[desc]).is_ok());
}

#[test]
fn validator_rejects_wrong_prefix_and_duplicate_ids() {
    let bad_prefix = valid_filter("com.example.other.sample");
    assert!(validate_plugin(&bad_prefix).is_err());

    let d1 = valid_filter("br.com.bulldog.complements.continuus.dup");
    let d2 = valid_filter("br.com.bulldog.complements.continuus.dup");
    assert!(validate_suite(&[d1, d2]).is_err());
}

#[test]
fn validator_rejects_commercial_terms_and_missing_transition_param() {
    let mut bad_brand = valid_filter("br.com.bulldog.complements.continuus.test");
    bad_brand.label = "Sapphire Clone".to_owned();
    assert!(validate_plugin(&bad_brand).is_err());

    let mut bad_transition = valid_filter("br.com.bulldog.complements.transitions_3d.broken");
    bad_transition.context = EffectContext::Transition;
    assert!(validate_plugin(&bad_transition).is_err());
}
