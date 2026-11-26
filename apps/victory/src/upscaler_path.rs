//! The title's FidelityFX upscaler path for DLSS (`VICTORY_DLSS=1`) or the
//! title's own FidelityFX at a chosen scale (`VICTORY_FSR_PATH=1`), on the
//! supported retail image (GTA VI 1.0.2.1). On Series S the stock title
//! already renders through that path (renderer 7) at 2/3 of its output; on
//! Series X it uses its own temporal pass. The patch makes the path taken on
//! both and sets the scene at `VICTORY_DLSS_MODE`'s scale of the output.
//!
//! Ported from the foxhound fork (`apps/victory/src/native_1440p.rs`,
//! `prepare_upscaler`, from DexNom's fork DexNom/VIctory, GPL-3.0-or-later),
//! without its Series X 1440p output patch.

use anyhow::{Context, Result, ensure};
use victory_hle::streamline::Mode;
use victory_loader::PreparedImage;

const RETAIL_BUILD: &str = "b99f876b74d4a77f03af5f878223b27bd647b5c35a79d5decd9f716504239e93";
/// The title's own temporal pass's scene scale (stock 2/3).
const TEMPORAL_SCALE: u32 = 0x028f_1e7e;

/// The title's renderer selection: `mov eax, 6; cmovne eax, ecx` (the
/// title's own temporal pass, or on Series S with the upscaler there the
/// FidelityFX path, 7).
const RENDERER_SELECTION: u32 = 0x007e_9421;
const STOCK_RENDERER_SELECTION: [u8; 8] = [0xb8, 6, 0, 0, 0, 0x0f, 0x45, 0xc1];
/// The FidelityFX path's render size: the console check's result (Series S)
/// is noted as the FidelityFX quality mode and picks the scaled branch
/// (`movzx eax, al; mov [r14+0x90], eax; test al, al; jne`), whose scale
/// multiplies the output size.
const UPSCALER_SIZING: u32 = 0x028e_f69a;
const STOCK_UPSCALER_SIZING: [u8; 14] = [0x0f, 0xb6, 0xc0, 0x41, 0x89, 0x86, 0x90, 0, 0, 0, 0x84, 0xc0, 0x75, 0x0a];
/// The native branch's load of 1.0, and the scaled branch's of the scale.
const NATIVE_LOAD: u32 = 0x028e_f6a8;
const STOCK_NATIVE_LOAD: [u8; 10] = [0xc5, 0xfa, 0x10, 0x05, 0x28, 0x78, 0x6d, 0x03, 0xeb, 0x08];
const SCALED_LOAD: u32 = 0x028e_f6b2;
const STOCK_SCALED_LOAD: [u8; 8] = [0xc5, 0xfa, 0x10, 0x05, 0xa2, 0xe2, 0x86, 0x03];
/// The scale the scaled branch loads (shared with other code: not written).
const UPSCALER_SCALE: u32 = 0x0615_d95c;

/// Makes the title render through its FidelityFX upscaler path (renderer 7,
/// as Series S does) with the scene at `mode`'s scale of the output, so that
/// DLSS can take its upscale dispatches over (`victory_hle::modules::fidelityfx_dlss`).
/// On Series X the stock title uses its own temporal pass instead and never
/// loads its upscaler. The FidelityFX quality mode the title notes is
/// `mode`'s; the temporal scene scale (the title's own pass, and
/// `prepare`'s 1440p patch) is set to the same scale, as the stock Series X
/// image pairs them (2/3 each). Every patched instruction is checked against
/// the stock bytes first; nothing is written if one differs.
///
/// From DexNom's fork (DexNom/VIctory, `apps/victory/src/native_1440p.rs`,
/// `prepare_fsr_4k`, GPL-3.0-or-later), there for a 4K output; kept here
/// with the 1440p one.
pub(super) fn prepare(image: &mut PreparedImage, mode: Mode) -> Result<()> {
    ensure!(
        image.original_hash.to_hex().as_str() == RETAIL_BUILD,
        "the upscaler path is only known for the retail build {RETAIL_BUILD}"
    );
    ensure!(*field::<8>(image, RENDERER_SELECTION)? == STOCK_RENDERER_SELECTION, "unexpected renderer selection");
    ensure!(*field::<14>(image, UPSCALER_SIZING)? == STOCK_UPSCALER_SIZING, "unexpected upscaler render size selection");
    ensure!(*field::<10>(image, NATIVE_LOAD)? == STOCK_NATIVE_LOAD, "unexpected upscaler native scale load");
    ensure!(*field::<8>(image, SCALED_LOAD)? == STOCK_SCALED_LOAD, "unexpected upscaler scaled load");
    ensure!(*field::<4>(image, UPSCALER_SCALE)? == (2.0f32 / 3.0).to_le_bytes(), "unexpected upscaler scale");
    let temporal = f32::from_le_bytes(*field::<4>(image, TEMPORAL_SCALE)?);
    ensure!(
        temporal == 2.0 / 3.0 || temporal == 1.0,
        "unexpected temporal scene scale {temporal} (stock 2/3, or 1 with a 1440p output patch)"
    );
    // Renderer 7 every time it is asked (the console check has run).
    *field::<8>(image, RENDERER_SELECTION)? = [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90];
    // `mov eax, mode; mov [r14+0x90], eax; jmp scaled`.
    *field::<14>(image, UPSCALER_SIZING)? = [0xb8, mode.ffx_mode(), 0, 0, 0, 0x41, 0x89, 0x86, 0x90, 0, 0, 0, 0xeb, 0x0a];
    if mode != Mode::Quality {
        // The jump leaves the native branch unreached: its load's operand
        // becomes a literal of ours, which the scaled branch loads.
        let literal = NATIVE_LOAD + 4;
        *field::<4>(image, literal)? = mode.scale().to_le_bytes();
        let displacement = literal as i32 - (SCALED_LOAD as i32 + 8);
        *field::<4>(image, SCALED_LOAD + 4)? = displacement.to_le_bytes();
    }
    *field::<4>(image, TEMPORAL_SCALE)? = mode.scale().to_le_bytes();
    Ok(())
}

fn field<const N: usize>(image: &mut PreparedImage, rva: u32) -> Result<&mut [u8; N]> {
    let offset = image.info.rva_to_file_offset(rva).context("display profile field missing")? as usize;
    image
        .bytes
        .get_mut(offset..offset + N)
        .context("incomplete display profile field")?
        .try_into()
        .context("invalid display profile field length")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the retail executable named by `VICTORY_DISPLAY_PROFILE_TEST_IMAGE`
    /// (not part of the repository): the upscaler path at each scale; the
    /// scaled branch then loads the mode's scale and the noted mode is its
    /// FidelityFX mode. Without it, nothing is checked.
    #[test]
    fn patches_the_upscaler_path() {
        let Some(path) = std::env::var_os("VICTORY_DISPLAY_PROFILE_TEST_IMAGE") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        for mode in [Mode::Dlaa, Mode::Quality, Mode::Balanced, Mode::Performance, Mode::UltraPerformance] {
            let mut image = victory_loader::prepare(&bytes).unwrap();
            prepare(&mut image, mode).unwrap();
            assert_eq!(*field::<8>(&mut image, RENDERER_SELECTION).unwrap(), [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90]);
            let sizing = *field::<14>(&mut image, UPSCALER_SIZING).unwrap();
            assert_eq!(sizing[1], mode.ffx_mode());
            assert_eq!(sizing[12..], [0xeb, 0x0a]);
            // The jump lands on the scaled load.
            assert_eq!(UPSCALER_SIZING + 14 + 0x0a, SCALED_LOAD);
            let disp = i32::from_le_bytes(*field::<4>(&mut image, SCALED_LOAD + 4).unwrap());
            let loaded = (SCALED_LOAD + 8).wrapping_add_signed(disp);
            assert_eq!(f32::from_le_bytes(*field::<4>(&mut image, loaded).unwrap()), mode.scale());
            assert_eq!(f32::from_le_bytes(*field::<4>(&mut image, TEMPORAL_SCALE).unwrap()), mode.scale());
            // The shared constant stays as it was.
            assert_eq!(*field::<4>(&mut image, UPSCALER_SCALE).unwrap(), (2.0f32 / 3.0).to_le_bytes());
            // Patched once, refused.
            assert!(prepare(&mut image, mode).is_err());
        }
    }
}
