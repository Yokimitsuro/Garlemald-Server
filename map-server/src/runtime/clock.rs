// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Single clock domain for the battle engine.
//!
//! Every value that feeds `AIContainer` state (`AttackState.next_swing_ms`,
//! cast completion deadlines, the ticker's `now_ms`) MUST come from this
//! anchor. Mixing epoch milliseconds (~1.78e12) into AI state armed it
//! ~56 years past the ticker's server-start-relative clock — script-engaged
//! allies entered the engaged state but never swung (Garlemald-Server #28).
//!
//! Wall-clock fields (`time_of_death_utc`, recast end timestamps, rental
//! expiry) stay on `common::utils::unix_timestamp()`; their *comparisons*
//! must read wall-clock too, never this anchor.

use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since the first call in this process — the shared
/// monotonic anchor for the ticker loop and every applier that arms
/// AI-state deadlines.
pub fn server_now_ms() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_and_anchored_to_first_call() {
        let a = server_now_ms();
        let b = server_now_ms();
        assert!(b >= a);
        // The anchor is process-lifetime: values stay small relative to
        // epoch ms (~1.78e12) — the regression this module exists to
        // prevent is AI deadlines armed in the epoch domain.
        assert!(a < 1_000_000_000_000, "anchor must not be epoch-based");
    }
}
