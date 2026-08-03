// ── Narrative & flavour text for Delve ──
//
// Ambient descriptions, death epitaphs, monster encounter flavour,
// biome-flavoured descent messages, combat lines, Crown whispers, and
// the surface win sequence — all the world‑building text that gives the
// dungeon a voice.
//
// ═══════════════════════════════════════════════════════════════════════════
// LORE CANON (the spine every surface-text should be consistent with)
// ═══════════════════════════════════════════════════════════════════════════
//
// THE CROWN OF DEPTH
//   Older than the kingdom, older than holiness. It is not gold — it is
//   something that learned to look like gold. It does not grant power; it
//   asks a question. Every delver hears a different version of the question.
//   The Crown wants to be carried back to the surface, but it does not tell
//   you why — or what happens when it gets there.
//
// THE OLD KING
//   He did not build a tomb. He built a test. He wore the Crown once,
//   never removed it, and walked into the deep knowing no one would ever
//   claim it for the wrong reason. The dungeon is the lock; the Crown is
//   the key; the Guardian is the last check.
//
// THE DUNGEON
//   It rearranges itself. Rooms appear and vanish. Stairs migrate when no
//   one watches. The geography is soft because it is not stone — it is
//   memory, held in place by the Crown's presence. The deeper you go, the
//   less the dungeon tolerates being mapped.
//
// THE RUSTGUARD
//   They aren't guarding treasure. They're guarding the stairs — the path
//   back to the surface. Every Rustguard carries a bell. You only ring it
//   when you're already dead, because it tells the other guards that
//   something is coming up behind you. The Rustguard's real enemy isn't
//   the monsters; it's whatever follows a delver home.
//
// THE BELLS
//   Silence underground is worse than noise. When the deep goes quiet, it
//   means something is listening. Bells mean the listening has stopped and
//   the something is moving. Do not follow the bells.
//
// THE SURFACE
//   Greyford: home of the Delvers' Compact, pays bounties for royal seals.
//   Ash Markets: trade in "deep goods," no questions asked.
//   Kingless Coast: hasn't had a king in three hundred years. Some say
//   the Crown is why.
//   Lantern Road: leads somewhere that isn't on any map.
//
// THE RETURN
//   Picking up the Crown changes the dungeon. The darkness presses closer.
//   More things wake up. The ascent is never the same path as the descent —
//   the dungeon doesn't want you to leave with what you took. When you
//   finally reach sunlight, the dungeon seals itself behind you. It is done
//   with this era, and you are its answer.
// ═══════════════════════════════════════════════════════════════════════════

use rand::Rng;

use crate::entity::MonsterKind;

// ══════════════════════════════════════════════════════════════════════════════
// W3 — AMBIENT SOUND DESCRIPTIONS (biome‑themed, fire rarely)
// ══════════════════════════════════════════════════════════════════════════════

/// Return a random ambient line for the current depth (caller gates
/// the probability; this function always picks one if called).
pub fn ambient_line(depth: u32, rng: &mut impl Rng) -> Option<&'static str> {
    let lines = match depth {
        0 => &[
            "You hear water dripping somewhere in the dark.",
            "A cold draught stirs the dust at your feet.",
            "The stone walls feel damp and ancient.",
            "Something skitters in the shadows beyond your torchlight.",
            "A faint, rhythmic tapping echoes from below.",
        ][..],
        1 => &[
            "The air grows thin and cold.",
            "You feel a distant rumble through the stone.",
            "Icy mist curls around your ankles.",
            "A mournful howl reverberates through the caverns.",
            "Frost clings to the walls — this place has been cold for centuries.",
        ][..],
        2 => &[
            "The scent of rust and old blood hangs in the air.",
            "You hear the clank of distant chains.",
            "Broken stone grinds underfoot.",
            "A low, rhythmic chanting seems to come from the walls themselves.",
            "Something metal scrapes against stone far below.",
        ][..],
        3 => &[
            "Webs brush against your face — sticky and cold.",
            "The air tastes of copper and venom.",
            "You hear chittering from every direction.",
            "Pale fungus glows faintly on the walls, pulsing like a heartbeat.",
            "A whisper, too soft to understand, slithers past your ear.",
        ][..],
        _ => &[
            "The silence here is absolute — no drip, no wind, no breath.",
            "Golden motes drift through the air like suspended stars.",
            "You feel the weight of centuries pressing down on you.",
            "The darkness itself seems to watch you.",
            "Something vast and old stirs in the deep.",
        ][..],
    };
    Some(lines[rng.gen_range(0..lines.len())])
}

// ══════════════════════════════════════════════════════════════════════════════
// W4 — DEPTH‑FLAVOURED DESCENT MESSAGES
// ══════════════════════════════════════════════════════════════════════════════

/// Replace "You descend to depth N." with a biome‑specific line.
pub fn descent_line(depth: u32) -> &'static str {
    match depth {
        0 => "You lower yourself into the moss‑choked entrance.",
        1 => "You descend. The air turns cold and blue.",
        2 => "You climb down into rusting crypt halls. The smell of iron is overwhelming.",
        3 => "You drop into a web‑choked lair. Pale eyes watch from every corner.",
        4 => "You enter the Void Chamber. A golden pulse thrums through the stone.",
        _ => "You descend deeper into the unknown.",
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// W4b — ASCENT MESSAGES (when climbing while carrying the Crown)
// ══════════════════════════════════════════════════════════════════════════════

/// Replace the generic ascent message with Crown‑aware ascent text.
/// Used when the player climbs stairs while holding the Crown.
pub fn ascent_line(depth: u32) -> &'static str {
    match depth {
        3 => "You climb upward. The spiders watch you go — they know what you carry.",
        2 => "You ascend through the rusting crypts. The Crown feels heavier with every step.",
        1 => "The cold blue halls give way to warmer stone. Almost there.",
        0 => "You climb toward daylight. The Crown pulses once, like a heartbeat.",
        _ => "You ascend toward the surface, the Crown's weight growing.",
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// W5 — MONSTER ENCOUNTER FLAVOUR (first sight per run)
// ══════════════════════════════════════════════════════════════════════════════

/// Return a flavour line when the player first sees this monster kind.
pub fn monster_first_sight(kind: MonsterKind) -> Option<&'static str> {
    match kind {
        MonsterKind::Rat => Some("A rat skitters out of the shadows — teeth bared."),
        MonsterKind::Bat => Some("Leathery wings flap overhead. A bat swoops at your face."),
        MonsterKind::Goblin => Some("A goblin snarls — beady eyes glinting with malice."),
        MonsterKind::Skeleton => Some("Bones clatter together. A skeleton rises from the rubble."),
        MonsterKind::Orc => Some("An orc lumbers into view, its breath foul and heavy."),
        MonsterKind::Spider => {
            Some("Legs the size of spears unfold. A giant spider descends from above.")
        }
        MonsterKind::Wraith => {
            Some("A wraith phases through the wall — a scream trapped in silence.")
        }
        MonsterKind::Slime => Some("A quivering mass of green ooze pulses toward you."),
        MonsterKind::Mimic => Some("That chest... blinked."),
        MonsterKind::Necromancer => {
            Some("A robed figure chants in a tongue that claws at your mind.")
        }
        MonsterKind::Sentinel => Some("An armoured statue creaks to life — its eyes burning red."),
        MonsterKind::Guardian => Some("The Crown's protector rises — ancient, colossal, silent."),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// W6 — CROWN'S CALL (ambient messages while Crown is on‑floor but not held)
// ══════════════════════════════════════════════════════════════════════════════

/// Return an ambient Crown‑call line (caller gates probability).
/// These are only fired while the Crown exists on the deepest floor and
/// the player has not yet picked it up.
pub fn crown_whisper(rng: &mut impl Rng) -> Option<&'static str> {
    if !rng.gen_bool(0.04) {
        return None;
    }
    let lines = &[
        "Something golden pulses in the deep — two beats, then silence.",
        "The Crown asks a question you can almost hear. Almost.",
        "Your footsteps echo twice: once from the stone, once from below.",
        "The air shimmers. Gold light flickers at the edge of your vision.",
        "You hear your own heartbeat echoing back at you — doubled.",
        "The Crown whispers: Why have you come this far?",
    ];
    Some(lines[rng.gen_range(0..lines.len())])
}

// ══════════════════════════════════════════════════════════════════════════════
// W8 — COMBAT FLAVOUR LINES (varied hit / kill messages per monster)
// ══════════════════════════════════════════════════════════════════════════════

/// Return a kill line for the given monster kind.
pub fn kill_flavour(kind: MonsterKind, rng: &mut impl Rng) -> &'static str {
    let lines: &[&str] = match kind {
        MonsterKind::Rat => &[
            "The rat squeals and falls still.",
            "You crush the rodent under your boot.",
            "A swift strike ends the rat's scurrying.",
        ],
        MonsterKind::Bat => &[
            "The bat crumples, wings folding inward.",
            "You swat the bat out of the air.",
            "The bat's screech cuts off abruptly.",
        ],
        MonsterKind::Goblin => &[
            "The goblin gurgles and collapses.",
            "Your blade finds the goblin's heart — it crumples.",
            "The goblin lets out a final, spiteful hiss.",
        ],
        MonsterKind::Skeleton => &[
            "The skeleton clatters into a heap of bones.",
            "You shatter the skeleton's skull — it crumbles.",
            "Bones scatter across the stone floor.",
        ],
        MonsterKind::Orc => &[
            "The orc crashes to the ground — dead weight.",
            "With a final roar, the orc falls.",
            "You drive your blade deep — the orc topples.",
        ],
        MonsterKind::Spider => &[
            "The spider's legs curl inward as it dies.",
            "Ichor pools beneath the spider's crumpled body.",
            "The spider twitches once, then lies still.",
        ],
        MonsterKind::Wraith => &[
            "The wraith dissolves into mist with a silent scream.",
            "You banish the wraith — it evaporates into nothing.",
            "The spectral form shrieks and fades.",
        ],
        MonsterKind::Slime => &[
            "The slime bursts, splattering green goo.",
            "You slice the ooze — it collapses into a puddle.",
            "The slime quivers and dissolves.",
        ],
        MonsterKind::Mimic => &[
            "The mimic reverts to a lifeless chest — now cracked and still.",
            "You smash the mimic's teeth — it lies motionless.",
            "The mimic's disguise shatters. It's just wood now.",
        ],
        MonsterKind::Necromancer => &[
            "The necromancer's chant dies on cracked lips.",
            "You sever the dark bond — the necromancer crumbles to dust.",
            "The robed figure shrieks as its magic unravels.",
        ],
        MonsterKind::Sentinel => &[
            "The sentinel's armour caves inward — it kneels, then falls.",
            "You strike the sentinel's core — its eyes go dark.",
            "The guardian statue collapses into ancient rubble.",
        ],
        MonsterKind::Guardian => &[
            "The Crown's protector shudders and falls — its vigil over.",
            "With a sound like mountains cracking, the Guardian falls.",
            "You stand over the Guardian's broken form. The way is open.",
        ],
    };
    lines[rng.gen_range(0..lines.len())]
}

/// Return a hit line for the given monster kind.
pub fn hit_flavour(kind: MonsterKind, _damage: i32, critical: bool, rng: &mut impl Rng) -> String {
    if critical {
        return format!(
            "A devastating blow tears into the {name}!",
            name = kind.name()
        );
    }
    let lines: &[&str] = match kind {
        MonsterKind::Rat => &["You slash the rat.", "Your blade cuts into the rat."],
        MonsterKind::Bat => &[
            "You clip the bat's wing.",
            "The bat recoils from your strike.",
        ],
        MonsterKind::Goblin => &["You strike the goblin.", "Your weapon bites goblin flesh."],
        MonsterKind::Skeleton => &[
            "You crack the skeleton's ribs.",
            "Bone splinters from your hit.",
        ],
        MonsterKind::Orc => &["You wound the orc.", "The orc grunts in pain."],
        MonsterKind::Spider => &["You slash a hairy leg.", "Ichor sprays from your strike."],
        MonsterKind::Wraith => &[
            "Your blade passes through the wraith.",
            "The wraith's form flickers.",
        ],
        MonsterKind::Slime => &["You cut into the slime.", "Goo splatters from your attack."],
        MonsterKind::Mimic => &[
            "You dent the mimic's lid.",
            "The mimic shudders from your strike.",
        ],
        MonsterKind::Necromancer => &[
            "You interrupt the necromancer's chant.",
            "Dark robes tear under your blade.",
        ],
        MonsterKind::Sentinel => &["Steel rings against stone.", "Sparks fly from your strike."],
        MonsterKind::Guardian => &[
            "Your weapon glances off ancient armour.",
            "The Guardian barely flinches.",
        ],
    };
    lines[rng.gen_range(0..lines.len())].to_string()
}

/// Return a hit line for when a monster hits the player.
pub fn monster_hit_flavour(kind: MonsterKind, _damage: i32, rng: &mut impl Rng) -> String {
    let lines: &[&str] = match kind {
        MonsterKind::Rat => &[
            "The rat bites your ankle!",
            "Sharp teeth sink into your leg!",
        ],
        MonsterKind::Bat => &["The bat rakes your face!", "Claws scratch your arm!"],
        MonsterKind::Goblin => &[
            "The goblin slashes at you!",
            "A crude blade finds its mark!",
        ],
        MonsterKind::Skeleton => &[
            "Bony fingers rake your arm!",
            "The skeleton's blade cuts deep!",
        ],
        MonsterKind::Orc => &["The orc's axe crashes down!", "The orc headbutts you!"],
        MonsterKind::Spider => &["Fangs pierce your shoulder!", "A hairy leg slams into you!"],
        MonsterKind::Wraith => &[
            "Icy claws pass through your chest!",
            "The wraith's touch burns cold!",
        ],
        MonsterKind::Slime => &["The slime burns your skin!", "Acidic ooze splashes you!"],
        MonsterKind::Mimic => &[
            "A sticky tongue lashes you!",
            "The mimic snaps at your arm!",
        ],
        MonsterKind::Necromancer => &[
            "Dark energy sears your flesh!",
            "A bolt of shadow strikes you!",
        ],
        MonsterKind::Sentinel => &[
            "The sentinel's halberd swings down!",
            "A stone fist crushes into you!",
        ],
        MonsterKind::Guardian => &[
            "The Guardian's blow shakes the chamber!",
            "Ancient fury smashes into you!",
        ],
    };
    lines[rng.gen_range(0..lines.len())].to_string()
}

// ══════════════════════════════════════════════════════════════════════════════
// W9 — CAMP AMBIENT LINES
// ══════════════════════════════════════════════════════════════════════════════

/// Camp ambient lines fire when the player is near a camp (in or adjacent).
/// Different camp types have different flavor.
pub fn camp_ambient(kind: crate::entity::CampKind, rng: &mut impl Rng) -> Option<&'static str> {
    if !rng.gen_bool(0.05) {
        return None;
    }
    let lines: &[&str] = match kind {
        crate::entity::CampKind::DelverWaystation => &[
            "A kettle bubbles over a blue campfire.",
            "Someone has scratched tally marks into a crate: \"Days below: 19.\"",
            "A guard mutters, \"No singing after dark. It answers back.\"",
            "Delvers huddle around a lantern, speaking in low voices.",
            "The smell of stew and damp wool fills the air.",
            "Bedrolls are arranged in a circle — feet toward the fire, backs to the dark.",
        ],
        crate::entity::CampKind::RustguardOutpost => &[
            "Iron barricades line the walls — scarred by claws and worse.",
            "A warning bell hangs from a rusted chain. It hasn't rung yet tonight.",
            "Guards rotate watches in silence. No one speaks unless necessary.",
            "A tally board reads: \"Days held: 47. Casualties this week: 2.\"",
            "The air smells of iron, lamp oil, and old blood.",
            "Someone has carved into the wall: \"Do not answer the bells.\"",
        ],
        crate::entity::CampKind::CartographerSurvey => &[
            "A map lies pinned beneath a dagger. The ink is still moving.",
            "Blue chalk arrows point in three directions. All are labeled \"UP.\"",
            "Strings and measuring rods mark the room's edges. The numbers keep changing.",
            "An apprentice frowns at two maps. They cannot both be right.",
            "A note in the margin reads: \"Room gained 2 ft since yesterday. Verify.\"",
            "Lantern light catches the dust suspended above the map table — frozen, unmoving.",
        ],
        crate::entity::CampKind::ShrinePilgrim => &[
            "Candles gutter in a draft that has no source.",
            "A pilgrim traces the old king's seal into the floor with ash.",
            "Quiet chanting echoes off the walls — half prayer, half plea.",
            "Bone charms hang from a tripod. One of them is still warm.",
            "A scrap of parchment reads: \"The Crown called my name last night.\"",
            "Incense smoulders in a cracked bowl. The smoke moves against the wind.",
        ],
        crate::entity::CampKind::Abandoned => &[
            "The fire pit is cold. It's been cold for days.",
            "Six bedrolls. Five packs. No bodies.",
            "A message is carved into the table: \"Do not answer the bells.\"",
            "A lantern still burns — but no one is here to tend it.",
            "Footprints lead to the center of the room and simply stop.",
            "Half-eaten rations lie scattered. The teeth marks aren't human.",
        ],
    };
    Some(lines[rng.gen_range(0..lines.len())])
}

// ══════════════════════════════════════════════════════════════════════════════
// W10 — RUSTGUARD COMBAT BARKS
// ══════════════════════════════════════════════════════════════════════════════

/// Bark when a Rustguard NPC engages a monster.
#[allow(dead_code)]
pub fn guard_engage(rng: &mut impl Rng) -> &'static str {
    let lines = &[
        "A Rustguard shouts: \"Contact! East side!\"",
        "\"Hold the line!\" a guard bellows.",
        "A Rustguard readies their weapon: \"Here they come!\"",
        "\"Back to the lanterns!\" a guard calls out.",
    ];
    lines[rng.gen_range(0..lines.len())]
}

/// Bark after a Rustguard kills a monster.
#[allow(dead_code)]
pub fn guard_kill(rng: &mut impl Rng) -> &'static str {
    let lines = &[
        "\"One more for the pit,\" a guard mutters.",
        "A Rustguard nods grimly: \"That's one.\"",
        "\"Stay down,\" a guard growls at the corpse.",
    ];
    lines[rng.gen_range(0..lines.len())]
}

/// Bark when a Rustguard returns to their patrol route.
#[allow(dead_code)]
pub fn guard_return(rng: &mut impl Rng) -> &'static str {
    let lines = &[
        "A guard murmurs: \"Line's clear. For now.\"",
        "\"Back to watch,\" a Rustguard says quietly.",
        "A guard resumes their patrol, eyes on the dark.",
        "\"They'll be back,\" a guard mutters. \"They always come back.\"",
    ];
    lines[rng.gen_range(0..lines.len())]
}

// ══════════════════════════════════════════════════════════════════════════════
// W11 — DUNGEON RUMORS (NPCs can pull from this pool)
// ══════════════════════════════════════════════════════════════════════════════

/// Returns a random rumor about the dungeon. Some contradict each other.
#[allow(dead_code)]
pub fn dungeon_rumor(rng: &mut impl Rng) -> &'static str {
    let rumors = &[
        // Crown lore
        "The Crown was forged before the sun first rose.",
        "The old king wore the Crown once and never removed it.",
        "The Crown is not gold — it's something else that learned to look like gold.",
        "Whoever wears the Crown remembers everything the old king knew.",
        // Dungeon nature
        "The dungeon grows upward one room every winter.",
        "Maps are safer if drawn in blood. Ink forgets.",
        "There is no bottom. The Void Chamber is just the lowest place anyone's come back from.",
        "Something down here is older than the Crown. Older than the king. Older than stone.",
        // Monsters and dangers
        "The spiders don't eat priests. Nobody knows why.",
        "If you see a door you don't remember opening, don't go through it.",
        "Mimics don't hunt. They wait. They're very patient.",
        "The necromancers weren't born here. They were delvers once.",
        // The Guardian
        "The Guardian used to be human. Some say it was the old king himself.",
        "The Guardian has been killed before. It always comes back.",
        "Only someone who doesn't want the Crown can defeat the Guardian.",
        // The Rustguard
        "The Rustguard's captain has been below for three years. They say he can't leave.",
        "The Rustguard don't guard the treasure. They guard the stairs.",
        "Every Rustguard has a bell. They say you only ring it when you're already dead.",
        // Above world references
        "The Delvers' Compact in Greyford offers a bounty for intact royal seals.",
        "They say the Lantern Road leads somewhere that isn't on any map.",
        "Merchants from the Ash Markets trade in 'deep goods.' No questions asked.",
        "The Kingless Coast hasn't had a king in three hundred years. Some say the Crown is why.",
    ];
    rumors[rng.gen_range(0..rumors.len())]
}

/// A named dead delver inscription, for history tiles or flavor.
pub fn named_delver_epitaph(rng: &mut impl Rng) -> &'static str {
    let epitaphs = &[
        "Here lies Tovin Reed. He reached the rust halls and turned back too late.",
        "Jessa Marr — delver, cartographer, friend. The map ends here.",
        "Belos Ward — killed by what he sought. May the Crown forgive him.",
        "Erin Voss. She heard the bells. She followed them.",
        "Galen Torch — carried a lantern for thirty others. None remain.",
        "\"I almost had it.\" — Last words of P. Dremm, depth unknown.",
        "Sovra Kell — first to map the Frozen Halls. Last to leave them.",
        "Rest here, Orin Greaves. The dungeon took your name but not your light.",
        "Mareth — \"Tell them I wasn't afraid.\" We know you were. We all are.",
        "Here, three delvers whose names the stone refuses to remember.",
    ];
    epitaphs[rng.gen_range(0..epitaphs.len())]
}

/// Room flavor for history tiles that mention camps or delver activity.
#[allow(dead_code)]
pub fn camp_room_flavor(rng: &mut impl Rng) -> &'static str {
    let lines = &[
        "A forgotten bedroll lies in the corner — empty.",
        "Someone scratched a message here: \"The Crown is real. Turn back.\"",
        "Faded chalk marks on the wall: three days of rations counted.",
        "A cracked lantern. The wick is still warm.",
        "Boot prints in the dust. They don't lead anywhere.",
        "A snapped bowstring. A dropped pack. No blood. No body.",
    ];
    lines[rng.gen_range(0..lines.len())]
}

// ══════════════════════════════════════════════════════════════════════════════
// W12 — SURFACE WIN SEQUENCE
// ══════════════════════════════════════════════════════════════════════════════

/// The narrative text shown when the player wins.
pub fn win_narrative() -> Vec<&'static str> {
    vec![
        "You emerge from the dungeon mouth, blinking against the sunlight.",
        "The Crown of Depth rests in your hands — warm, alive, waiting.",
        "Behind you, the earth groans. The dungeon seals itself.",
        "",
        "You do not know if you answered the old king's question",
        "or simply survived it. It does not matter. The Crown is here,",
        "in the light, for the first time in centuries. And it is yours.",
        "",
        "── YOU WON ──",
    ]
}

// ══════════════════════════════════════════════════════════════════════════════
// W10 — DEATH EPITAPHS
// ══════════════════════════════════════════════════════════════════════════════

/// Generate a 1‑line epitaph from cause, depth, and kill count.
pub fn death_epitaph(
    depth: u32,
    kills: u32,
    killed_by: Option<&str>,
    rng: &mut impl Rng,
) -> String {
    let depth_str = match depth {
        0 => "the mossy entrance".to_string(),
        1 => "the frozen caverns".to_string(),
        2 => "the rusting crypts".to_string(),
        3 => "the spider lair".to_string(),
        4 => "the Void Chamber".to_string(),
        _ => format!("depth {}", depth + 1),
    };

    let cause = match killed_by {
        Some("poison") => "venom claimed them".to_string(),
        Some(name) => format!("a {} ended their quest", name),
        None => "the darkness consumed them".to_string(),
    };

    let epitaphs = [
        format!(
            "Here lies a delver who reached {} before {}.",
            depth_str, cause
        ),
        format!(
            "A brave soul, lost in {} — {} was their last foe. Slain: {}.",
            depth_str, cause, kills
        ),
        format!(
            "In {}, {} — {} monsters fell before them.",
            depth_str, cause, kills
        ),
        format!(
            "The dungeon claimed another. {} kills. {} was their grave.",
            kills, depth_str
        ),
    ];

    epitaphs[rng.gen_range(0..epitaphs.len())].clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn ambient_line_returns_some_for_all_depths() {
        let mut rng = StdRng::seed_from_u64(42);
        for depth in 0..=4 {
            let mut found = false;
            for _ in 0..500 {
                if ambient_line(depth, &mut rng).is_some() {
                    found = true;
                    break;
                }
            }
            assert!(found, "depth {} should produce ambient lines", depth);
        }
    }

    #[test]
    fn descent_lines_cover_all_depths() {
        for d in 0..=4 {
            let line = descent_line(d);
            assert!(!line.is_empty(), "depth {} should have a descent line", d);
        }
    }

    #[test]
    fn all_monsters_have_first_sight_flavour() {
        use MonsterKind::*;
        for kind in &[
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
        ] {
            assert!(
                monster_first_sight(*kind).is_some(),
                "{:?} should have first-sight text",
                kind
            );
        }
    }

    #[test]
    fn crown_whisper_may_return_some() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut found = false;
        for _ in 0..500 {
            if crown_whisper(&mut rng).is_some() {
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn kill_flavour_works_for_all_monsters() {
        let mut rng = StdRng::seed_from_u64(42);
        use MonsterKind::*;
        for kind in &[
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
        ] {
            let line = kill_flavour(*kind, &mut rng);
            assert!(!line.is_empty(), "{:?} kill flavour empty", kind);
        }
    }

    #[test]
    fn hit_flavour_works_for_all_monsters() {
        let mut rng = StdRng::seed_from_u64(42);
        use MonsterKind::*;
        for kind in &[
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
        ] {
            let line = hit_flavour(*kind, 10, false, &mut rng);
            assert!(!line.is_empty(), "{:?} hit flavour empty", kind);
        }
    }

    #[test]
    fn win_narrative_has_content() {
        let lines = win_narrative();
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|l| l.contains("WON")));
    }

    #[test]
    fn death_epitaph_is_never_empty() {
        let mut rng = StdRng::seed_from_u64(42);
        for depth in 0..=4 {
            let ep = death_epitaph(depth, 5, Some("spider"), &mut rng);
            assert!(!ep.is_empty(), "depth {} epitaph empty", depth);
        }
    }

    #[test]
    fn death_epitaph_handles_unknown_killer() {
        let mut rng = StdRng::seed_from_u64(42);
        let ep = death_epitaph(2, 10, None, &mut rng);
        assert!(!ep.is_empty());
    }

    #[test]
    fn death_epitaph_handles_poison() {
        let mut rng = StdRng::seed_from_u64(42);
        let ep = death_epitaph(3, 0, Some("poison"), &mut rng);
        assert!(!ep.is_empty());
    }
}
