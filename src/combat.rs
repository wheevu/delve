// Combat resolution — deterministic damage calculation used by the turn loop.

/// Simple, deterministic damage formula.  Always hits, always deals ≥ 1 damage.
///
/// Callers rely on this staying predictable so replays and tactical previews
/// agree with the simulation.
pub fn resolve_attack(attacker_attack: i32, defender_defense: i32) -> i32 {
    let damage = attacker_attack - defender_defense;
    damage.max(1)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_attack (simple) ──

    #[test]
    fn simple_attack_min_damage_is_one() {
        assert_eq!(resolve_attack(0, 10), 1);
        assert_eq!(resolve_attack(5, 10), 1);
    }

    #[test]
    fn simple_attack_excess_attack_is_damage() {
        assert_eq!(resolve_attack(10, 2), 8);
    }

    #[test]
    fn simple_attack_equal_stats_is_one() {
        assert_eq!(resolve_attack(5, 5), 1);
    }
}
