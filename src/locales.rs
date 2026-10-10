use git_agent_plugin_sdk::{Dictionary, text_key};
use std::{collections::BTreeMap, sync::OnceLock};

pub(super) fn dictionary() -> &'static Dictionary {
    static CATALOG: OnceLock<Dictionary> = OnceLock::new();
    CATALOG.get_or_init(|| {
        Dictionary(BTreeMap::from([
            (
                "zh".into(),
                serde_json::from_str(include_str!("../locales/zh.json"))
                    .expect("valid Chinese dictionary"),
            ),
            (
                "en".into(),
                serde_json::from_str(include_str!("../locales/en.json"))
                    .expect("valid English dictionary"),
            ),
        ]))
    })
}
pub(super) fn reference(zh: &str, en: &str) -> Option<String> {
    static INDEX: OnceLock<BTreeMap<(String, String), String>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        dictionary().0["en"]
            .iter()
            .filter_map(|(key, en)| {
                dictionary().0["zh"]
                    .get(key)
                    .map(|zh| ((zh.clone(), en.clone()), key.clone()))
            })
            .collect()
    });
    index
        .get(&(zh.into(), en.into()))
        .map(|key| text_key(key))
        .or_else(|| {
            dictionary().0["en"]
                .iter()
                .find(|(key, value)| key.starts_with("ui.") && value.as_str() == en)
                .map(|(key, _)| text_key(key))
        })
}
pub(super) fn value_label(value: &str) -> Option<String> {
    let key = format!("value.{value}");
    dictionary().0["en"]
        .contains_key(&key)
        .then(|| text_key(&key))
}
