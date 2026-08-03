use serde::{Deserialize, Serialize};

/// Character stats for player and monsters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MonsterKind {
    Rat,
    Bat,
    Goblin,
    Skeleton,
    Orc,
    Spider,
    Wraith,
    Slime,
    Mimic,
    Necromancer,
    Sentinel,
    Guardian,
}

impl MonsterKind {
    pub fn stats(&self) -> Stats {
        match self {
            MonsterKind::Rat => Stats {
                hp: 4,
                max_hp: 4,
                attack: 2,
                defense: 0,
            },
            MonsterKind::Bat => Stats {
                hp: 3,
                max_hp: 3,
                attack: 2,
                defense: 0,
            },
            MonsterKind::Goblin => Stats {
                hp: 8,
                max_hp: 8,
                attack: 3,
                defense: 1,
            },
            MonsterKind::Skeleton => Stats {
                hp: 12,
                max_hp: 12,
                attack: 4,
                defense: 0,
            },
            MonsterKind::Orc => Stats {
                hp: 18,
                max_hp: 18,
                attack: 6,
                defense: 2,
            },
            MonsterKind::Spider => Stats {
                hp: 10,
                max_hp: 10,
                attack: 3,
                defense: 1,
            },
            MonsterKind::Wraith => Stats {
                hp: 14,
                max_hp: 14,
                attack: 5,
                defense: 3,
            },
            MonsterKind::Slime => Stats {
                hp: 6,
                max_hp: 6,
                attack: 2,
                defense: 0,
            },
            MonsterKind::Mimic => Stats {
                hp: 10,
                max_hp: 10,
                attack: 5,
                defense: 2,
            },
            MonsterKind::Necromancer => Stats {
                hp: 16,
                max_hp: 16,
                attack: 4,
                defense: 2,
            },
            MonsterKind::Sentinel => Stats {
                hp: 22,
                max_hp: 22,
                attack: 8,
                defense: 3,
            },
            MonsterKind::Guardian => Stats {
                hp: 40,
                max_hp: 40,
                attack: 10,
                defense: 4,
            },
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            MonsterKind::Rat => 'r',
            MonsterKind::Bat => 'b',
            MonsterKind::Goblin => 'g',
            MonsterKind::Skeleton => 's',
            MonsterKind::Orc => 'O',
            MonsterKind::Spider => 'S',
            MonsterKind::Wraith => 'W',
            MonsterKind::Slime => 'm',
            MonsterKind::Mimic => 'M',
            MonsterKind::Necromancer => 'N',
            MonsterKind::Sentinel => 'T',
            MonsterKind::Guardian => 'G',
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            MonsterKind::Rat => "rat",
            MonsterKind::Bat => "bat",
            MonsterKind::Goblin => "goblin",
            MonsterKind::Skeleton => "skeleton",
            MonsterKind::Orc => "orc",
            MonsterKind::Spider => "spider",
            MonsterKind::Wraith => "wraith",
            MonsterKind::Slime => "slime",
            MonsterKind::Mimic => "mimic",
            MonsterKind::Necromancer => "necromancer",
            MonsterKind::Sentinel => "sentinel",
            MonsterKind::Guardian => "guardian",
        }
    }

    pub fn act_chance(&self) -> f64 {
        match self {
            MonsterKind::Rat => 0.7,
            MonsterKind::Bat => 1.0,
            MonsterKind::Goblin => 0.5,
            MonsterKind::Skeleton => 0.75,
            MonsterKind::Orc => 1.0,
            MonsterKind::Spider => 0.6,
            MonsterKind::Wraith => 0.8,
            MonsterKind::Slime => 0.6,
            MonsterKind::Mimic => 0.0, // stationary until revealed
            MonsterKind::Necromancer => 0.7,
            MonsterKind::Sentinel => 0.0, // stationary guardian
            MonsterKind::Guardian => 1.0, // always acts
        }
    }

    pub fn min_depth(&self) -> u32 {
        match self {
            MonsterKind::Rat | MonsterKind::Bat | MonsterKind::Goblin | MonsterKind::Slime => 0,
            MonsterKind::Skeleton | MonsterKind::Mimic => 1,
            MonsterKind::Orc | MonsterKind::Necromancer => 2,
            MonsterKind::Spider | MonsterKind::Sentinel => 3,
            MonsterKind::Wraith => 4,
            MonsterKind::Guardian => 4,
        }
    }

    pub fn weight(&self) -> u32 {
        match self {
            MonsterKind::Rat => 4,
            MonsterKind::Bat => 3,
            MonsterKind::Goblin => 5,
            MonsterKind::Skeleton => 3,
            MonsterKind::Orc => 2,
            MonsterKind::Spider => 2,
            MonsterKind::Wraith => 1,
            MonsterKind::Slime => 3,
            MonsterKind::Mimic => 2,
            MonsterKind::Necromancer => 2,
            MonsterKind::Sentinel => 1,
            MonsterKind::Guardian => 0, // never random, always placed
        }
    }

    /// XP awarded for killing this monster kind.
    pub fn xp_value(&self) -> u32 {
        match self {
            MonsterKind::Rat => 5,
            MonsterKind::Bat => 8,
            MonsterKind::Goblin => 15,
            MonsterKind::Skeleton => 20,
            MonsterKind::Orc => 35,
            MonsterKind::Spider => 25,
            MonsterKind::Wraith => 50,
            MonsterKind::Slime => 10,
            MonsterKind::Mimic => 30,
            MonsterKind::Necromancer => 45,
            MonsterKind::Sentinel => 40,
            MonsterKind::Guardian => 100,
        }
    }

    /// Gold dropped on kill (random range min‑max).
    pub fn gold_range(&self) -> (u32, u32) {
        match self {
            MonsterKind::Rat => (0, 2),
            MonsterKind::Bat => (0, 1),
            MonsterKind::Goblin => (2, 8),
            MonsterKind::Skeleton => (1, 5),
            MonsterKind::Orc => (5, 15),
            MonsterKind::Spider => (3, 10),
            MonsterKind::Wraith => (5, 20),
            MonsterKind::Slime => (0, 3),
            MonsterKind::Mimic => (10, 25),
            MonsterKind::Necromancer => (10, 30),
            MonsterKind::Sentinel => (8, 20),
            MonsterKind::Guardian => (30, 60),
        }
    }

    /// Random monster kind weighted by allowed depth.
    pub fn random_for_depth(rng: &mut impl rand::Rng, depth: u32) -> Self {
        let all = [
            MonsterKind::Rat,
            MonsterKind::Bat,
            MonsterKind::Goblin,
            MonsterKind::Skeleton,
            MonsterKind::Orc,
            MonsterKind::Spider,
            MonsterKind::Wraith,
            MonsterKind::Slime,
            MonsterKind::Mimic,
            MonsterKind::Necromancer,
            MonsterKind::Sentinel,
            // Guardian is always placed explicitly, never random
        ];
        let eligible: Vec<MonsterKind> = all
            .iter()
            .filter(|k| k.min_depth() <= depth)
            .copied()
            .collect();
        let total: u32 = eligible.iter().map(|k| k.weight()).sum();
        if total == 0 {
            return eligible[0];
        }
        let roll = rng.gen_range(0..total);
        let mut accum = 0;
        for &k in &eligible {
            accum += k.weight();
            if roll < accum {
                return k;
            }
        }
        eligible[0]
    }
}

/// A monster on the map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Monster {
    pub x: i32,
    pub y: i32,
    pub kind: MonsterKind,
    pub stats: Stats,
    pub alive: bool,
    pub poisoned: bool,
    pub poison_ticks: i32,
    /// Turns until the monster can use its special ability again.
    #[serde(default)]
    pub attack_cooldown: i32,
    /// Whether the monster has been awakened (mimics / sentinels activate
    /// permanently once the player is adjacent or attacks them).
    #[serde(default)]
    pub awakened: bool,
    /// A heavy melee attack has been announced and will land on the next action.
    #[serde(default)]
    pub windup: u8,
    /// Skips the monster's next action after a sufficiently forceful hit.
    #[serde(default)]
    pub staggered: u8,
    /// Last place the monster saw the player; valid while `alert_turns` is nonzero.
    #[serde(default)]
    pub last_seen_player_x: i32,
    #[serde(default)]
    pub last_seen_player_y: i32,
    #[serde(default)]
    pub alert_turns: u8,
}

impl Monster {
    pub fn new(x: i32, y: i32, kind: MonsterKind) -> Self {
        let stats = kind.stats();
        Self {
            x,
            y,
            kind,
            stats,
            alive: true,
            poisoned: false,
            poison_ticks: 0,
            attack_cooldown: 0,
            awakened: false,
            windup: 0,
            staggered: 0,
            last_seen_player_x: 0,
            last_seen_player_y: 0,
            alert_turns: 0,
        }
    }
}

/// A projectile fired by a monster (e.g. necromancer bolt).
/// Moves in a fixed direction each turn.  Damage is applied on collision
/// with the player; projectiles stop on walls/doors or after their range
/// expires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    pub x: i32,
    pub y: i32,
    /// Fixed direction of travel (one of -1, 0, 1 for each axis).
    pub dx: i32,
    pub dy: i32,
    /// Damage dealt on hit.
    pub damage: i32,
    /// Remaining lifetime in turns.
    pub lifetime: i32,
}

/// Item types found on dungeon floors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    HealthPotion,
    LargeHealthPotion,
    Antidote,
    LanternOil,
    ScrollOfFire,
    ScrollOfConfusion,
    ScrollOfBlink,
    SmokeBomb,
    RustySword,
    Longsword,
    Warhammer,
    EnchantmentScroll,
    LeatherArmor,
    Chainmail,
    Shield,
    CrownOfDepth,
    LoreFragment,
    /// Campaign quest item: the Rustguard bell-token.
    BellToken,
    IronKey,
    Emberbrand,
    TideglassMail,
    Graveward,
}

impl ItemKind {
    pub fn name(&self) -> &'static str {
        match self {
            ItemKind::HealthPotion => "health potion",
            ItemKind::LargeHealthPotion => "large health potion",
            ItemKind::Antidote => "antidote",
            ItemKind::LanternOil => "lantern oil",
            ItemKind::ScrollOfFire => "scroll of fire",
            ItemKind::ScrollOfConfusion => "scroll of confusion",
            ItemKind::ScrollOfBlink => "scroll of blink",
            ItemKind::SmokeBomb => "smoke bomb",
            ItemKind::RustySword => "rusty sword",
            ItemKind::Longsword => "longsword",
            ItemKind::Warhammer => "warhammer",
            ItemKind::EnchantmentScroll => "enchantment scroll",
            ItemKind::LeatherArmor => "leather armor",
            ItemKind::Chainmail => "chainmail",
            ItemKind::Shield => "shield",
            ItemKind::CrownOfDepth => "Crown of Depth",
            ItemKind::LoreFragment => "lore fragment",
            ItemKind::BellToken => "bell-token",
            ItemKind::IronKey => "iron key",
            ItemKind::Emberbrand => "Emberbrand",
            ItemKind::TideglassMail => "Tideglass Mail",
            ItemKind::Graveward => "Graveward",
        }
    }

    pub fn symbol(&self) -> char {
        match self {
            ItemKind::HealthPotion
            | ItemKind::LargeHealthPotion
            | ItemKind::Antidote
            | ItemKind::LanternOil => '!',
            ItemKind::ScrollOfFire
            | ItemKind::ScrollOfConfusion
            | ItemKind::ScrollOfBlink
            | ItemKind::EnchantmentScroll
            | ItemKind::LoreFragment => '?',
            ItemKind::SmokeBomb => '!',
            ItemKind::RustySword | ItemKind::Longsword | ItemKind::Warhammer => '/',
            ItemKind::LeatherArmor | ItemKind::Chainmail | ItemKind::Shield => '[',
            ItemKind::CrownOfDepth => '*',
            ItemKind::BellToken => '\u{25C6}',
            ItemKind::IronKey => 'k',
            ItemKind::Emberbrand => '/',
            ItemKind::TideglassMail | ItemKind::Graveward => '[',
        }
    }

    pub fn is_weapon(&self) -> bool {
        matches!(
            self,
            ItemKind::RustySword | ItemKind::Longsword | ItemKind::Warhammer | ItemKind::Emberbrand
        )
    }

    pub fn is_armor(&self) -> bool {
        matches!(
            self,
            ItemKind::LeatherArmor
                | ItemKind::Chainmail
                | ItemKind::Shield
                | ItemKind::TideglassMail
                | ItemKind::Graveward
        )
    }

    pub fn is_equippable(&self) -> bool {
        self.is_weapon() || self.is_armor()
    }

    pub fn requires_identification(&self) -> bool {
        matches!(
            self,
            ItemKind::HealthPotion
                | ItemKind::LargeHealthPotion
                | ItemKind::Antidote
                | ItemKind::LanternOil
                | ItemKind::ScrollOfFire
                | ItemKind::ScrollOfConfusion
                | ItemKind::ScrollOfBlink
                | ItemKind::SmokeBomb
                | ItemKind::EnchantmentScroll
        )
    }

    pub fn display_name(self, player: &Player) -> &'static str {
        if player.knows_item(self) {
            self.name()
        } else if matches!(
            self,
            ItemKind::HealthPotion
                | ItemKind::LargeHealthPotion
                | ItemKind::Antidote
                | ItemKind::LanternOil
        ) {
            "unidentified potion"
        } else if self == ItemKind::SmokeBomb {
            "unidentified bomb"
        } else {
            "unidentified scroll"
        }
    }

    pub fn max_durability(&self) -> i32 {
        match self {
            ItemKind::RustySword => 24,
            ItemKind::Longsword => 36,
            ItemKind::Warhammer => 30,
            ItemKind::Emberbrand => 0,
            _ => 0,
        }
    }

    pub fn heal_amount(&self) -> i32 {
        match self {
            ItemKind::HealthPotion => 10,
            ItemKind::LargeHealthPotion => 25,
            _ => 0,
        }
    }

    pub fn attack_bonus(&self) -> i32 {
        match self {
            ItemKind::RustySword => 1,
            ItemKind::Longsword => 3,
            ItemKind::Warhammer => 5,
            ItemKind::Emberbrand => 4,
            _ => 0,
        }
    }

    pub fn defense_bonus(&self) -> i32 {
        match self {
            ItemKind::LeatherArmor => 1,
            ItemKind::Chainmail => 3,
            ItemKind::Shield => 2,
            ItemKind::TideglassMail => 2,
            ItemKind::Graveward => 4,
            _ => 0,
        }
    }

    pub fn min_depth(&self) -> u32 {
        match self {
            ItemKind::HealthPotion => 0,
            ItemKind::LargeHealthPotion => 2,
            ItemKind::Antidote => 1,
            ItemKind::LanternOil => 2,
            ItemKind::ScrollOfFire => 1,
            ItemKind::ScrollOfConfusion => 2,
            ItemKind::ScrollOfBlink => 2,
            ItemKind::SmokeBomb => 1,
            ItemKind::RustySword => 0,
            ItemKind::Longsword => 2,
            ItemKind::Warhammer => 4,
            ItemKind::EnchantmentScroll => 3,
            ItemKind::LeatherArmor => 0,
            ItemKind::Chainmail => 3,
            ItemKind::Shield => 1,
            ItemKind::CrownOfDepth => 4,
            ItemKind::LoreFragment => 0,
            ItemKind::BellToken => 0,
            ItemKind::IronKey => 2,
            ItemKind::Emberbrand => 2,
            ItemKind::TideglassMail => 3,
            ItemKind::Graveward => 4,
        }
    }

    /// Short description line displayed in the inventory tooltip.
    pub fn description(&self) -> &'static str {
        match self {
            ItemKind::HealthPotion => "Restores 10 HP when consumed.",
            ItemKind::LargeHealthPotion => "Restores 25 HP when consumed.",
            ItemKind::Antidote => "Cures poison and restores 5 HP.",
            ItemKind::LanternOil => "+3 FOV radius for 50 turns.",
            ItemKind::ScrollOfFire => "Deals 8 fire damage to all visible monsters.",
            ItemKind::ScrollOfConfusion => "Randomly teleports all visible monsters.",
            ItemKind::ScrollOfBlink => "Teleports you to a random room.",
            ItemKind::SmokeBomb => "Teleports adjacent monsters away.",
            ItemKind::RustySword => "A worn blade whose Lunge closes a two-tile gap.",
            ItemKind::Longsword => "A balanced sword built for a three-cell Sweep.",
            ItemKind::Warhammer => "A crushing hammer whose Earthbreak drives a line back.",
            ItemKind::EnchantmentScroll => "Upgrades equipped weapon by +2 ATK.",
            ItemKind::LeatherArmor => "Basic leather protection.",
            ItemKind::Chainmail => "Sturdy interlocking chain links.",
            ItemKind::Shield => "A sturdy shield; unarmed, it enables Shield Bash.",
            ItemKind::CrownOfDepth => {
                "It is not gold. It is something that learned to look like gold."
            }
            ItemKind::LoreFragment => "A previous Crown-seeker's journal entry.",
            ItemKind::BellToken => "A heavy iron token stamped with the Rustguard bell insignia.",
            ItemKind::IronKey => "Opens one locked dungeon door, then breaks in the lock.",
            ItemKind::Emberbrand => "A named blade whose Cinder Arc burns a broad path.",
            ItemKind::TideglassMail => "Cold blue links that move through water without drag.",
            ItemKind::Graveward => "Funerary plate that turns aside spectral claws.",
        }
    }

    /// Broad category for inventory grouping.
    pub fn category_label(&self) -> &'static str {
        match self {
            ItemKind::HealthPotion
            | ItemKind::LargeHealthPotion
            | ItemKind::Antidote
            | ItemKind::LanternOil => "Consumable — Potion",
            ItemKind::ScrollOfFire
            | ItemKind::ScrollOfConfusion
            | ItemKind::ScrollOfBlink
            | ItemKind::EnchantmentScroll => "Consumable — Scroll",
            ItemKind::SmokeBomb => "Consumable — Bomb",
            ItemKind::RustySword | ItemKind::Longsword | ItemKind::Warhammer => "Weapon",
            ItemKind::LeatherArmor | ItemKind::Chainmail | ItemKind::Shield => "Armor",
            ItemKind::CrownOfDepth | ItemKind::BellToken => "Quest Item",
            ItemKind::IronKey => "Utility — Key",
            ItemKind::Emberbrand | ItemKind::TideglassMail | ItemKind::Graveward => {
                "Named Artifact"
            }
            ItemKind::LoreFragment => "Lore",
        }
    }

    /// Lines describing the stat effects for the inventory tooltip.
    pub fn stat_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.is_weapon() {
            let atk = self.attack_bonus();
            lines.push(format!("ATK +{}", atk));
        }
        if self.is_armor() {
            let def = self.defense_bonus();
            lines.push(format!("DEF +{}", def));
        }
        match self {
            ItemKind::HealthPotion => lines.push("Heal 10 HP".into()),
            ItemKind::LargeHealthPotion => lines.push("Heal 25 HP".into()),
            ItemKind::Antidote => lines.push("Cure poison".into()),
            ItemKind::LanternOil => lines.push("+3 FOV, 50 turns".into()),
            ItemKind::ScrollOfFire => lines.push("8 AoE damage".into()),
            ItemKind::ScrollOfConfusion => lines.push("Teleport foes".into()),
            ItemKind::ScrollOfBlink => lines.push("Teleport self".into()),
            ItemKind::SmokeBomb => lines.push("Push foes away".into()),
            ItemKind::EnchantmentScroll => lines.push("Weapon +2 ATK".into()),
            ItemKind::CrownOfDepth => lines.push("Win condition".into()),
            ItemKind::LoreFragment => lines.push("Read story".into()),
            ItemKind::BellToken => lines.push("Campaign key item".into()),
            ItemKind::IronKey => lines.push("Opens one locked door".into()),
            ItemKind::RustySword => lines.push("T: Lunge (2 STA), reach 2".into()),
            ItemKind::Longsword => lines.push("T: Sweep (3 STA), three cells".into()),
            ItemKind::Warhammer => lines.push("T: Earthbreak (4 STA), line + push".into()),
            ItemKind::Shield => lines.push("T unarmed: Shield Bash (2 STA)".into()),
            ItemKind::Emberbrand => {
                lines.push("+2 damage vs spiders/slimes".into());
                lines.push("T: Cinder Arc (4 STA), burns webs".into());
            }
            ItemKind::TideglassMail => lines.push("Ignore shallow-water delay".into()),
            ItemKind::Graveward => lines.push("Reduce wraith damage".into()),
            _ => {}
        }
        lines
    }

    /// Random item kind weighted by allowed depth.
    /// The Crown of Depth is always placed explicitly and never spawned randomly.
    pub fn random_for_depth(rng: &mut impl rand::Rng, depth: u32) -> Self {
        let all = [
            ItemKind::HealthPotion,
            ItemKind::LargeHealthPotion,
            ItemKind::Antidote,
            ItemKind::LanternOil,
            ItemKind::ScrollOfFire,
            ItemKind::ScrollOfConfusion,
            ItemKind::ScrollOfBlink,
            ItemKind::SmokeBomb,
            ItemKind::RustySword,
            ItemKind::Longsword,
            ItemKind::Warhammer,
            ItemKind::EnchantmentScroll,
            ItemKind::LeatherArmor,
            ItemKind::Chainmail,
            ItemKind::Shield,
            ItemKind::LoreFragment,
        ];
        let eligible: Vec<ItemKind> = all
            .iter()
            .filter(|k| k.min_depth() <= depth)
            .copied()
            .collect();
        let idx = rng.gen_range(0..eligible.len());
        eligible[idx]
    }
}

/// An item lying on the dungeon floor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub x: i32,
    pub y: i32,
    pub kind: ItemKind,
    /// Whether this item is cursed (auto‑equips, can't be removed without scroll).
    #[serde(default)]
    pub cursed: bool,
    /// Remaining uses before the item breaks (0 = infinite durability).
    #[serde(default)]
    pub durability: i32,
}

impl Item {
    pub fn new(x: i32, y: i32, kind: ItemKind) -> Self {
        let durability = if kind.is_weapon() { 30 } else { 0 };
        Self {
            x,
            y,
            kind,
            cursed: false,
            durability,
        }
    }
}

/// The player character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Discipline {
    Vigor,
    Finesse,
    Resolve,
}

impl Discipline {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vigor => "Vigor",
            Self::Finesse => "Finesse",
            Self::Resolve => "Resolve",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Vigor => "+6 maximum health and heal 6",
            Self::Finesse => "+1 attack and +1 maximum stamina",
            Self::Resolve => "+1 defense and stronger counters",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechniqueKind {
    Kick,
    ShieldBash,
    Lunge,
    Sweep,
    Earthbreak,
    CinderArc,
}

impl TechniqueKind {
    pub fn from_loadout(player: &Player) -> Self {
        match player.weapon {
            Some(ItemKind::RustySword) => Self::Lunge,
            Some(ItemKind::Longsword) => Self::Sweep,
            Some(ItemKind::Warhammer) => Self::Earthbreak,
            Some(ItemKind::Emberbrand) => Self::CinderArc,
            _ if player.armor == Some(ItemKind::Shield) => Self::ShieldBash,
            _ => Self::Kick,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Kick => "Kick",
            Self::ShieldBash => "Shield Bash",
            Self::Lunge => "Lunge",
            Self::Sweep => "Sweep",
            Self::Earthbreak => "Earthbreak",
            Self::CinderArc => "Cinder Arc",
        }
    }

    pub const fn cost(self) -> u8 {
        match self {
            Self::Kick | Self::ShieldBash | Self::Lunge => 2,
            Self::Sweep => 3,
            Self::Earthbreak | Self::CinderArc => 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub x: i32,
    pub y: i32,
    pub stats: Stats,
    pub alive: bool,
    pub depth: u32,
    pub inventory: Vec<ItemKind>,
    pub weapon: Option<ItemKind>,
    pub armor: Option<ItemKind>,
    pub poisoned: bool,
    pub poison_ticks: i32,
    /// Turns remaining on lantern oil FOV bonus (0 = inactive).
    #[serde(default)]
    pub lantern_turns: i32,
    /// Whether the Crown has been picked up (triggers return-journey pressure).
    #[serde(default)]
    pub crown_held: bool,
    /// Experience points earned.
    #[serde(default)]
    pub xp: u32,
    /// Player level.
    #[serde(default = "default_level")]
    pub level: u32,
    /// Gold coins carried.
    #[serde(default)]
    pub gold: u32,
    /// A consumable assigned to the in-game quick-use key.
    #[serde(default)]
    pub quick_item: Option<ItemKind>,
    /// Waiting grants a one-turn defensive stance.
    #[serde(default)]
    pub guarding: bool,
    /// Temporary camp/shrine offensive preparation.
    #[serde(default)]
    pub focus_turns: i32,
    /// Temporary defense and poison protection.
    #[serde(default)]
    pub ward_turns: i32,
    #[serde(default)]
    pub weapon_durability: i32,
    #[serde(default)]
    pub weapon_cursed: bool,
    #[serde(default)]
    pub armor_cursed: bool,
    #[serde(default = "default_stamina")]
    pub stamina: u8,
    #[serde(default = "default_stamina")]
    pub max_stamina: u8,
    #[serde(default)]
    pub evasion_turns: u8,
    /// A stable block primes a stronger strike until the next turn ends.
    #[serde(default)]
    pub counter_turns: u8,
    #[serde(default)]
    pub vigor_rank: u8,
    #[serde(default)]
    pub finesse_rank: u8,
    #[serde(default)]
    pub resolve_rank: u8,
    #[serde(default)]
    pub pending_level_ups: u8,
    #[serde(default)]
    pub identified_items: std::collections::HashSet<ItemKind>,
}

fn default_level() -> u32 {
    1
}

pub const MAX_STAMINA: u8 = 8;
pub const MAX_STAMINA_CAP: u8 = 12;

fn default_stamina() -> u8 {
    MAX_STAMINA
}

impl Player {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            stats: Stats {
                hp: 30,
                max_hp: 30,
                attack: 5,
                defense: 2,
            },
            alive: true,
            depth: 0,
            inventory: Vec::new(),
            weapon: None,
            armor: None,
            poisoned: false,
            poison_ticks: 0,
            lantern_turns: 0,
            crown_held: false,
            xp: 0,
            level: 1,
            gold: 0,
            quick_item: None,
            guarding: false,
            focus_turns: 0,
            ward_turns: 0,
            weapon_durability: 0,
            weapon_cursed: false,
            armor_cursed: false,
            stamina: MAX_STAMINA,
            max_stamina: MAX_STAMINA,
            evasion_turns: 0,
            counter_turns: 0,
            vigor_rank: 0,
            finesse_rank: 0,
            resolve_rank: 0,
            pending_level_ups: 0,
            identified_items: std::collections::HashSet::new(),
        }
    }

    pub fn total_attack(&self) -> i32 {
        let bonus: i32 = self.weapon.map(|w| w.attack_bonus()).unwrap_or(0);
        self.stats.attack + bonus + i32::from(self.focus_turns > 0)
    }

    pub fn spend_stamina(&mut self, amount: u8) -> bool {
        let enough = self.stamina >= amount;
        self.stamina = self.stamina.saturating_sub(amount);
        enough
    }

    pub fn recover_stamina(&mut self, amount: u8) {
        self.stamina = self.stamina.saturating_add(amount).min(self.max_stamina);
    }

    pub fn apply_discipline(&mut self, discipline: Discipline) {
        match discipline {
            Discipline::Vigor => {
                self.vigor_rank = self.vigor_rank.saturating_add(1);
                self.stats.max_hp += 6;
                self.stats.hp = (self.stats.hp + 6).min(self.stats.max_hp);
            }
            Discipline::Finesse => {
                self.finesse_rank = self.finesse_rank.saturating_add(1);
                self.stats.attack += 1;
                self.max_stamina = self.max_stamina.saturating_add(1).min(MAX_STAMINA_CAP);
                self.stamina = self.max_stamina;
            }
            Discipline::Resolve => {
                self.resolve_rank = self.resolve_rank.saturating_add(1);
                self.stats.defense += 1;
            }
        }
    }

    pub fn knows_item(&self, kind: ItemKind) -> bool {
        !kind.requires_identification() || self.identified_items.contains(&kind)
    }

    pub fn total_defense(&self) -> i32 {
        let bonus: i32 = self.armor.map(|a| a.defense_bonus()).unwrap_or(0);
        self.stats.defense + bonus + i32::from(self.guarding) * 2 + i32::from(self.ward_turns > 0)
    }

    /// XP needed to reach the next level.
    pub fn xp_to_next(&self) -> u32 {
        self.level * 20 + 10
    }

    /// Try to level up. Returns true if a level was gained.
    pub fn check_level_up(&mut self) -> bool {
        let needed = self.xp_to_next();
        if self.xp >= needed {
            self.xp -= needed;
            self.level += 1;
            self.stats.max_hp += 5;
            self.stats.hp = self.stats.max_hp;
            self.stats.attack += 2;
            true
        } else {
            false
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONVERSATION SYSTEM
// ══════════════════════════════════════════════════════════════════════════════

/// A single choice the player can make in a branching conversation.
#[derive(Debug, Clone, Copy)]
pub struct TalkChoice {
    /// Short label shown to the player (e.g. "What question?")
    pub label: &'static str,
    /// Index of the next TalkNode in the talk tree after this choice.
    /// The special value usize::MAX means the conversation ends.
    pub next_node: usize,
}

/// One node in a branching conversation tree.
#[derive(Debug, Clone, Copy)]
pub struct TalkNode {
    /// What the NPC says at this point.
    pub npc_line: &'static str,
    /// Player choices (empty = leaf node, conversation ends here).
    pub choices: &'static [TalkChoice],
    /// If present, the NPC says this before the conversation ends
    /// (after the player makes a choice, or immediately if no choices).
    pub closing: Option<&'static str>,
}

// ══════════════════════════════════════════════════════════════════════════════
// NPCs
// ══════════════════════════════════════════════════════════════════════════════

/// Friendly NPC types found in the dungeon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcKind {
    // ── Gameplay-effective NPCs (branching conversation + gameplay effect) ──
    /// Wounded delver — offers a free potion and a tip.
    WoundedDelver,
    /// Cartographer — reveals the entire floor map.
    Cartographer,
    /// Merchant — trades items with the player.
    Merchant,
    /// Shrine Keeper — offers risky bargains.
    ShrineKeeper,

    // ── Camp NPCs (branching conversation, no gameplay effect) ──
    /// Watch-captain of a camp — delivers warnings and lore about guarding the deep.
    WatchCaptain,
    /// A Rustguard soldier — weary, holding a line against the dark.
    Rustguard,

    // ── Camp NPCs (simple one-liner, no gameplay effect) ──
    /// Cook at a delver waystation — offers warmth not wares.
    Cook,
    /// Quartermaster — keeps supplies and records.
    Quartermaster,
    /// Cartographer's apprentice — learning the dungeon's shifting ways.
    ApprenticeMapper,
    /// A scavenger picking through the dungeon's leavings.
    Scavenger,
    /// A delver who lost their way or their nerve.
    LostDelver,
    /// A pilgrim seeking the Crown for faith.
    Pilgrim,
    /// A doubter among the faithful.
    Doubter,
    /// A lookout keeping watch on darkness.
    Lookout,
    /// Tends the blue lanterns that mark safe ground.
    LanternTender,
    /// A survivor from an abandoned camp.
    CampSurvivor,
}

impl NpcKind {
    pub fn symbol(&self) -> char {
        match self {
            NpcKind::WoundedDelver => 'h',
            NpcKind::Cartographer => 'c',
            NpcKind::Merchant => 'M',
            NpcKind::ShrineKeeper => 'S',
            NpcKind::WatchCaptain => 'C',
            NpcKind::Rustguard => 'G',
            NpcKind::Cook => 'k',
            NpcKind::Quartermaster => 'Q',
            NpcKind::ApprenticeMapper => 'm',
            NpcKind::Scavenger => 'v',
            NpcKind::LostDelver => 'd',
            NpcKind::Pilgrim => 'p',
            NpcKind::Doubter => 'D',
            NpcKind::Lookout => 'L',
            NpcKind::LanternTender => 'l',
            NpcKind::CampSurvivor => 'H',
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            NpcKind::WoundedDelver => "Wounded Delver",
            NpcKind::Cartographer => "Cartographer",
            NpcKind::Merchant => "Merchant",
            NpcKind::ShrineKeeper => "Shrine Keeper",
            NpcKind::WatchCaptain => "Watch-Captain",
            NpcKind::Rustguard => "Rustguard",
            NpcKind::Cook => "Cook",
            NpcKind::Quartermaster => "Quartermaster",
            NpcKind::ApprenticeMapper => "Apprentice Mapper",
            NpcKind::Scavenger => "Scavenger",
            NpcKind::LostDelver => "Lost Delver",
            NpcKind::Pilgrim => "Pilgrim",
            NpcKind::Doubter => "Doubter",
            NpcKind::Lookout => "Lookout",
            NpcKind::LanternTender => "Lantern Tender",
            NpcKind::CampSurvivor => "Survivor",
        }
    }

    /// Greeting spoken when the player first talks to this NPC (legacy static dialog).
    /// For branching NPCs, this is only used as a fallback; prefer `talk_tree()`.
    pub fn dialog(&self) -> &'static str {
        match self {
            NpcKind::WoundedDelver => "Cough... take this potion, friend. I won't make it back.\nThe Crown lies deepest — beware the Guardian.",
            NpcKind::Cartographer => "I've mapped this floor. Let me share what I know.\n*Reveals the dungeon layout*",
            NpcKind::Merchant => "Greetings, traveler. I trade in rare wares.\nWhat would you offer?",
            NpcKind::ShrineKeeper => "The ancient shrine hums with power.\nA bargain awaits — but nothing comes free.",
            NpcKind::WatchCaptain => "Keep your torch low. The Crown notices light before it notices blood.",
            NpcKind::Rustguard => "They test us every few hours. Rats first. Goblins after. Then the things the Crown has already spoken to.",
            NpcKind::Cook => "Stew's hot. Don't ask what's in it.",
            NpcKind::Quartermaster => "Lantern oil, rations, bandages. Mark your name if you take anything.",
            NpcKind::ApprenticeMapper => "The master says the dungeon breathes. I used to think that was poetry.",
            NpcKind::Scavenger => "One delver's bones are another's fortune. That's the truth of it.",
            NpcKind::LostDelver => "I was supposed to turn back three rooms ago. Which way was that?",
            NpcKind::Pilgrim => "The Crown asks a different question to every soul. It asked me why I was still afraid of the dark.",
            NpcKind::Doubter => "They say the Crown chose the old king. I say the old king chose poorly.",
            NpcKind::Lookout => "Nothing moving. Nothing I can see, anyway.",
            NpcKind::LanternTender => "The flame burns blue when something's close. It's been blue for an hour.",
            NpcKind::CampSurvivor => "Six of us came down. Six bedrolls. Five packs. No bodies. I don't know what happened.",
        }
    }

    /// Returns the branching conversation tree for this NPC, if any.
    /// Only the major NPCs have branching trees; flavor NPCs return None.
    pub fn talk_tree(&self) -> Option<&'static [TalkNode]> {
        match self {
            NpcKind::WoundedDelver => Some(&WOUNDED_DELVER_TREE),
            NpcKind::Cartographer => Some(&CARTOGRAPHER_TREE),
            NpcKind::Merchant => Some(&MERCHANT_TREE),
            NpcKind::ShrineKeeper => Some(&SHRINE_KEEPER_TREE),
            NpcKind::WatchCaptain => Some(&WATCH_CAPTAIN_TREE),
            NpcKind::Rustguard => Some(&RUSTGUARD_TREE),
            _ => None,
        }
    }

    /// Effect when the player talks to this NPC. Returns an effect tag.
    pub fn effect(&self) -> &'static str {
        match self {
            NpcKind::WoundedDelver => "heal_and_potion",
            NpcKind::Cartographer => "reveal_map",
            NpcKind::Merchant => "trade",
            NpcKind::ShrineKeeper => "bargain",
            _ => "",
        }
    }

    /// Short repeat dialog spoken when talking to an NPC a second time.
    /// This dialog is NOT logged to the message log.
    pub fn repeat_dialog(&self) -> &'static str {
        match self {
            NpcKind::WoundedDelver => "The delver nods weakly...",
            NpcKind::Cartographer => "The cartographer is busy with their maps.",
            NpcKind::Merchant => "No more trades for now, friend.",
            NpcKind::ShrineKeeper => "The shrine has nothing more to offer.",
            NpcKind::WatchCaptain => "The captain watches the dark silently.",
            NpcKind::Rustguard => "The guard nods and returns to their vigil.",
            NpcKind::Cook => "The cook stirs the pot, ignoring you.",
            NpcKind::Quartermaster => "The quartermaster checks their ledger.",
            NpcKind::ApprenticeMapper => "The apprentice is lost in their charts.",
            NpcKind::Scavenger => "The scavenger keeps sorting through salvage.",
            NpcKind::LostDelver => "The delver stares into the dark, unseeing.",
            NpcKind::Pilgrim => "The pilgrim prays quietly.",
            NpcKind::Doubter => "The doubter shakes their head and looks away.",
            NpcKind::Lookout => "The lookout stays focused on the perimeter.",
            NpcKind::LanternTender => "The lantern tender adjusts the wick.",
            NpcKind::CampSurvivor => "The survivor says nothing more.",
        }
    }

    /// Whether this NPC is a camp NPC (spawned as part of a camp, not standalone).
    #[allow(dead_code)]
    pub fn is_camp_npc(&self) -> bool {
        matches!(
            self,
            NpcKind::WatchCaptain
                | NpcKind::Rustguard
                | NpcKind::Cook
                | NpcKind::Quartermaster
                | NpcKind::ApprenticeMapper
                | NpcKind::Scavenger
                | NpcKind::LostDelver
                | NpcKind::Pilgrim
                | NpcKind::Doubter
                | NpcKind::Lookout
                | NpcKind::LanternTender
                | NpcKind::CampSurvivor
        )
    }

    /// Random NPC for the given depth (standalone NPCs, not camp NPCs).
    /// Camp NPCs are spawned separately via `camp_npcs_for_kind()`.
    pub fn random_for_depth(rng: &mut impl rand::Rng, depth: u32) -> Self {
        let pool: &[NpcKind] = match depth {
            0 => &[NpcKind::WoundedDelver],
            1..=2 => &[
                NpcKind::WoundedDelver,
                NpcKind::Cartographer,
                NpcKind::Merchant,
            ],
            _ => &[
                NpcKind::Cartographer,
                NpcKind::Merchant,
                NpcKind::ShrineKeeper,
            ],
        };
        pool[rng.gen_range(0..pool.len())]
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// CONVERSATION TREES
// ══════════════════════════════════════════════════════════════════════════════

// ── Wounded Delver ──

static WOUNDED_DELVER_TREE: [TalkNode; 4] = [
    TalkNode {
        npc_line: "Don't waste a clean bandage on me. I spent my last good luck two rooms ago.",
        choices: &[
            TalkChoice { label: "Who hurt you?", next_node: 1 },
            TalkChoice { label: "Why did you come here?", next_node: 2 },
            TalkChoice { label: "Can I help?", next_node: 3 },
        ],
        closing: None,
    },
    TalkNode {
        npc_line: "Doesn't matter now. Just... don't trust the silence. When it goes quiet, run.",
        choices: &[],
        closing: Some("The delver presses a potion into your hands. You feel restored."),
    },
    TalkNode {
        npc_line: "The same reason everyone does. Gold first. Glory second. Then the Crown asks you a question and you pretend you knew the answer all along.",
        choices: &[],
        closing: Some("The delver presses a potion into your hands. You feel restored."),
    },
    TalkNode {
        npc_line: "Too late for me. But you... keep your torch close. The deep remembers faces.",
        choices: &[],
        closing: Some("The delver presses a potion into your hands. You feel restored."),
    },
];

// ── Cartographer ──

static CARTOGRAPHER_TREE: [TalkNode; 4] = [
    TalkNode {
        npc_line: "I mapped this hall yesterday. Today it has one more door.",
        choices: &[
            TalkChoice { label: "That isn't possible.", next_node: 1 },
            TalkChoice { label: "Can your maps be trusted?", next_node: 2 },
            TalkChoice { label: "What are you really mapping?", next_node: 3 },
        ],
        closing: None,
    },
    TalkNode {
        npc_line: "Possible or not, I measured it twice. Stone can be trusted. Memory cannot. Down here, both are soft.",
        choices: &[],
        closing: Some("The cartographer shares their maps. The layout becomes clear."),
    },
    TalkNode {
        npc_line: "Trusted? No. Useful? Sometimes. The dungeon doesn't lie — it just changes its mind.",
        choices: &[],
        closing: Some("The cartographer shares their maps. The layout becomes clear."),
    },
    TalkNode {
        npc_line: "Not the walls. The gaps. Where rooms should be but aren't. Where stairs go when no one's looking. The geography of absence.",
        choices: &[],
        closing: Some("The cartographer shares their maps. The layout becomes clear."),
    },
];

// ── Merchant ──

static MERCHANT_TREE: [TalkNode; 4] = [
    TalkNode {
        npc_line: "You're looking at me like I'm mad for setting up shop down here.",
        choices: &[
            TalkChoice { label: "You are.", next_node: 1 },
            TalkChoice { label: "Business is business?", next_node: 2 },
            TalkChoice { label: "What do people buy down here?", next_node: 3 },
        ],
        closing: None,
    },
    TalkNode {
        npc_line: "Madness pays better than sanity ever did. A delver's coin spends twice — once when they descend, once when they don't come back.",
        choices: &[],
        closing: None,
    },
    TalkNode {
        npc_line: "Exactly. Fear has a market value. So does hope. And lantern oil. Mostly lantern oil.",
        choices: &[],
        closing: None,
    },
    TalkNode {
        npc_line: "Proof. Bones. Royal seals. Spider silk. Anything that lets fools above pretend they understand what sleeps below.",
        choices: &[],
        closing: None,
    },
];

// ── Shrine Keeper ──

static SHRINE_KEEPER_TREE: [TalkNode; 4] = [
    TalkNode {
        npc_line: "The old king did not build a tomb. He built a question.",
        choices: &[
            TalkChoice { label: "What question?", next_node: 1 },
            TalkChoice { label: "I don't believe in your shrine.", next_node: 2 },
            TalkChoice { label: "Is the Crown holy?", next_node: 3 },
        ],
        closing: None,
    },
    TalkNode {
        npc_line: "What would you carry back to the surface, if it cost you everything? That is the only question the old king ever asked. He asked it of himself. Now the Crown asks it of you.",
        choices: &[],
        closing: None,
    },
    TalkNode {
        npc_line: "Belief isn't required. The shrine doesn't ask for faith. It only asks for a price. You'll know yours when you hear it.",
        choices: &[],
        closing: None,
    },
    TalkNode {
        npc_line: "Holy? No. The Crown is older than holiness. It was here before there were gods to name things good or evil. It doesn't judge. It asks.",
        choices: &[],
        closing: None,
    },
];

// ── Watch-Captain ──

static WATCH_CAPTAIN_TREE: [TalkNode; 4] = [
    TalkNode {
        npc_line: "Keep your torch low. Things down here learn where the light gathers.",
        choices: &[
            TalkChoice { label: "Are you guarding this camp?", next_node: 1 },
            TalkChoice { label: "What are you afraid of?", next_node: 2 },
            TalkChoice { label: "I'm going deeper.", next_node: 3 },
        ],
        closing: None,
    },
    TalkNode {
        npc_line: "Guarding? No. Holding a line. There's a difference. A guard waits for trouble. A line tells trouble where it stops.",
        choices: &[],
        closing: Some("The captain nods once. \"Stay behind the lantern posts if you want to keep breathing.\""),
    },
    TalkNode {
        npc_line: "Not death. Noise. When the deep goes quiet, it means something is listening. The bells only ring when it's already too late.",
        choices: &[],
        closing: Some("The captain glances toward the dark. \"Don't follow the bells, if you hear them.\""),
    },
    TalkNode {
        npc_line: "Then remember this: if you hear bells underground, don't follow them. And if you reach the Crown... be sure you have an answer. It will ask you something. Everyone hears a different question.",
        choices: &[],
        closing: Some("The captain steps aside. \"The Rustguard doesn't stop delvers. We just make sure whatever follows you back doesn't reach the surface.\""),
    },
];

// ── Rustguard ──

static RUSTGUARD_TREE: [TalkNode; 3] = [
    TalkNode {
        npc_line: "\"They test us every few hours,\" the guard mutters, not taking eyes off the corridor. \"Rats first. Goblins after. Then the things that learn our names.\"",
        choices: &[
            TalkChoice { label: "How long have you been down here?", next_node: 1 },
            TalkChoice { label: "Why don't you leave?", next_node: 2 },
        ],
        closing: None,
    },
    TalkNode {
        npc_line: "Long enough to forget what sunlight feels like. The captain says six months. Feels like six years. Time moves different when it's always dark.",
        choices: &[],
        closing: Some("The guard tightens their grip on their weapon. \"But someone has to hold the line.\""),
    },
    TalkNode {
        npc_line: "Some of us tried. The ones who made it back said the surface felt wrong. Too bright. Too quiet. You don't leave the deep — you just carry it with you.",
        choices: &[],
        closing: Some("The guard's eyes flick toward you. \"If you make it out, don't look back. Not even once.\""),
    },
];

// ══════════════════════════════════════════════════════════════════════════════
// CAMPS
// ══════════════════════════════════════════════════════════════════════════════

/// Types of camps that can appear in the dungeon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CampKind {
    /// A waystation for delvers — bedrolls, crates, lanterns.
    DelverWaystation,
    /// A Rustguard defensive outpost — barricades, guards, warning bells.
    RustguardOutpost,
    /// A cartographer survey camp — map tables, chalk marks, measuring rods.
    CartographerSurvey,
    /// A shrine pilgrim camp — candles, bone charms, old king iconography.
    ShrinePilgrim,
    /// An abandoned camp — dead fire, torn tents, no NPCs or one survivor.
    Abandoned,
}

impl CampKind {
    pub fn name(&self) -> &'static str {
        match self {
            CampKind::DelverWaystation => "Delver Waystation",
            CampKind::RustguardOutpost => "Rustguard Outpost",
            CampKind::CartographerSurvey => "Cartographer Survey Camp",
            CampKind::ShrinePilgrim => "Shrine Pilgrim Camp",
            CampKind::Abandoned => "Abandoned Camp",
        }
    }

    /// The NPC roles that populate this camp type.
    pub fn npc_kinds(&self) -> &'static [NpcKind] {
        match self {
            CampKind::DelverWaystation => &[
                NpcKind::WatchCaptain,
                NpcKind::Cook,
                NpcKind::LanternTender,
                NpcKind::LostDelver,
            ],
            CampKind::RustguardOutpost => &[
                NpcKind::WatchCaptain,
                NpcKind::Rustguard,
                NpcKind::Rustguard,
                NpcKind::Quartermaster,
                NpcKind::Lookout,
            ],
            CampKind::CartographerSurvey => &[
                NpcKind::Cartographer,
                NpcKind::ApprenticeMapper,
                NpcKind::Scavenger,
            ],
            CampKind::ShrinePilgrim => &[
                NpcKind::ShrineKeeper,
                NpcKind::Pilgrim,
                NpcKind::Doubter,
                NpcKind::LanternTender,
            ],
            CampKind::Abandoned => &[NpcKind::CampSurvivor],
        }
    }

    /// Random camp kind for the given depth.
    pub fn random_for_depth(rng: &mut impl rand::Rng, depth: u32) -> Self {
        let pool: &[CampKind] = match depth {
            1 => &[CampKind::DelverWaystation, CampKind::CartographerSurvey],
            2 => &[
                CampKind::DelverWaystation,
                CampKind::RustguardOutpost,
                CampKind::CartographerSurvey,
                CampKind::ShrinePilgrim,
            ],
            3 => &[
                CampKind::RustguardOutpost,
                CampKind::ShrinePilgrim,
                CampKind::Abandoned,
            ],
            _ => &[CampKind::RustguardOutpost, CampKind::Abandoned],
        };
        pool[rng.gen_range(0..pool.len())]
    }

    /// Whether monsters should spawn in the same room as this camp.
    /// Abandoned camps can have monsters; active camps should not.
    #[allow(dead_code)]
    pub fn allows_monsters(&self) -> bool {
        matches!(self, CampKind::Abandoned)
    }
}

/// A camp placed in a dungeon room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camp {
    /// Index into `map.rooms`.
    pub room_idx: usize,
    /// Camp center (x, y).
    pub x: i32,
    pub y: i32,
    /// Type of camp.
    pub kind: CampKind,
    /// Indices into `GameState.npcs` for NPCs belonging to this camp.
    pub npc_indices: Vec<usize>,
    /// Whether the player has already rested here.
    #[serde(default)]
    pub rested: bool,
}

/// A friendly NPC on the dungeon map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Npc {
    pub x: i32,
    pub y: i32,
    pub kind: NpcKind,
    /// Whether the player has already talked to this NPC.
    #[serde(default)]
    pub talked: bool,
    /// Custom display name (set for campaign named NPCs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
}

impl Npc {
    pub fn new(x: i32, y: i32, kind: NpcKind) -> Self {
        Self {
            x,
            y,
            kind,
            talked: false,
            custom_name: None,
        }
    }

    /// Display name: uses custom_name if set, otherwise the kind's default name.
    pub fn display_name(&self) -> String {
        self.custom_name
            .clone()
            .unwrap_or_else(|| self.kind.name().to_string())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// LANDMARKS & SHRINES
// ══════════════════════════════════════════════════════════════════════════════

/// Special room landmark types for visual and gameplay variety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Landmark {
    SpiderNest,
    FloodedVault,
    BoneChapel,
    CollapsedForge,
    Shrine,
    /// Campaign-specific: the corrupted bell chamber.
    BellChamber,
}

impl Landmark {
    pub fn name(&self) -> &'static str {
        match self {
            Landmark::SpiderNest => "Spider Nest",
            Landmark::FloodedVault => "Flooded Vault",
            Landmark::BoneChapel => "Bone Chapel",
            Landmark::CollapsedForge => "Collapsed Forge",
            Landmark::Shrine => "Ancient Shrine",
            Landmark::BellChamber => "Bell Chamber",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Landmark::SpiderNest => "Webs coat every surface.",
            Landmark::FloodedVault => "Knee-deep water fills the room.",
            Landmark::BoneChapel => "Ribs line the walls like pews.",
            Landmark::CollapsedForge => "An anvil stands amid rubble.",
            Landmark::Shrine => "A shimmering altar pulses faintly.",
            Landmark::BellChamber => "A massive iron bell hangs cracked and silent.",
        }
    }

    pub fn random(rng: &mut impl rand::Rng, depth: u32) -> Self {
        let pool: &[Landmark] = match depth {
            0 => &[Landmark::CollapsedForge, Landmark::FloodedVault],
            1 => &[
                Landmark::BoneChapel,
                Landmark::CollapsedForge,
                Landmark::FloodedVault,
            ],
            2 => &[Landmark::BoneChapel, Landmark::SpiderNest, Landmark::Shrine],
            _ => &[Landmark::SpiderNest, Landmark::BoneChapel, Landmark::Shrine],
        };
        pool[rng.gen_range(0..pool.len())]
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// LORE FRAGMENTS
// ══════════════════════════════════════════════════════════════════════════════

/// Returns a lore text for the given depth (journal entries from
/// a previous Crown-seeker), or None if no lore for that floor.
pub fn lore_for_depth(depth: u32) -> Option<&'static str> {
    match depth {
        0 => Some("Day 1: The entrance was easier to find than I expected. The old stories weren't lying about the moss-choked mouth."),
        1 => Some("Day 3: The cold seeps through my cloak. Scratching in the walls. The Crown must be close — I can feel its weight even from here."),
        2 => Some("Day 5: The rust smells like blood. I found the old king's seal on a collapsed archway. This was his domain. I am walking through history."),
        3 => Some("Day 7: The spiders know I'm here. They watch from the dark, patient. But I can feel it — the Crown is near. I hope it asks me the right question."),
        4 => Some("Final entry: I see it. The Crown of Depth, resting on obsidian. But something guards it. Something old. If you read this... take the Crown. Finish what I started."),
        _ => None,
    }
}
