// ── Campaign mode: story-driven chapters with named characters,
//     authored level specs, and choice-driven narrative ──
//
// Character voices:
//   Mara Voss   — clipped, military, tired, never wastes a word
//   Tovan Reed   — scared, injured, gallows humour, rambles to avoid panic
//   Edda Slate   — sharp, dry, precise, talks like she corrects idiots
//   Jun          — gentle, superstitious, poetic but grounded
//   Vey          — charming, morally rotten, too comfortable underground

use serde::{Deserialize, Serialize};

use crate::entity::{CampKind, ItemKind, Landmark, MonsterKind, NpcKind};

// ══════════════════════════════════════════════════════════════════════════════
// CAMPAIGN STATE
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignChapter {
    /// "The Bell Line" — discover the Crown is trying to return
    Chapter1,
}

impl CampaignChapter {
    pub fn name(&self) -> &'static str {
        match self {
            CampaignChapter::Chapter1 => "Chapter I: The Bell Line",
        }
    }
}

/// Flags set by player choices that persist across campaign levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CampaignFlag {
    // ── Tovan Reed ──
    SavedTovan,
    TookTovanSupplies,
    TovanHelped, // set after first non-loot interaction with Tovan

    // ── Edda Slate ──
    RescuedApprentice,
    AskedEddaAboutCrown,
    AskedEddaForMap,
    AskedJunForOil,

    // ── Level 1 objective ──
    RecoveredBellToken,
    GateOpened, // set when Mara opens the Rustguard gate

    // ── Level 2 objective ──
    Lantern1Lit,
    Lantern2Lit,
    Lantern3Lit,
    AllLanternsLit,

    // ── Chapter 3 Bell choice ──
    BellChoiceRung,
    BellChoiceShattered,
    BellChoiceMuffled,

    // ── Vey ──
    VeyRewardClaimed, // prevents infinite smoke-bomb farming
}

/// Trust score per character. Positive = trust, negative = distrust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CampaignNpc {
    MaraVoss,
    TovanReed,
    EddaSlate,
    Jun,
    Vey,
}

impl CampaignNpc {
    pub fn name(&self) -> &'static str {
        match self {
            CampaignNpc::MaraVoss => "Mara Voss",
            CampaignNpc::TovanReed => "Tovan Reed",
            CampaignNpc::EddaSlate => "Edda Slate",
            CampaignNpc::Jun => "Jun",
            CampaignNpc::Vey => "Vey",
        }
    }

    pub fn npc_kind(&self) -> NpcKind {
        match self {
            CampaignNpc::MaraVoss => NpcKind::WatchCaptain,
            CampaignNpc::TovanReed => NpcKind::WoundedDelver,
            CampaignNpc::EddaSlate => NpcKind::Cartographer,
            CampaignNpc::Jun => NpcKind::LanternTender,
            CampaignNpc::Vey => NpcKind::Merchant,
        }
    }

    /// Voice description for writing reference.
    #[allow(dead_code)]
    pub fn voice(&self) -> &'static str {
        match self {
            CampaignNpc::MaraVoss => "clipped, military, tired — never wastes a word",
            CampaignNpc::TovanReed => "scared, injured, gallows humour — rambles to avoid panic",
            CampaignNpc::EddaSlate => "sharp, dry, precise — talks like she corrects idiots",
            CampaignNpc::Jun => "gentle, superstitious, poetic but grounded",
            CampaignNpc::Vey => "charming, morally rotten, too comfortable underground",
        }
    }
}

/// A campaign-level objective the player must complete before descending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampaignObjective {
    /// Level 1: recover the Rustguard bell-token.
    RecoverBellToken,
    /// Level 2: relight lanterns. (lit_count, needed_count)
    RelightLanterns { lit: u32, needed: u32 },
    /// Level 3: reach and resolve the bell chamber.
    SilenceTheBell,
}

impl CampaignObjective {
    /// Human-readable objective text for the sidebar.
    pub fn display(&self) -> String {
        match self {
            CampaignObjective::RecoverBellToken => {
                "Recover the missing Rustguard bell-token".into()
            }
            CampaignObjective::RelightLanterns { lit, needed } => {
                format!("Relight the blue lanterns ({}/{})", lit, needed)
            }
            CampaignObjective::SilenceTheBell => {
                "Reach the bell chamber and silence the corrupted bell".into()
            }
        }
    }
}

/// Full campaign progression state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignState {
    pub chapter: CampaignChapter,
    /// 0-based campaign level (0 = Level 1, 1 = Level 2, 2 = Level 3).
    pub level: u32,
    pub objectives: Vec<CampaignObjective>,
    /// Trust scores for named NPCs.
    pub trust: Vec<(CampaignNpc, i32)>,
    /// Flags set by player choices.
    pub flags: Vec<CampaignFlag>,
}

impl CampaignState {
    pub fn new() -> Self {
        Self {
            chapter: CampaignChapter::Chapter1,
            level: 0,
            objectives: vec![CampaignObjective::RecoverBellToken],
            trust: vec![
                (CampaignNpc::MaraVoss, 0),
                (CampaignNpc::TovanReed, 0),
                (CampaignNpc::EddaSlate, 0),
                (CampaignNpc::Jun, 0),
                (CampaignNpc::Vey, 0),
            ],
            flags: Vec::new(),
        }
    }

    pub fn has_flag(&self, flag: CampaignFlag) -> bool {
        self.flags.contains(&flag)
    }

    pub fn set_flag(&mut self, flag: CampaignFlag) {
        if !self.flags.contains(&flag) {
            self.flags.push(flag);
        }
    }

    pub fn trust_of(&self, npc: CampaignNpc) -> i32 {
        self.trust
            .iter()
            .find(|(n, _)| *n == npc)
            .map(|(_, t)| *t)
            .unwrap_or(0)
    }

    pub fn modify_trust(&mut self, npc: CampaignNpc, delta: i32) {
        if let Some((_, t)) = self.trust.iter_mut().find(|(n, _)| *n == npc) {
            *t = (*t + delta).clamp(-5, 5);
        }
    }

    /// Set objectives for the given campaign level.
    pub fn set_objectives_for_level(&mut self, level: u32) {
        self.objectives.clear();
        match level {
            0 => {
                self.objectives.push(CampaignObjective::RecoverBellToken);
            }
            1 => {
                self.objectives
                    .push(CampaignObjective::RelightLanterns { lit: 1, needed: 3 });
            }
            2 => {
                self.objectives.push(CampaignObjective::SilenceTheBell);
            }
            _ => {}
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CAMPAIGN LEVEL SPECS
// ══════════════════════════════════════════════════════════════════════════════

/// Defines what must appear on a campaign level: landmarks, NPCs, enemy mix.
pub struct CampaignLevelSpec {
    /// Level name shown on entry.
    pub name: &'static str,
    /// Biome flavour line.
    pub descent_line: &'static str,
    /// Depth used for biome palette and monster scaling.
    pub depth: u32,
    /// Required landmarks (placed in specific rooms).
    pub landmarks: &'static [Landmark],
    /// Named campaign NPCs to spawn.
    pub named_npcs: &'static [(CampaignNpc, &'static str /* role label */)],
    /// Camp kind (Some = place a camp of this type).
    pub camp: Option<CampKind>,
    /// Enemy mix override: which monsters to prefer.
    pub enemy_mix: &'static [(MonsterKind, u32 /* weight */)],
    /// Monster count override (None = use default 3 + depth*2).
    pub monster_count: Option<usize>,
    /// Ending flavour lines shown when descending from this level.
    pub chapter_outro: &'static [&'static str],
}

/// Chapter 1 level specs.
pub fn chapter1_level_spec(level: u32) -> Option<CampaignLevelSpec> {
    match level {
        0 => Some(CampaignLevelSpec {
            name: "The Moss-Mouth",
            descent_line: "You lower yourself into the moss-choked entrance. Somewhere below, a bell waits.",
            depth: 0,
            landmarks: &[Landmark::CollapsedForge, Landmark::FloodedVault],
            named_npcs: &[
                (CampaignNpc::TovanReed, "Wounded delver — first human face"),
                (CampaignNpc::MaraVoss, "Rustguard Watch-Captain — controls the stair-gate"),
            ],
            camp: None,
            enemy_mix: &[
                (MonsterKind::Rat, 5),
                (MonsterKind::Bat, 3),
                (MonsterKind::Goblin, 4),
                (MonsterKind::Slime, 2),
            ],
            monster_count: None,
            chapter_outro: LEVEL1_OUTRO,
        }),
        1 => Some(CampaignLevelSpec {
            name: "The Blue Lantern Waystation",
            descent_line: "You descend into frozen halls. Blue lanterns flicker — most are dead. The waystation is intact but blind.",
            depth: 1,
            landmarks: &[Landmark::FloodedVault],
            named_npcs: &[
                (CampaignNpc::Jun, "Lantern Tender — teaches about light"),
                (CampaignNpc::EddaSlate, "Cartographer — maps are failing"),
                (CampaignNpc::Vey, "Merchant — selling hope underground"),
            ],
            camp: Some(CampKind::DelverWaystation),
            enemy_mix: &[
                (MonsterKind::Bat, 3),
                (MonsterKind::Skeleton, 4),
                (MonsterKind::Slime, 2),
                (MonsterKind::Mimic, 1),
            ],
            monster_count: None,
            chapter_outro: LEVEL2_OUTRO,
        }),
        2 => Some(CampaignLevelSpec {
            name: "The Toll of Rust",
            descent_line: "You climb down into rusting crypt halls. The bell is close now — you can feel it in your teeth.",
            depth: 2,
            landmarks: &[Landmark::BoneChapel, Landmark::BellChamber],
            named_npcs: &[
                (CampaignNpc::MaraVoss, "Watch-Captain — preparing for what follows you"),
            ],
            camp: Some(CampKind::RustguardOutpost),
            enemy_mix: &[
                (MonsterKind::Skeleton, 3),
                (MonsterKind::Goblin, 2),
                (MonsterKind::Orc, 2),
                (MonsterKind::Necromancer, 1),
                (MonsterKind::Sentinel, 1),
            ],
            monster_count: Some(6),
            chapter_outro: &[
                "Beneath the broken bell, the stairs continue downward. Warm gold light pulses once, very far below.",
                "Not a treasure gleam. A heartbeat.",
            ],
        }),
        _ => None,
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CAMPAIGN CHARACTER DIALOG TREES
// ══════════════════════════════════════════════════════════════════════════════
//
// Each tree is parameterised by campaign state flags so the same character
// can offer different conversation based on what has already happened.
//
// The trees use i32 next_node indices where:
//   -2 = apply effect + close conversation
//   -3 = close conversation (no effect)
//   >= 0 = advance to that node

use std::collections::HashSet;

/// Gets the campaign dialog tree for a named NPC at the current campaign level.
/// Returns (tree, node labels, choice effect map).
pub fn campaign_dialog(
    npc: CampaignNpc,
    _campaign_level: u32,
    flags: &[CampaignFlag],
    trust: i32,
) -> Option<&'static [CampaignDialogNode]> {
    let flag_set: HashSet<CampaignFlag> = flags.iter().copied().collect();

    match npc {
        CampaignNpc::MaraVoss => Some(mara_dialog(&flag_set, trust)),
        CampaignNpc::TovanReed => Some(tovan_dialog(&flag_set, trust)),
        CampaignNpc::EddaSlate => Some(edda_dialog(&flag_set, trust)),
        CampaignNpc::Jun => Some(jun_dialog(&flag_set, trust)),
        CampaignNpc::Vey => Some(vey_dialog(&flag_set, trust)),
    }
}

/// What happens when a campaign dialog node is resolved.
#[derive(Debug, Clone)]
pub enum DialogEffect {
    /// Set a campaign flag.
    SetFlag(CampaignFlag),
    /// Modify trust for a named NPC.
    Trust(CampaignNpc, i32),
    /// Give an item to the player.
    GiveItem(ItemKind),
    /// Reveal the entire map.
    RevealMap,
    /// Heal player fully + give health potion.
    HealAndPotion,
    /// Complete an objective (advances progress).
    CompleteObjective,
}

/// A campaign dialog node — like TalkNode but with effect metadata.
pub struct CampaignDialogNode {
    /// What the NPC says.
    pub npc_line: &'static str,
    /// Player choices. Empty = leaf node.
    pub choices: &'static [CampaignDialogChoice],
    /// Closing line after choice.
    pub closing: Option<&'static str>,
}

/// A choice in a campaign dialog.
pub struct CampaignDialogChoice {
    /// Short label shown to the player.
    pub label: &'static str,
    /// Index into the dialog tree for the next node.
    /// Special: -2 = close with effect, -3 = close without effect.
    pub next_node: i32,
    /// What happens when this choice is made.
    pub effect: &'static [DialogEffect],
}

// ══════════════════════════════════════════════════════════════════════════════
// MARA VOSS — Level 1: The Moss-Mouth
// ══════════════════════════════════════════════════════════════════════════════

fn mara_dialog(flags: &HashSet<CampaignFlag>, trust: i32) -> &'static [CampaignDialogNode] {
    if flags.contains(&CampaignFlag::RecoveredBellToken) {
        if trust >= 2 {
            &MARA_WARM
        } else {
            &MARA_POST_TOKEN
        }
    } else if trust <= -2 {
        &MARA_COLD
    } else {
        &MARA_GATE
    }
}

/// Mara — hostile variant (trust ≤ -2)
static MARA_COLD: [CampaignDialogNode; 2] = [
    CampaignDialogNode {
        npc_line: "\"I don't know why the Compact sent you. The last three were more convincing.\" She keeps the gate lever at her back.",
        choices: &[
            CampaignDialogChoice {
                label: "\"I'm here for the token.\"",
                next_node: 1,
                effect: &[],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"Then hurry up. You're wasting torchlight and my patience.\"",
        choices: &[],
        closing: Some("She doesn't turn her back on you."),
    },
];

/// Mara — warm variant (trust ≥ 2)
static MARA_WARM: [CampaignDialogNode; 2] = [
    CampaignDialogNode {
        npc_line: "\"You came back. The token, the gate — they can wait. Sit. Your torch is almost done.\" She pulls a flask from her belt. \"Rustguard tradition. Don't refuse.\"",
        choices: &[
            CampaignDialogChoice {
                label: "\"The gate — I need to go deeper.\"",
                next_node: 1,
                effect: &[DialogEffect::CompleteObjective],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"Then go.\" She stands. \"But remember — if you hear bells below, don't follow them. Nothing down there rings for your benefit.\" She pulls the lever. The gate groans open.",
        choices: &[],
        closing: Some("Cold air rushes up from below. She hands you the flask anyway. \"For the road.\""),
    },
];

/// Mara at the Rustguard gate — before you have the token.
static MARA_GATE: [CampaignDialogNode; 3] = [
    CampaignDialogNode {
        npc_line: "\"Compact sent you? Of course they did. Greyford hears a bell in the dark and sends someone who still thinks contracts matter underground.\" Her hand rests on the gate lever. It doesn't move.",
        choices: &[
            CampaignDialogChoice {
                label: "\"I'm here for the missing bell-token.\"",
                next_node: 1,
                effect: &[],
            },
            CampaignDialogChoice {
                label: "\"You sound like you expected this.\"",
                next_node: 2,
                effect: &[DialogEffect::Trust(CampaignNpc::MaraVoss, 1)],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"Good. Then pretend you're useful and keep your hands where I can see them. Find the token. Bring it back. Then we talk about what's below.\"",
        choices: &[],
        closing: Some("She gestures toward the dark. \"Last delver who passed here dropped it. Or something pulled it off him. Either way, the gate stays closed.\""),
    },
    CampaignDialogNode {
        npc_line: "\"I expected rats. Goblins. Starving fools looking for treasure. I did not expect something below us to learn Rustguard signals. That bell rang from the wrong direction.\" She taps the gate. \"Find the token. I'll know when you have it.\"",
        choices: &[],
        closing: Some("She turns away. \"Move. You're burning torchlight.\""),
    },
];

/// Mara after the player returns with the bell-token.
static MARA_POST_TOKEN: [CampaignDialogNode; 3] = [
    CampaignDialogNode {
        npc_line: "She takes the token without looking at you. Turns it over once. \"Rustguard iron. No mistaking it. Whoever carried this died facing upward — that means the thing that killed him was already behind him.\"",
        choices: &[
            CampaignDialogChoice {
                label: "\"Open the gate.\"",
                next_node: 1,
                effect: &[DialogEffect::CompleteObjective],
            },
            CampaignDialogChoice {
                label: "\"What killed him?\"",
                next_node: 2,
                effect: &[DialogEffect::Trust(CampaignNpc::MaraVoss, 1), DialogEffect::CompleteObjective],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"Fine. But listen — if you hear bells below, don't follow them. The Rustguard rings to warn the surface. Something down there has learned to ring back.\" She pulls the lever. The gate groans open.",
        choices: &[],
        closing: Some("The gate grinds open. Cold air rushes up from below. \"Don't make me regret this.\""),
    },
    CampaignDialogNode {
        npc_line: "\"Something quiet. The deep doesn't announce itself with noise — it announces itself with silence. When everything stops, that's when you should be afraid.\" She studies the token again. \"This was still warm when they found it. Three days after he died.\"",
        choices: &[],
        closing: Some("She pulls the lever without another word. The gate opens."),
    },
];

// ══════════════════════════════════════════════════════════════════════════════
// TOVAN REED — Level 1: Wounded Delver
// ══════════════════════════════════════════════════════════════════════════════

fn tovan_dialog(flags: &HashSet<CampaignFlag>, _trust: i32) -> &'static [CampaignDialogNode] {
    if flags.contains(&CampaignFlag::SavedTovan) {
        &TOVAN_SAVED
    } else if flags.contains(&CampaignFlag::TovanHelped) {
        &TOVAN_HELPED
    } else {
        &TOVAN_WOUNDED
    }
}

static TOVAN_WOUNDED: [CampaignDialogNode; 4] = [
    CampaignDialogNode {
        npc_line: "\"Don't step there. Blood makes the moss slick. Learned that with my face.\" He props himself against the wall, breathing shallow. The pack beside him is torn open.",
        choices: &[
            CampaignDialogChoice {
                label: "\"Who did this to you?\"",
                next_node: 1,
                effect: &[DialogEffect::SetFlag(CampaignFlag::TovanHelped), DialogEffect::GiveItem(ItemKind::HealthPotion)],
            },
            CampaignDialogChoice {
                label: "\"Can you walk?\"",
                next_node: 2,
                effect: &[DialogEffect::SetFlag(CampaignFlag::SavedTovan), DialogEffect::SetFlag(CampaignFlag::TovanHelped), DialogEffect::Trust(CampaignNpc::TovanReed, 2), DialogEffect::HealAndPotion],
            },
            CampaignDialogChoice {
                label: "\"I need your supplies.\"",
                next_node: 3,
                effect: &[DialogEffect::SetFlag(CampaignFlag::TookTovanSupplies), DialogEffect::SetFlag(CampaignFlag::TovanHelped), DialogEffect::GiveItem(ItemKind::HealthPotion)],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"Wish I could say goblins. Goblins are honest. They bite you and you know why. This was quiet. Too quiet. Then the bell rang under us — not above, not from camp. Under us.\" He shudders. \"We were climbing out. That's the part I keep remembering.\"",
        choices: &[],
        closing: Some("He presses a potion into your hands. \"Take it. I won't need it where I'm... well. You know.\""),
    },
    CampaignDialogNode {
        npc_line: "\"I can lie convincingly, if that helps. Walking's more ambitious.\" He manages a crooked grin. \"But if you're serious... there's a waystation further down. Blue lanterns. Get me there and I owe you.\"",
        choices: &[],
        closing: Some("He grips your arm. \"Thank you. Really. I was starting to think the last voice I'd hear would be my own.\""),
    },
    CampaignDialogNode {
        npc_line: "\"Sure. Take them. Leave me my jokes, though — they're the only thing not bleeding.\" He pushes the pack toward you with a shaking hand.",
        choices: &[],
        closing: Some("You take the supplies. He doesn't look at you as you leave."),
    },
];

static TOVAN_SAVED: [CampaignDialogNode; 1] = [
    CampaignDialogNode {
        npc_line: "\"Still here. Still breathing. Which is more than I expected ten minutes ago. Go on — I'll find my way back up. Someone has to tell Greyford what they're really digging toward.\"",
        choices: &[],
        closing: Some("He nods weakly. \"Good luck down there. Try not to end up like me.\""),
    },
];

static TOVAN_HELPED: [CampaignDialogNode; 1] = [
    CampaignDialogNode {
        npc_line: "\"I already told you everything I know. The bell. The silence. Something learned to ring back. Go — you're wasting time standing here.\"",
        choices: &[],
        closing: Some("He waves you on. \"Find out what's ringing. Someone has to.\""),
    },
];

// ══════════════════════════════════════════════════════════════════════════════
// JUN — Level 2: Lantern Tender
// ══════════════════════════════════════════════════════════════════════════════
//
// Jun mentions Lantern Oil. An NPC with a name override. She helps the player
// and gives the oil if the player helps relight the lanterns.

fn jun_dialog(flags: &HashSet<CampaignFlag>, _trust: i32) -> &'static [CampaignDialogNode] {
    if flags.contains(&CampaignFlag::AskedJunForOil) {
        &JUN_GAVE_OIL
    } else {
        &JUN_INTRO
    }
}

static JUN_INTRO: [CampaignDialogNode; 3] = [
    CampaignDialogNode {
        npc_line: "\"Careful with the blue flame.\" She adjusts the wick without looking up. \"It doesn't show where monsters are. It shows where the dark has noticed us.\"",
        choices: &[
            CampaignDialogChoice {
                label: "\"The lanterns are dying.\"",
                next_node: 1,
                effect: &[DialogEffect::SetFlag(CampaignFlag::AskedJunForOil), DialogEffect::GiveItem(ItemKind::LanternOil)],
            },
            CampaignDialogChoice {
                label: "\"Can you relight them?\"",
                next_node: 2,
                effect: &[DialogEffect::SetFlag(CampaignFlag::AskedJunForOil), DialogEffect::GiveItem(ItemKind::LanternOil)],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"Not dying. Hiding. Flame has instincts, same as flesh. Down here, even fire learns fear.\" She gestures at the nearest lantern — its blue light trembles, leaning away from something you can't see.",
        choices: &[],
        closing: Some("She presses a small vial into your hand. \"Lantern oil. Use it when the dark presses too close. It burns blue where ordinary fire lies.\""),
    },
    CampaignDialogNode {
        npc_line: "\"With oil, yes. With courage, no. I'm running short of both.\" She nods toward the dark corridors. \"There are three dead lanterns between here and the stairs. Light them. The flame remembers the route — it'll show you where the stone is lying.\"",
        choices: &[],
        closing: Some("She hands you a flask of oil. \"This should be enough. Find the lanterns. Light them. And don't trust any flame that burns yellow down here.\""),
    },
];

static JUN_GAVE_OIL: [CampaignDialogNode; 1] = [
    CampaignDialogNode {
        npc_line: "\"The oil I gave you — it'll burn blue for fifty turns. After that, you're on your own. So make them count.\" She smiles, but her eyes stay on the dark.",
        choices: &[],
        closing: Some("She touches the nearest lantern. The flame leans toward you, briefly. \"Stay close to the light.\""),
    },
];

// ══════════════════════════════════════════════════════════════════════════════
// EDDA SLATE — Level 2: Cartographer
// ══════════════════════════════════════════════════════════════════════════════

fn edda_dialog(flags: &HashSet<CampaignFlag>, _trust: i32) -> &'static [CampaignDialogNode] {
    if flags.contains(&CampaignFlag::AskedEddaForMap) {
        &EDDA_MAPPED
    } else {
        &EDDA_INTRO
    }
}

static EDDA_INTRO: [CampaignDialogNode; 4] = [
    CampaignDialogNode {
        npc_line: "\"If you're about to ask whether I'm lost, don't. I know exactly where I am. The problem is the room disagrees.\" She taps her map with a charcoal stick. The markings look recent — and uncertain.",
        choices: &[
            CampaignDialogChoice {
                label: "\"Your map is wrong?\"",
                next_node: 1,
                effect: &[DialogEffect::RevealMap],
            },
            CampaignDialogChoice {
                label: "\"Can you show me the way?\"",
                next_node: 2,
                effect: &[DialogEffect::SetFlag(CampaignFlag::AskedEddaForMap), DialogEffect::RevealMap],
            },
            CampaignDialogChoice {
                label: "\"What are you really mapping?\"",
                next_node: 3,
                effect: &[DialogEffect::SetFlag(CampaignFlag::AskedEddaAboutCrown)],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"My map is beautiful. Accurate, even — as of three hours ago. Unfortunately, the dungeon has poor manners. It keeps revising itself.\" She traces a wall that now has a door. \"That wasn't there when I drew this.\"",
        choices: &[],
        closing: Some("She shares her map anyway. \"Better than blindness, even if it lies a little.\""),
    },
    CampaignDialogNode {
        npc_line: "\"I can show you where the way was. If it still is, consider yourself blessed. If it isn't...\" she gestures vaguely, \"...then the dungeon has already decided where you're going.\"",
        choices: &[],
        closing: Some("She spreads the map flat. \"There. Stairs should be that direction. Probably.\""),
    },
    CampaignDialogNode {
        npc_line: "\"Absences. Missing doors. Rooms that remember being chapels. Stairs that move when frightened.\" She pauses. \"The dungeon doesn't shift like stone. It shifts like memory. Every delver who dies down here — the deep keeps a piece. More rooms. More hallways. More ways to get lost.\"",
        choices: &[],
        closing: Some("She looks at you differently now. \"You're going deeper, aren't you? Then remember: whatever question the Crown asks you — have an answer ready.\""),
    },
];

static EDDA_MAPPED: [CampaignDialogNode; 1] = [
    CampaignDialogNode {
        npc_line: "\"Still standing. Still wrong about which wall is where. But you're alive, which means my map wasn't entirely useless. Consider that your compliment.\"",
        choices: &[],
        closing: Some("She returns to her charts. \"Don't die down there. The dungeon has enough rooms.\""),
    },
];

// ══════════════════════════════════════════════════════════════════════════════
// VEY — Level 2: Merchant of the Ash Markets
// ══════════════════════════════════════════════════════════════════════════════

fn vey_dialog(_flags: &HashSet<CampaignFlag>, trust: i32) -> &'static [CampaignDialogNode] {
    if trust >= 2 && !_flags.contains(&CampaignFlag::VeyRewardClaimed) {
        &VEY_TRUSTED
    } else if trust >= 2 {
        &VEY_TRUSTED_DONE
    } else {
        &VEY_INTRO
    }
}

/// Vey trusts you enough to offer something real.
static VEY_TRUSTED: [CampaignDialogNode; 2] = [
    CampaignDialogNode {
        npc_line: "\"Alright. I've been holding this back — call it an investment in the future you.\" Vey pulls a scroll from beneath the counter.",
        choices: &[
            CampaignDialogChoice {
                label: "\"What is it?\"",
                next_node: 1,
                effect: &[DialogEffect::SetFlag(CampaignFlag::VeyRewardClaimed), DialogEffect::GiveItem(ItemKind::SmokeBomb)],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"Smoke bomb. A merchant's favorite — it makes problems go away without anyone getting angry at you.\" Vey slides it across the counter. \"I won't charge. Not yet. When you find the Crown, you'll remember who helped you.\"",
        choices: &[],
        closing: Some("The smoke bomb is heavy in your hand. \"Don't use it where you can't see the exit.\""),
    },
];

/// Vey already gave you the reward — nothing left to hand out.
static VEY_TRUSTED_DONE: [CampaignDialogNode; 1] = [
    CampaignDialogNode {
        npc_line: "\"I've already given you my best stock. You want more, come back with the Crown — then we'll really talk business.\"",
        choices: &[],
        closing: Some("Vey's smile doesn't waver. \"Good luck down there. You'll need it.\""),
    },
];

static VEY_INTRO: [CampaignDialogNode; 3] = [
    CampaignDialogNode {
        npc_line: "\"You have the look. First descent, yes? Still counting exits. Still thinking exits matter.\" Vey's smile doesn't quite reach their eyes. The stall behind them is immaculate — lanterns polished, potions arranged by colour.",
        choices: &[
            CampaignDialogChoice {
                label: "\"You sell to delvers?\"",
                next_node: 1,
                effect: &[],
            },
            CampaignDialogChoice {
                label: "\"Do you know about the Crown?\"",
                next_node: 2,
                effect: &[DialogEffect::Trust(CampaignNpc::Vey, 1)],
            },
        ],
        closing: None,
    },
    CampaignDialogNode {
        npc_line: "\"I sell to optimism. Delvers merely carry it.\" Vey gestures at the wares. \"Potions for the hopeful. Weapons for the realistic. Lantern oil for the ones who've already seen what's down there and came back anyway.\"",
        choices: &[],
        closing: Some("\"Take what you need. I'll be here when you need more. Assuming you come back. Assuming any of us do.\""),
    },
    CampaignDialogNode {
        npc_line: "\"Everyone knows about the Crown. The trick is admitting how much you'd pay to pretend you don't.\" Vey leans closer. \"The surface wants it. Not for power — for closure. Three hundred years without a king, and everyone blames the hole in the ground.\"",
        choices: &[],
        closing: Some("Vey's smile returns. \"Find the Crown. Find out what it asks. Then come back and tell me. That information is worth more than anything in this stall.\""),
    },
];

// ══════════════════════════════════════════════════════════════════════════════
// CAMPAIGN MENU / CHAPTER TEXT
// ══════════════════════════════════════════════════════════════════════════════

/// Chapter 1 intro text shown when starting a campaign.
pub static CHAPTER1_INTRO: &[&str] = &[
    "The bell rang from below.",
    "",
    "Not from the Rustguard outpost. Not from the watchtowers.",
    "From somewhere deep. Somewhere that shouldn't have bells.",
    "",
    "Greyford's Delvers' Compact sent three teams.",
    "Two came back. One of them screaming.",
    "",
    "Now they're sending you.",
    "",
    "Recover the missing bell-token.",
    "Find out what rang.",
    "And if the Crown is really down there...",
    "don't let it follow you back.",
];

/// Level 1 outro — shown after descending from Level 1.
pub static LEVEL1_OUTRO: &[&str] = &[
    "The gate seals behind you with a sound like a coffin lid.",
    "The air turns blue and cold. Somewhere ahead, a lantern gutters.",
    "",
    "Mara's last words echo: \"If you hear bells, don't follow them.\"",
];

/// Level 2 outro — shown after descending from Level 2.
pub static LEVEL2_OUTRO: &[&str] = &[
    "The stone groans and shifts. Stairs that were absent a moment ago",
    "now spiral downward into rust-red corridors.",
    "",
    "A distant bell tolls once. Then silence.",
    "The kind of silence that means something is listening.",
];

/// Chapter 1 ending — shown after Level 3 bell chamber resolution.
pub static CHAPTER1_ENDING: &[&str] = &[
    "",
    "The bell chamber falls silent.",
    "",
    "Below the broken bell, stairs continue downward — older than",
    "the Rustguard, older than the Compact, older than Greyford.",
    "",
    "Warm gold light pulses once. Very far below.",
    "Not a treasure gleam.",
    "A heartbeat.",
    "",
    "── End of Chapter I: The Bell Line ──",
    "    Chapter II: The Kingless Road",
    "",
    "The Crown knows you're coming now.",
    "And it is asking you a question.",
    "You can almost hear the words.",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_state_new_has_chapter1() {
        let state = CampaignState::new();
        assert_eq!(state.chapter, CampaignChapter::Chapter1);
        assert_eq!(state.level, 0);
        assert_eq!(state.objectives.len(), 1);
    }

    #[test]
    fn campaign_flag_set_and_check() {
        let mut state = CampaignState::new();
        assert!(!state.has_flag(CampaignFlag::SavedTovan));
        state.set_flag(CampaignFlag::SavedTovan);
        assert!(state.has_flag(CampaignFlag::SavedTovan));
    }

    #[test]
    fn campaign_trust_modify() {
        let mut state = CampaignState::new();
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), 0);
        state.modify_trust(CampaignNpc::MaraVoss, 2);
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), 2);
        state.modify_trust(CampaignNpc::MaraVoss, -3);
        assert_eq!(state.trust_of(CampaignNpc::MaraVoss), -1);
    }

    #[test]
    fn campaign_level_specs_exist() {
        assert!(chapter1_level_spec(0).is_some());
        assert!(chapter1_level_spec(1).is_some());
        assert!(chapter1_level_spec(2).is_some());
        assert!(chapter1_level_spec(3).is_none());
    }

    #[test]
    fn objective_display() {
        let obj = CampaignObjective::RecoverBellToken;
        assert!(obj.display().contains("bell-token"));

        let obj2 = CampaignObjective::RelightLanterns { lit: 1, needed: 3 };
        assert!(obj2.display().contains("1/3"));

        let obj3 = CampaignObjective::SilenceTheBell;
        assert!(obj3.display().contains("bell chamber"));
    }
}
