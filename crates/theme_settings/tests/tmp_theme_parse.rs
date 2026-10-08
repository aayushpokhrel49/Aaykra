use theme_settings::{ThemeFamilyContent, refine_theme_family};

#[test]
fn tmp_parse_bundled_aaykra_theme() {
    let json = std::fs::read("assets/themes/aaykra/aaykra.json").unwrap();
    let family: ThemeFamilyContent = serde_json_lenient::from_slice(&json).unwrap();
    assert_eq!(family.themes.len(), 2);
    let refined = refine_theme_family(family);
    assert_eq!(refined.themes.len(), 2);
    assert!(refined.themes.iter().any(|t| t.name == "Aaykra"));
    assert!(refined.themes.iter().any(|t| t.name == "Aaykra Light"));
}
