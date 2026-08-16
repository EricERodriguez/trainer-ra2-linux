use crate::ptrace_mem::{Attached, MemError};
use serde::{Deserialize, Serialize};

/// A single memory location to inspect/patch. `check` is the byte sequence
/// expected at `address` before the cheat is applied; `patch` is what gets
/// written (and, on re-read, what identifies the cheat as already applied).
/// `check` and `patch` may differ in length: some signatures only verify a
/// prefix of the instruction they end up overwriting in full.
#[derive(Debug, Clone, Copy)]
pub struct PatchSite {
    pub address: u64,
    pub check: &'static [u8],
    pub patch: &'static [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct CheatVariant {
    pub version_label: &'static str,
    pub process_name: &'static str,
    pub sites: &'static [PatchSite],
}

#[derive(Debug, Clone, Copy)]
pub struct Cheat {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub variants: &'static [CheatVariant],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheatState {
    /// Signature matched for a known version; cheat not applied yet.
    NotApplied,
    /// Patch bytes matched for a known version; cheat already applied.
    Applied,
    /// Neither the original nor the patched bytes matched any known
    /// version/process combination (game not running, unsupported version,
    /// or patched by something else).
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheatStatus {
    pub cheat_id: String,
    pub state: CheatState,
    pub version_label: Option<String>,
}

// --- we-can-build-anywhere.sh: allow placing structures outside base range ---

const BUILD_ANYWHERE_RA2_1000: &[PatchSite] = &[
    PatchSite { address: 0x49b8ac, check: &[0x0f, 0x84, 0xa4, 0x01], patch: &[0x90; 6] },
    PatchSite { address: 0x49b8ba, check: &[0x0f, 0x84, 0x96, 0x01], patch: &[0x90; 6] },
];
const BUILD_ANYWHERE_RA2_1006: &[PatchSite] = &[
    PatchSite { address: 0x49bc1c, check: &[0x0f, 0x84, 0xc4, 0x01], patch: &[0x90; 6] },
    PatchSite { address: 0x49bc2a, check: &[0x0f, 0x84, 0xb6, 0x01], patch: &[0x90; 6] },
];
const BUILD_ANYWHERE_RA2MD_1000: &[PatchSite] = &[
    PatchSite { address: 0x4ab9bc, check: &[0x0f, 0x84, 0xc4, 0x01], patch: &[0x90; 6] },
    PatchSite { address: 0x4ab9ca, check: &[0x0f, 0x84, 0xb6, 0x01], patch: &[0x90; 6] },
];

// --- we-are-free.sh: stop credits from decreasing ---

const CREDITS_RA2_1000: &[PatchSite] =
    &[PatchSite { address: 0x4e48f9, check: &[0x2b, 0xc7], patch: &[0xeb, 0x58] }];
const CREDITS_RA2_1006: &[PatchSite] =
    &[PatchSite { address: 0x4e53a9, check: &[0x2b, 0xc7], patch: &[0xeb, 0x58] }];
const CREDITS_RA2MD_1000: &[PatchSite] =
    &[PatchSite { address: 0x4f96d9, check: &[0x2b, 0xc7], patch: &[0xeb, 0x58] }];

// --- we-are-watching-everywhere.sh: reveal full map ---
// The original script only ever patches the first of the three addresses it
// defines per version (the other two are dead code); ported as-is.

const REVEAL_MAP_RA2_1000: &[PatchSite] =
    &[PatchSite { address: 0x4f23a8, check: &[0x74, 0x4a], patch: &[0x90, 0x90] }];
const REVEAL_MAP_RA2_1006: &[PatchSite] =
    &[PatchSite { address: 0x4f2ff8, check: &[0x74, 0x4a], patch: &[0x90, 0x90] }];

// --- our-engineer-fixed-radar.sh: turn on the radar map unconditionally ---

const RADAR_RA2_1000: &[PatchSite] = &[
    PatchSite { address: 0x4f22ca, check: &[0x74, 0x49], patch: &[0x90, 0x90] },
    PatchSite { address: 0x62a469, check: &[0x75, 0x5d], patch: &[0x90, 0x90] },
];
const RADAR_RA2_1006: &[PatchSite] = &[
    PatchSite { address: 0x4f2f1a, check: &[0x74, 0x49], patch: &[0x90, 0x90] },
    PatchSite { address: 0x632f39, check: &[0x75, 0x5d], patch: &[0x90, 0x90] },
];

pub const CHEATS: &[Cheat] = &[
    Cheat {
        id: "build-anywhere",
        name: "Construir en cualquier lugar",
        description: "Permite colocar estructuras fuera del rango de la base",
        variants: &[
            CheatVariant { version_label: "Red Alert 2 v1.000", process_name: "game.exe", sites: BUILD_ANYWHERE_RA2_1000 },
            CheatVariant { version_label: "Red Alert 2 v1.006", process_name: "game.exe", sites: BUILD_ANYWHERE_RA2_1006 },
            CheatVariant { version_label: "Yuri's Revenge v1.000", process_name: "gamemd.exe", sites: BUILD_ANYWHERE_RA2MD_1000 },
        ],
    },
    Cheat {
        id: "infinite-credits",
        name: "Creditos infinitos",
        description: "Detiene la disminucion de creditos",
        variants: &[
            CheatVariant { version_label: "Red Alert 2 v1.000", process_name: "game.exe", sites: CREDITS_RA2_1000 },
            CheatVariant { version_label: "Red Alert 2 v1.006", process_name: "game.exe", sites: CREDITS_RA2_1006 },
            CheatVariant { version_label: "Yuri's Revenge v1.000", process_name: "gamemd.exe", sites: CREDITS_RA2MD_1000 },
        ],
    },
    Cheat {
        id: "reveal-map",
        name: "Revelar mapa completo",
        description: "Revela el mapa completo mientras poseas alguna estructura (solo Red Alert 2, no Yuri's Revenge)",
        variants: &[
            CheatVariant { version_label: "Red Alert 2 v1.000", process_name: "game.exe", sites: REVEAL_MAP_RA2_1000 },
            CheatVariant { version_label: "Red Alert 2 v1.006", process_name: "game.exe", sites: REVEAL_MAP_RA2_1006 },
        ],
    },
    Cheat {
        id: "radar-always-on",
        name: "Radar siempre activo",
        description: "Activa el mapa de radar incondicionalmente (solo Red Alert 2, no Yuri's Revenge)",
        variants: &[
            CheatVariant { version_label: "Red Alert 2 v1.000", process_name: "game.exe", sites: RADAR_RA2_1000 },
            CheatVariant { version_label: "Red Alert 2 v1.006", process_name: "game.exe", sites: RADAR_RA2_1006 },
        ],
    },
];

pub fn find_cheat(id: &str) -> Option<&'static Cheat> {
    CHEATS.iter().find(|c| c.id == id)
}

fn matches(mem: &Attached, site: &PatchSite, expected: &[u8]) -> Result<bool, MemError> {
    let actual = mem.read(site.address, expected.len())?;
    Ok(actual == expected)
}

/// Reads current memory for every variant of `cheat` and reports whether it
/// looks not-applied, applied, or unsupported for whatever process `mem` is
/// attached to. Does not modify memory.
pub fn evaluate_cheat(mem: &Attached, cheat: &Cheat, process_name: &str) -> Result<CheatStatus, MemError> {
    for variant in cheat.variants {
        if variant.process_name != process_name {
            continue;
        }
        let mut all_check = true;
        let mut all_patch = true;
        for site in variant.sites {
            if !matches(mem, site, site.check)? {
                all_check = false;
            }
            if !matches(mem, site, site.patch)? {
                all_patch = false;
            }
        }
        if all_check {
            return Ok(CheatStatus {
                cheat_id: cheat.id.to_string(),
                state: CheatState::NotApplied,
                version_label: Some(variant.version_label.to_string()),
            });
        }
        if all_patch {
            return Ok(CheatStatus {
                cheat_id: cheat.id.to_string(),
                state: CheatState::Applied,
                version_label: Some(variant.version_label.to_string()),
            });
        }
    }
    Ok(CheatStatus { cheat_id: cheat.id.to_string(), state: CheatState::Unsupported, version_label: None })
}

/// Finds the variant matching the current (unpatched) signature for
/// `process_name` and writes its patch bytes. Errors if no variant's
/// original signature matches (already applied, unsupported version, or
/// wrong process).
pub fn apply_cheat(mem: &Attached, cheat: &Cheat, process_name: &str) -> Result<CheatStatus, MemError> {
    for variant in cheat.variants {
        if variant.process_name != process_name {
            continue;
        }
        let mut all_check = true;
        for site in variant.sites {
            if !matches(mem, site, site.check)? {
                all_check = false;
                break;
            }
        }
        if all_check {
            for site in variant.sites {
                mem.write(site.address, site.patch)?;
            }
            return Ok(CheatStatus {
                cheat_id: cheat.id.to_string(),
                state: CheatState::Applied,
                version_label: Some(variant.version_label.to_string()),
            });
        }
    }
    Err(MemError::NoMatchingVariant)
}
