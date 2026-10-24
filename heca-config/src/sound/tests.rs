use super::*;

#[derive(Serialize, Deserialize)]
struct Settings { sound: Sound }

#[test]
fn filenames_and_properties_round_trip_through_one_sound_type() {
    for (text, expected) in [
        ("sound = '/my/finished.mp3'", Sound::load("/my/finished.mp3")),
        ("sound = { source = 'asset:completed', enabled = false, volume = 0.4 }",
            Sound::asset("completed").enabled(false).volume(0.4)),
        ("sound = { source = '/my/error.mp3', volume = 0.2 }", Sound::load("/my/error.mp3").volume(0.2)),
    ] {
        let settings: Settings = toml::from_str(text).unwrap();
        assert_eq!(settings.sound, expected);
        let roundtrip: Settings = toml::from_str(&toml::to_string(&settings).unwrap()).unwrap();
        assert_eq!(roundtrip.sound, expected);
    }
}

#[test]
fn invalid_sound_properties_are_reported_as_configuration_errors() {
    for text in [
        "sound = { source = 'asset:' }",
        "sound = { source = 'file:' }",
        "sound = { source = 'asset:completed', volume = 1.1 }",
        "sound = { source = 'asset:completed', volume = -0.1 }",
        "sound = { source = 'asset:completed', volume = nan }",
        "sound = { source = 'asset:completed', typo = true }",
    ] {
        assert!(toml::from_str::<Settings>(text).is_err(), "accepted {text}");
    }
}

#[test]
fn a_literal_file_with_an_asset_prefix_keeps_its_identity() {
    let expected = Sound::load("asset:my-filename.mp3");
    let text = toml::to_string(&Settings { sound: expected.clone() }).unwrap();
    assert_eq!(toml::from_str::<Settings>(&text).unwrap().sound, expected);
}
