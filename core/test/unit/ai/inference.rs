use super::*;

#[test]
fn stt_detects_its_audio_format_from_bytes() {
    let prepare = |audio: &[u8]| {
        Input::Stt {
            audio_base64: STANDARD.encode(audio),
        }
        .prepare(AiSafety::Strict)
    };
    let cases: [(&[u8], &str); 4] = [
        (b"RIFF\x24\0\0\0WAVEfmt ", "wav"),
        (b"OggS\0\x02", "ogg"),
        (b"ID3\x04\0\0", "mp3"),
        (b"\xff\xfb\x90\x64", "mp3"),
    ];
    for (audio, format) in cases {
        assert_eq!(prepare(audio).unwrap().audit_input()["format"], format);
    }
    // FLAC and AAC ADTS, whose frame sync resembles MP3.
    for audio in [&b"fLaC\0\0\0\x22"[..], b"\xff\xf1\x50\x80"] {
        assert!(matches!(
            prepare(audio),
            Err(AiError::InvalidInference(
                "unsupported audio format; send WAV, MP3 or Ogg"
            ))
        ));
    }
}
