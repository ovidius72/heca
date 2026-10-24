use super::*;

#[test]
fn an_embedding_adds_scoped_sound_assets_through_its_extension() {
    QUEUE.with(StartupQueue::reset);
    ADDED.with(|added| added.borrow_mut().clear());
    let demo = crate::extension("sound-example");
    demo.sound("ready", Audio::from_bytes(b"example audio"))
        .sound("file", Audio::load("/package/attention.mp3"));
    let library = super::super::library();
    assert!(library.names().any(|name| name == "sound-example.ready"));
    assert_eq!(library.resolve(&super::super::SoundSource::Asset("sound-example.file".into())).unwrap(),
        Audio::load("/package/attention.mp3"));
    assert_eq!(super::super::library().names().collect::<Vec<_>>(), library.names().collect::<Vec<_>>());
}
