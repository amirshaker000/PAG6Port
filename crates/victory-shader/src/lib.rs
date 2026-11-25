//! Xbox Series shaders to host shaders.
//!
//! An Xbox Series shader is a standard DXBC container ([`dxbc`]). Its `DXIL`
//! part ([`dxil`]) may carry Xbox additions around the LLVM bitcode: an
//! `XDAT` table before it indexing blobs after it, among them `xbop`
//! (compiler options) and the zlib-compressed GFX10 code. The bitcode may use
//! console-only intrinsics, so the GFX10 code is translated in general.
//! Precompiled programs without a `DXIL` part keep the code in an `ACCC`
//! part instead ([`fspx`]).

pub mod bvh;
pub mod dxbc;
pub mod dxil;
pub mod emit;
pub mod fspx;
pub mod gfx10;
pub mod handoff;
pub mod io;
pub mod ir;
pub mod merged;
pub mod native;
pub mod resource;
pub mod spill;
pub mod structure;

/// Synthetic shader containers, for the tests of crates that use this one
/// (feature `synthetic`): programs written for the tests, not the title's.
#[cfg(feature = "synthetic")]
pub mod synthetic {
    /// The vertex container (ES half) and the geometry container (GS half,
    /// three emits) of a merged stage.
    pub fn merged_stage() -> (Vec<u8>, Vec<u8>) {
        (
            crate::emit::gs::tests::es_container(),
            crate::emit::gs::tests::gs_container(3),
        )
    }

    /// A pixel container whose program does nothing.
    pub fn pixel_container() -> Vec<u8> {
        crate::native::tests::container(&[(crate::native::HwStage::Ps, &0xBF81_0000u32.to_le_bytes())])
    }
}

pub use dxbc::{Container, FourCC, Part};
pub use dxil::{DxilProgram, ShaderKind, XboxExtras};
pub use native::{
    HwStage, NativeProgram, ResourceType, RootParam, RootType, UsedResource, native_programs,
    program_part,
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShaderError {
    #[error("not a DXBC container")]
    NotDxbc,
    #[error("malformed shader: {0}")]
    Malformed(&'static str),
    #[error("GFX10 code at {offset:#x}: {reason}")]
    Isa { offset: usize, reason: &'static str },
    /// Valid code the translator cannot handle yet: an opcode name or a construct.
    #[error("GFX10 code at {offset:#x}: {what} is not supported")]
    Unsupported { offset: usize, what: &'static str },
}

// Keep the paths used by the coverage test and the bytes used by the shader cache
// together. Adding any new Rust source requires classifying it below.
macro_rules! translation_inputs {
    ($($path:literal),+ $(,)?) => {
        /// Sources that can affect a translation kept on disk.
        pub fn sources() -> &'static [&'static str] {
            &[$(include_str!($path)),+]
        }

        #[cfg(test)]
        const INPUT_PATHS: &[&str] = &[$($path),+];
    };
}

translation_inputs! {
    "bvh.rs",
    "dxbc.rs",
    "dxil.rs",
    "emit/bitcode.rs",
    "emit/bitcode/census.rs",
    "emit/bitcode/group_sync.rs",
    "emit/bitcode/opt.rs",
    "emit/container.rs",
    "emit/descriptor_data.rs",
    "emit/descriptor_tests.rs",
    "emit/float64.rs",
    "emit/gs.rs",
    "emit/idiom.rs",
    "emit/intersect.rs",
    "emit/link.rs",
    "merged.rs",
    "emit/lower.rs",
    "emit/mod.rs",
    "emit/own_bits.rs",
    "emit/rect.rs",
    "emit/sampler.rs",
    "emit/shader.rs",
    "emit/threading.rs",
    "emit/uniform.rs",
    "emit/validator.rs",
    "emit/lower/split_lds.rs",
    "emit/stand_in.rs",
    "emit/wave.rs",
    "gfx10/fields.rs",
    "gfx10/mod.rs",
    "gfx10/opcodes.rs",
    "gfx10/sp3.rs",
    "handoff.rs",
    "io.rs",
    "ir.rs",
    "lib.rs",
    "native.rs",
    "resource.rs",
    "spill.rs",
    "fspx.rs",
    "structure.rs",
}

#[cfg(test)]
mod source_tests {
    use std::path::{Path, PathBuf};

    // host.rs consumes finished DXIL to create or run host pipelines. The
    // remaining modules are only compiled by the Windows execution tests.
    const OUTSIDE_TRANSLATION: &[&str] = &[
        "emit/diff_tests.rs",
        "emit/host.rs",
        "emit/intersect_tests.rs",
        "emit/valu_tests.rs",
        "emit/waterfall_tests.rs",
        "emit/wave64_tests.rs",
    ];

    #[test]
    fn every_shader_source_is_classified_by_path() {
        fn collect(dir: &Path, root: &Path, paths: &mut Vec<PathBuf>) {
            for entry in dir.read_dir().unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(&path, root, paths);
                } else if path.extension().is_some_and(|s| s == "rs") {
                    paths.push(path.strip_prefix(root).unwrap().to_path_buf());
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect(&root, &root, &mut files);
        let mut classified: Vec<PathBuf> = super::INPUT_PATHS
            .iter()
            .chain(OUTSIDE_TRANSLATION)
            .map(PathBuf::from)
            .collect();
        let norm = |v: Vec<PathBuf>| {
            let mut v: Vec<String> = v
                .into_iter()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .collect();
            v.sort();
            v
        };
        assert_eq!(norm(classified), norm(files));
    }
}
