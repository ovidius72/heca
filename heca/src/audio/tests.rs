use super::*;

#[test]
fn every_bundled_sound_decodes_from_embedded_bytes_without_filesystem_paths() {
    let library = assets::library();
    assert_eq!(library.names().collect::<Vec<_>>(), ["attention", "completed", "error", "locked", "success"]);
    for name in library.names() {
        let audio = library.resolve(&SoundSource::Asset(name.into())).unwrap();
        assert!(matches!(audio, Audio::Bytes(_)));
        let samples: Vec<_> = audio.decode().unwrap().collect();
        assert!(!samples.is_empty(), "empty audio: {name}");
        assert!(samples.iter().all(|sample| sample.is_finite()), "invalid samples: {name}");
    }
}

#[test]
fn file_and_embedded_sources_use_the_same_decoder() {
    let source = assets::library().resolve(&SoundSource::Asset("completed".into())).unwrap();
    let Audio::Bytes(bytes) = &source else { panic!("asset is not embedded") };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("custom.mp3");
    std::fs::write(&path, bytes).unwrap();
    let file = SoundLibrary::new().resolve(&SoundSource::File(path)).unwrap();
    assert_eq!(file.decode().unwrap().collect::<Vec<_>>(), source.decode().unwrap().collect::<Vec<_>>());
    assert!(Audio::from_bytes(b"invalid audio").decode().is_err());
}

#[test]
fn an_open_catalog_accepts_another_asset_without_a_status_or_program_branch() {
    let audio = assets::library().resolve(&SoundSource::Asset("success".into())).unwrap();
    let library = SoundLibrary::new().sound("custom.review-ready", audio.clone());
    assert_eq!(library.resolve(&SoundSource::Asset("custom.review-ready".into())).unwrap(), audio);
    assert!(library.resolve(&SoundSource::Asset("missing".into())).is_err());
}
