use super::*;
use crate::server::wire::ClientMsg;

fn limit() -> NonZeroUsize {
    NonZeroUsize::new(1024).unwrap()
}

#[test]
fn successive_frames_remain_separate() {
    let mut bytes = Vec::new();
    write(&mut bytes, &ClientMsg::Attach, limit()).unwrap();
    write(&mut bytes, &ClientMsg::Detach, limit()).unwrap();
    let mut input = bytes.as_slice();
    assert!(matches!(read(&mut input, limit()).unwrap(), Some(ClientMsg::Attach)));
    assert!(matches!(read(&mut input, limit()).unwrap(), Some(ClientMsg::Detach)));
    assert!(read::<ClientMsg>(&mut input, limit()).unwrap().is_none());
}

#[test]
fn truncated_frames_are_not_clean_disconnects() {
    let mut bytes = Vec::new();
    write(&mut bytes, &ClientMsg::Attach, limit()).unwrap();
    for end in 1..bytes.len() {
        let error = read::<ClientMsg>(&mut &bytes[..end], limit()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}

#[test]
fn lengths_are_checked_before_reading_a_payload() {
    for length in [0, 1025, u32::MAX] {
        let bytes = length.to_be_bytes();
        assert_eq!(read::<ClientMsg>(&mut bytes.as_slice(), limit()).unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}

#[test]
fn invalid_json_is_rejected() {
    let mut bytes = 1u32.to_be_bytes().to_vec();
    bytes.push(b'{');
    assert_eq!(read::<ClientMsg>(&mut bytes.as_slice(), limit()).unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn an_outgoing_oversized_frame_writes_nothing() {
    let mut bytes = Vec::new();
    assert!(write(&mut bytes, &ClientMsg::Attach, NonZeroUsize::new(1).unwrap()).is_err());
    assert!(bytes.is_empty());
}
