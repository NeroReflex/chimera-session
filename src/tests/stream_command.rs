#[cfg(test)]
use bytevec2::*;

#[test]
fn test_stream_command() {
    bytevec_decl! {
        #[derive(Debug, Clone, PartialEq)]
        struct TestVec {
            data: Vec<u8>
        }
    }

    let test_data = TestVec {
        data: vec![0x00, 0x00, 0xFF, 0xCF],
    };

    let mut sc = crate::stream_command::StreamCommand::new(1024);

    let to_stream = sc.encode(test_data.clone()).unwrap();

    let decoded = sc.decode::<TestVec>(to_stream.as_slice()).unwrap();

    assert_eq!(decoded, vec![test_data]);
}
