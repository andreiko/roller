/// Represents a range on a number scale mapped to a value.
pub struct Zone {
    pub value: u8,
    pub start: u16,
    pub end: u16,
}

/// Provides a shortcut to initialize a Zone.
const fn zone(value: u8, start: u16, end: u16) -> Zone {
    Zone {
        value,
        start,
        end,
    }
}

/// Maps values of 1-20 to ~equal zones on the 0-1023 scale.
pub const QUANTITY: [Zone; 20] = [
    zone(20, 0, 52),
    zone(19, 53, 103),
    zone(18, 104, 154),
    zone(17, 155, 205),
    zone(16, 206, 256),
    zone(15, 257, 307),
    zone(14, 308, 358),
    zone(13, 359, 409),
    zone(12, 410, 460),
    zone(11, 461, 511),
    zone(10, 512, 562),
    zone(9, 563, 613),
    zone(8, 614, 664),
    zone(7, 665, 715),
    zone(6, 716, 766),
    zone(5, 767, 817),
    zone(4, 818, 868),
    zone(3, 869, 919),
    zone(2, 920, 970),
    zone(1, 971, 1023),
];

/// Maps values representing side counts of typical board game
/// dice to ~equal zones on the 0-1023 scale.
pub const QUALITY: [Zone; 6] = [
    zone(20, 0, 169),
    zone(12, 170, 339),
    zone(10, 340, 509),
    zone(8, 510, 679),
    zone(6, 680, 849),
    zone(4, 850, 1023),
];

/// Returns the matching zone given the position on the corresponding scale.
pub fn detect_zone(position: u16, zones: &[Zone]) -> &Zone {
    for zone in zones {
        if zone.start <= position && zone.end >= position {
            return zone;
        }
    }

    panic!()
}

/// Detects whether a transition from the current zone into the new one should be made given
/// the new position on the scale and taking simple debouncing rules into account. Returns the new
/// zone or None if no transition should be made.
///
/// Debouncing is useful when the new scale position comes from a noisy source like an ADC.
/// If there was no debouncing and the position was right between two adjacent zones,
/// simple range matching would be constantly switching between the two zones.
///
/// This implementation registers a transition only if the new position crosses the specified
/// dead area following or preceding the current zone depending on the direction of the transition.
pub fn detect_zone_change(new_position: u16, dead_area: u16, current_zone: &'static Zone,
                          all_zones: &'static [Zone]) -> Option<&'static Zone> {
    if current_zone.start > dead_area && new_position < (current_zone.start - dead_area) {
        for s in all_zones {
            if new_position < s.end - dead_area {
                return Some(s);
            }
        }
    } else if new_position > current_zone.end + dead_area {
        for i in (0..all_zones.len()).rev() {
            if new_position > all_zones[i].start + dead_area {
                return Some(&all_zones[i]);
            }
        }
    }

    None
}
