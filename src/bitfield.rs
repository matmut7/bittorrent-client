pub fn bitfield_has_piece(bitfield: &Vec<u8>, index: usize) -> bool {
    let byte_index = index / 8;
    let offset = index % 8;

    if byte_index >= bitfield.len() {
        return false;
    }

    bitfield[byte_index] >> (7 - offset) & 1 != 0
}

pub fn bitfield_set_piece(bitfield: &mut Vec<u8>, index: usize) {
    let byte_index = index / 8;
    let offset = index % 8;

    if byte_index >= bitfield.len() {
        return;
    }

    bitfield[byte_index] |= 1 << (7 - offset);
}
