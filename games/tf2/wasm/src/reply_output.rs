//! Immutable transport ownership of the already-encoded authoritative outputs.
//! A lease survives map disposal; returning it to a stale owner only frees it.

use super::{decode, simulation_hosts, slots};

fn with_output<T>(handle: u32, kind: u32, operation: impl FnOnce(&mut Vec<u8>) -> T) -> Option<T> {
    if kind == 1 {
        let mut hosts = simulation_hosts().lock().expect("TF2 Simulation hosts");
        return hosts.get_mut(&handle).map(|entry| operation(&mut entry.output));
    }
    let (index, generation) = decode(handle)?;
    let mut slots = slots().lock().expect("TF2 slots");
    let slot = slots.get_mut(index)?;
    if slot.generation != generation { return None; }
    let output = match kind {
        2 => &mut slot.particle_output,
        4 => &mut slot.visibility_output,
        _ => return None,
    };
    Some(operation(output))
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_reply_output_capacity(handle: u32, kind: u32) -> usize {
    with_output(handle, kind, |output| output.capacity()).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_reply_output_take(handle: u32, kind: u32) -> *mut u8 {
    with_output(handle, kind, |output| {
        if output.is_empty() { return std::ptr::null_mut(); }
        let mut bytes = std::mem::take(output);
        let pointer = bytes.as_mut_ptr();
        std::mem::forget(bytes);
        pointer
    }).unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// Return the exact pointer/capacity from take exactly once, after the reader's
/// acquire/copy/release acknowledgement. No browser view may outlive that read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn playsrc_reply_output_recycle(handle: u32, kind: u32, pointer: *mut u8, capacity: usize) {
    if pointer.is_null() { return; }
    let bytes = unsafe { Vec::from_raw_parts(pointer, 0, capacity) };
    with_output(handle, kind, |output| {
        if output.is_empty() && output.capacity() < capacity { *output = bytes; }
    });
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::encode;

    pub fn assert_reply_ownership(handle: u32) {
        for kind in [2, 4] {
            with_output(handle, kind, |bytes| bytes.extend_from_slice(b"first-view"));
            let capacity = playsrc_reply_output_capacity(handle, kind);
            let pointer = playsrc_reply_output_take(handle, kind);
            assert!(!pointer.is_null());
            assert!(playsrc_reply_output_take(handle, kind).is_null());
            with_output(handle, kind, |bytes| bytes.extend_from_slice(b"second-view"));
            assert_eq!(unsafe { std::slice::from_raw_parts(pointer, 10) }, b"first-view");
            unsafe { playsrc_reply_output_recycle(handle, kind, pointer, capacity) };
            assert_eq!(with_output(handle, kind, |bytes| bytes.clone()).unwrap(), b"second-view");
            let capacity = playsrc_reply_output_capacity(handle, kind);
            let pointer = playsrc_reply_output_take(handle, kind);
            let (index, generation) = decode(handle).unwrap();
            unsafe { playsrc_reply_output_recycle(encode(index, generation + 1), kind, pointer, capacity) };
            assert!(playsrc_reply_output_take(handle, kind).is_null());
        }
    }
}
