//! Ordered sound-group membership, configured mixes and local ducking state.
use crate::dsp::Error;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Group {
    name: Vec<u8>,
    priority: i32,
    ducked: bool,
    causes_ducking: bool,
    target: f32,
    threshold: f32,
    gain: f32,
}
#[derive(Clone, Debug)]
struct Rule {
    group: usize,
    path: Vec<u8>,
    class: Vec<u8>,
    channel: Option<i32>,
    levels: [Option<i32>; 2],
}
#[derive(Clone, Debug)]
pub struct Mixers {
    groups: Vec<Group>,
    rules: Vec<Rule>,
    mixes: BTreeMap<Vec<u8>, Vec<Option<f32>>>,
    selected: Vec<u8>,
    last_update: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Membership {
    pub groups: [i16; 8],
}

fn tokens(bytes: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
    let mut result = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at].is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if bytes.get(at..at + 2) == Some(b"//") {
            while at < bytes.len() && bytes[at] != b'\n' {
                at += 1;
            }
            continue;
        }
        if b"{}".contains(&bytes[at]) {
            result.push(vec![bytes[at]]);
            at += 1;
            continue;
        }
        let quoted = bytes[at] == b'"';
        at += usize::from(quoted);
        let start = at;
        while at < bytes.len()
            && if quoted {
                bytes[at] != b'"'
            } else {
                !bytes[at].is_ascii_whitespace() && !b"{}".contains(&bytes[at])
            }
        {
            at += 1;
        }
        if quoted && at == bytes.len() {
            return Err(Error::Malformed("unterminated mixer token"));
        }
        if at - start > 31 {
            return Err(Error::Malformed("mixer token limit"));
        }
        result.push(bytes[start..at].to_vec());
        at += usize::from(quoted);
    }
    Ok(result)
}
fn number(token: &[u8]) -> f32 {
    playsrc_keyvalues::NumericValue::Bytes(token).get_float()
}
fn integer(token: &[u8]) -> i32 {
    playsrc_keyvalues::NumericValue::Bytes(token).get_int()
}
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|part| part.eq_ignore_ascii_case(needle))
}

impl Mixers {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > 1024 * 1024 {
            return Err(Error::Malformed("mixer input limit"));
        }
        let values = tokens(bytes)?;
        let mut at = 0;
        let mut result = Self {
            groups: vec![],
            rules: vec![],
            mixes: BTreeMap::new(),
            selected: b"default_mix".to_vec(),
            last_update: 0.0,
        };
        while at < values.len() {
            let name = values[at].to_ascii_lowercase();
            at += 1;
            if values.get(at).map(Vec::as_slice) != Some(b"{") {
                return Err(Error::Malformed("mixer root"));
            }
            at += 1;
            if name == b"grouprules" {
                while values.get(at).map(Vec::as_slice) != Some(b"}") {
                    let row = values
                        .get(at..at + 11)
                        .ok_or(Error::Malformed("mixer rule"))?;
                    at += 11;
                    let group = if let Some(index) = result
                        .groups
                        .iter()
                        .position(|group| group.name.eq_ignore_ascii_case(&row[0]))
                    {
                        index
                    } else {
                        let index = result.groups.len();
                        result.groups.push(Group {
                            name: row[0].to_ascii_lowercase(),
                            priority: integer(&row[6]).clamp(0, 100),
                            ducked: integer(&row[7]) != 0,
                            causes_ducking: integer(&row[8]) != 0,
                            target: integer(&row[9]) as f32 / 100.0,
                            threshold: integer(&row[10]) as f32 / 100.0,
                            gain: 1.0,
                        });
                        index
                    };
                    let channel = if row[3].is_empty() {
                        None
                    } else {
                        let channels = [
                            b"CHAN_AUTO".as_slice(),
                            b"CHAN_WEAPON",
                            b"CHAN_VOICE",
                            b"CHAN_ITEM",
                            b"CHAN_BODY",
                            b"CHAN_STREAM",
                            b"CHAN_STATIC",
                        ];
                        Some(
                            channels
                                .iter()
                                .position(|name| row[3].eq_ignore_ascii_case(name))
                                .ok_or(Error::Malformed("mixer channel"))?
                                as i32,
                        )
                    };
                    result.rules.push(Rule {
                        group,
                        path: row[1].clone(),
                        class: row[2].clone(),
                        channel,
                        levels: [
                            (!row[4].is_empty()).then(|| integer(&row[4])),
                            (!row[5].is_empty()).then(|| integer(&row[5])),
                        ],
                    });
                    if result.groups.len() > 64 || result.rules.len() > 80 {
                        return Err(Error::Malformed("mixer group limit"));
                    }
                }
            } else {
                let mut mix = vec![None; result.groups.len()];
                while values.get(at).map(Vec::as_slice) != Some(b"}") {
                    let pair = values
                        .get(at..at + 2)
                        .ok_or(Error::Malformed("mixer value"))?;
                    at += 2;
                    let gain = number(&pair[1]);
                    if !gain.is_finite() {
                        return Err(Error::Malformed("mixer gain"));
                    }
                    if let Some(index) = result
                        .groups
                        .iter()
                        .position(|group| group.name.eq_ignore_ascii_case(&pair[0]))
                    {
                        mix[index] = Some(gain);
                    }
                }
                result.mixes.insert(name, mix);
                if result.mixes.len() > 32 {
                    return Err(Error::Malformed("mixer count limit"));
                }
            }
            at += 1;
        }
        if !result.mixes.contains_key(b"default_mix".as_slice()) {
            return Err(Error::Malformed("missing default mixer"));
        }
        Ok(result)
    }

    pub fn select(&mut self, name: Option<&[u8]>) {
        let name = name.unwrap_or(b"Default_Mix").to_ascii_lowercase();
        if self.mixes.contains_key(&name) {
            self.selected = name;
        }
    }

    pub fn membership(&self, path: &[u8], class: &[u8], channel: i32, level: i32) -> Membership {
        let mut result = Membership { groups: [-1; 8] };
        let mut count = 0;
        for rule in &self.rules {
            if !contains(path, &rule.path)
                || !contains(class, &rule.class)
                || rule.channel.is_some_and(|value| value != channel)
                || rule.levels[0].is_some_and(|value| level < value)
                || rule.levels[1].is_some_and(|value| level > value)
            {
                continue;
            }
            result.groups[count] = rule.group as i16;
            count += 1;
            if count == 8 {
                break;
            }
        }
        result
    }

    pub fn gain(&self, membership: Membership) -> f32 {
        let mut duck = 1.0_f32;
        let mix = &self.mixes[&self.selected];
        for &index in &membership.groups {
            if index >= 0 {
                duck = duck.min(self.groups[index as usize].gain);
            }
        }
        for index in membership.groups {
            if index >= 0
                && let Some(gain) = mix[index as usize]
            {
                return gain * duck;
            }
        }
        duck
    }

    pub fn update_ducking(
        &mut self,
        host_time: f64,
        voices: impl IntoIterator<Item = (Membership, f32)>,
    ) {
        if (host_time - self.last_update).abs() < 0.1 {
            return;
        }
        self.last_update = host_time;
        let mut totals = [0.0_f32; 64];
        let mut has_ducked = false;
        for (membership, volume) in voices {
            if volume <= 0.0 {
                continue;
            }
            for index in membership
                .groups
                .into_iter()
                .filter(|index| *index >= 0)
                .map(|index| index as usize)
            {
                let group = &self.groups[index];
                if group.causes_ducking {
                    totals[index] += volume;
                }
                has_ducked |= group.ducked;
            }
        }
        if !has_ducked {
            return;
        }
        let mut targets = [1.0_f32; 64];
        for (index, group) in self
            .groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.ducked)
        {
            if self.groups.iter().enumerate().any(|(other_index, other)| {
                other.priority > group.priority
                    && other.causes_ducking
                    && totals[other_index] > other.threshold
            }) {
                targets[index] = group.target;
            }
        }
        for (index, group) in self
            .groups
            .iter_mut()
            .enumerate()
            .filter(|(_, group)| group.ducked)
        {
            let target = targets[index];
            if group.gain == target {
                continue;
            }
            let amount = (f64::from(1.0 - group.target)
                * (0.1 / if target < group.gain { 0.5 } else { 2.5 }))
                as f32;
            group.gain = if target < group.gain {
                (group.gain - amount).max(target)
            } else {
                (group.gain + amount).min(target)
            };
        }
    }
}
