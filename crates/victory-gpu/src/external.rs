//! Host objects for PC libraries the title's calls are redirected to (the
//! FidelityFX upscaler of the PC). The title holds our console objects; a
//! PC library needs the host device, the host command list the title's list
//! records onto, and the host textures behind the title's.
//!
//! Also the host work that DLSS (`crate::streamline`) records into a title's
//! command list in place of the title's FidelityFX upscaling: the title
//! textures' host copies in the states the library is told, prepared as a
//! compute shader's inputs would be, and what was written. That part is
//! ported from the foxhound fork (from DexNom's fork, DexNom/VIctory,
//! GPL-3.0-or-later); the work runs where the title's list is translated
//! (`crate::list::external`, also with `VICTORY_THIN=1`).
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::c_void;

use windows::Win32::Graphics::Direct3D12::ID3D12Resource;

use crate::host::HostList;
use crate::object::This;

/// The host `ID3D12Device` (no reference taken), if there is a host GPU.
pub fn host_device() -> Option<*mut c_void> {
    crate::host::host().map(|h| windows::core::Interface::as_raw(&h.device))
}

/// Whether `object` is the device the title created.
pub fn is_title_device(object: *const c_void) -> bool {
    std::ptr::eq(object, (&raw const crate::device::DEVICE).cast())
}

/// Whether `object` is a command list of ours.
pub fn is_title_list(object: *const c_void) -> bool {
    crate::list::is_list(object.cast())
}

/// Whether `object` is a live title texture.
pub fn is_title_texture(object: *const c_void) -> bool {
    crate::resource::is_texture(object.cast())
}

/// One title list's host list, open for a PC library to record onto.
pub struct Pass<'a> {
    list: &'a crate::host::HostList,
}

impl Pass<'_> {
    /// The host `ID3D12GraphicsCommandList` (no reference taken).
    pub fn command_list(&self) -> *mut c_void {
        windows::core::Interface::as_raw(&self.list.list)
    }

    /// The host texture behind title texture `texture`, moved to the host
    /// resource state `state` (a D3D12 state, as the library will see it)
    /// on this list; `write` if the library writes it. `None` if the title
    /// has no texture there or the host refuses it.
    pub fn texture(&mut self, texture: *const c_void, state: u32, write: bool) -> Option<*mut c_void> {
        let title: This = texture.cast();
        if !crate::resource::is_texture(title) {
            return None;
        }
        let host = crate::resource::host_resource(title)?;
        // What the title fills itself is uploaded first, as for a copy.
        if !write && crate::texture::title_written(host) {
            crate::texture::upload(self.list, title, host);
        }
        // Depth textures shaders also write: what they wrote, first.
        self.list.twin_use(title, false, write);
        crate::list::ensure_state(self.list, title, state);
        if write && crate::texture::title_written(host) {
            crate::texture::host_written(title);
        }
        Some(windows::core::Interface::as_raw(host))
    }
}

/// Runs `f` with title list `list`'s host list; afterwards the title's
/// bindings are set again on it (the library set its own). `false` if
/// `list` is not a list of ours with a host list.
pub fn record(list: *const c_void, f: impl FnOnce(&mut Pass)) -> bool {
    crate::list::with_host(list.cast(), |host, state| {
        f(&mut Pass { list: host });
        state.replay(host); // (cool_fox port) sets again what a new host list would need
    })
}

/// A title texture's host copy, moved to the host resource `state` in
/// `list`, and the state it is in: `state` without the bits the host copy
/// cannot take (see `host::legal_state`). What the list records next finds
/// it there and must leave it there. The list holds the texture until it is
/// reset. `None` for a buffer.
pub(crate) fn host_texture(list: &HostList, resource: This, state: u32) -> Option<(ID3D12Resource, u32)> {
    let texture = crate::resource::host_resource(resource)?.clone();
    crate::list::ensure_state(list, resource, state);
    let state = crate::host::legal_state(&texture, state);
    Some((texture, state))
}

/// Prepares a title-authored input with the alias, upload and depth twin
/// rules a compute shader's resource view has in the renderer.
pub(crate) fn prepare_input(list: &HostList, resource: This) -> Option<()> {
    let texture = crate::resource::host_resource(resource)?;
    let aliased = crate::resource::alias_of(resource).is_some_and(|source| crate::texture::alias(list, resource, source));
    if !aliased && crate::texture::title_written(texture) {
        crate::texture::upload_from(list, resource, 0);
    }
    list.twin_use(resource, false, false);
    Some(())
}

/// The library wrote `resource` on the host: its contents are there, not in
/// title memory.
pub(crate) fn output_written(resource: This) {
    crate::texture::host_written(resource);
}

/// Sends what the title submitted (the line of submissions first) and
/// settles it, before a library lets go of resources the GPU may still use.
pub(crate) fn finish_submitted() {
    crate::queue::before_sync();
    crate::host::settle_for_fence();
}
