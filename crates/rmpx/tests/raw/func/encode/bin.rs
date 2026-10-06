use crate::msgpack::Marker;
use crate::msgpack::encode::*;

#[test]
fn pass_pack_len_u8() {
    let mut buf = [0x00, 0x00];

    assert_eq!(Marker::Bin8, write_bin_len(&mut &mut buf[..], 255).unwrap());

    assert_eq!([0xc4, 0xff], buf);
}

#[test]
fn pass_pack_len_u16() {
    let mut buf = [0x00, 0x00, 0x00];

    assert_eq!(Marker::Bin16, write_bin_len(&mut &mut buf[..], 65535).unwrap());

    assert_eq!([0xc5, 0xff, 0xff], buf);
}

#[test]
fn pass_pack_len_u32() {
    let mut buf = [0x00, 0x00, 0x00, 0x00, 0x00];

    assert_eq!(Marker::Bin32, write_bin_len(&mut &mut buf[..], 4294967295).unwrap());

    assert_eq!([0xc6, 0xff, 0xff, 0xff, 0xff], buf);
}

// A zeroed buffer this large is mapped lazily, the length check fails before any byte is touched.
#[cfg(target_pointer_width = "64")]
#[test]
#[ignore = "maps 4 GiB of address space, too much for strictly overcommitting hosts"]
fn fail_pack_bin_longer_than_u32() {
    let data = vec![0u8; u32::MAX as usize + 1];
    let mut buf = Vec::new();
    assert!(matches!(write_bin(&mut buf, &data), Err(ValueWriteError::OutOfRange)));
    assert!(buf.is_empty());
}
