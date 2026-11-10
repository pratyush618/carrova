//! `path-rewrite`: a Carrova plugin replacing the old computer's home
//! folder path with this computer's (e.g. `/home/ada` → `/Users/ada` →
//! `C:\Users\ada`) in files that store absolute paths, such as Obsidian's
//! vault list. See `crates/carrova-plugin` for the ABI.
//!
//! The ABI glue below is the only `unsafe`: the host hands over raw
//! pointers into this module's own memory.

pub mod rewrite;

use rewrite::{Homes, Os, context_value};

/// ABI: a buffer of `len` bytes for the host to fill. Leaked on purpose:
/// every call runs in a fresh instance.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: i32) -> i32 {
    let buf = vec![0u8; usize::try_from(len).unwrap_or(0)].leak();
    buf.as_mut_ptr() as usize as i32
}

/// ABI: rewrite one file; returns `(ptr << 32) | len` of the new content.
#[unsafe(no_mangle)]
pub extern "C" fn transform(
    ctx_ptr: i32,
    ctx_len: i32,
    path_ptr: i32,
    path_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i64 {
    let (ctx, path, data) = (
        bytes(ctx_ptr, ctx_len),
        bytes(path_ptr, path_len),
        bytes(data_ptr, data_len),
    );
    let ctx = core::str::from_utf8(ctx).unwrap_or_default();
    let homes = Homes {
        from: context_value(ctx, "source_home"),
        to: context_value(ctx, "target_home"),
        from_os: Os::from_id(context_value(ctx, "source_os")),
        to_os: Os::from_id(context_value(ctx, "target_os")),
    };
    let out = rewrite::rewrite(data, &homes, path.ends_with(b".json")).leak();
    ((out.as_ptr() as i64) << 32) | out.len() as i64
}

/// The host's buffer at `ptr`.
fn bytes(ptr: i32, len: i32) -> &'static [u8] {
    let len = usize::try_from(len).unwrap_or(0);
    if len == 0 {
        return &[];
    }
    // SAFETY: the host wrote `len` bytes at `ptr`, a buffer `alloc` returned
    // and never freed.
    unsafe { core::slice::from_raw_parts(ptr as u32 as usize as *const u8, len) }
}
