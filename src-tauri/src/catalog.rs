use std::collections::HashMap;
use std::path::PathBuf;

use regex::Regex;

use crate::config::assets_root;
use crate::models::{StageInfo, StaticItem};
use crate::utils::{read_csv, safe_int};

pub struct StaticCatalog {
    root: PathBuf,
    items: HashMap<i64, StaticItem>,
    display_names: HashMap<i64, String>,
    groups: HashMap<i64, Vec<i64>>,
    drops: HashMap<i64, Vec<(String, Option<i64>, i64)>>,
    stages: Vec<StageInfo>,
    pub valid: bool,
}

impl StaticCatalog {
    pub fn new(root: Option<PathBuf>) -> Self {
        let root = root.unwrap_or_else(assets_root);
        let text = root.join("TextAsset");

        let required_files = [
            "ItemInfoData.txt",
            "DropInfoData.txt",
            "StageInfoData.txt",
            "ItemGroupInfoData.txt",
        ];
        for f in &required_files {
            if !text.join(f).exists() {
                eprintln!(
                    "[TBH] Warning: missing required asset: {}",
                    text.join(f).display()
                );
            }
        }

        let mut catalog = Self {
            root,
            items: HashMap::new(),
            display_names: HashMap::new(),
            groups: HashMap::new(),
            drops: HashMap::new(),
            stages: Vec::new(),
            valid: false,
        };
        if text.exists() {
            catalog.load();
            catalog.valid = true;
        }
        catalog
    }

    fn load(&mut self) {
        self.load_items();
        self.load_display_names();
        self.load_groups();
        self.load_drops();
        self.load_stages();
    }

    fn load_items(&mut self) {
        let path = self.root.join("TextAsset").join("ItemInfoData.txt");
        if !path.exists() {
            eprintln!("[catalog] ItemInfoData.txt not found at {}", path.display());
            return;
        }
        for row in read_csv(&path) {
            if row.is_empty() || !row[0].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let iid = row[0].parse::<i64>().unwrap_or(0);
            self.items.insert(
                iid,
                StaticItem {
                    id: Some(iid),
                    kind: row.get(1).cloned().unwrap_or_default(),
                    rarity: row.get(2).cloned().unwrap_or_default(),
                    slot: row.get(3).cloned().unwrap_or_default(),
                    subtype: row.get(4).cloned().unwrap_or_default(),
                    name: row.get(7).cloned().unwrap_or_default(),
                    drop_table: row.get(10).and_then(|s| safe_int(s)),
                },
            );
        }
    }

    fn load_display_names(&mut self) {
        let mb = self.root.join("MonoBehaviour");
        if !mb.exists() {
            return;
        }
        let id_re = Regex::new(r"_Inv_\s*(\d+)").unwrap();
        let name_re = Regex::new(r"value:\s*(.+)").unwrap();
        let fallback_re = Regex::new(r"_Inv_\s*\d+\s+(.+?)\.asset$").unwrap();

        if let Ok(entries) = std::fs::read_dir(&mb) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.starts_with("_Inv_") || !name_str.ends_with(".asset") {
                    continue;
                }
                let iid = id_re
                    .captures(&name_str)
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse::<i64>().ok());
                let iid = match iid {
                    Some(id) => id,
                    None => continue,
                };
                let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let val = text
                    .lines()
                    .find_map(|line| {
                        let line = line.trim();
                        name_re.captures(line).and_then(|c| {
                            let v = c
                                .get(1)?
                                .as_str()
                                .trim()
                                .trim_matches(|c| c == '\'' || c == '"');
                            if v.is_empty() {
                                None
                            } else {
                                Some(v.to_string())
                            }
                        })
                    })
                    .or_else(|| {
                        fallback_re.captures(&name_str).and_then(|c| {
                            let v = c.get(1)?.as_str().trim();
                            if v.is_empty() {
                                None
                            } else {
                                Some(v.to_string())
                            }
                        })
                    });
                if let Some(val) = val {
                    self.display_names.insert(iid, val);
                }
            }
        }
    }

    fn load_groups(&mut self) {
        let path = self.root.join("TextAsset").join("ItemGroupInfoData.txt");
        if !path.exists() {
            eprintln!(
                "[catalog] ItemGroupInfoData.txt not found at {}",
                path.display()
            );
            return;
        }
        for row in read_csv(&path) {
            if row.len() >= 3
                && row[0].chars().all(|c| c.is_ascii_digit())
                && row[2].chars().all(|c| c.is_ascii_digit())
            {
                let gid = row[0].parse::<i64>().unwrap_or(0);
                let iid = row[2].parse::<i64>().unwrap_or(0);
                self.groups.entry(gid).or_default().push(iid);
            }
        }
    }

    fn load_drops(&mut self) {
        let path = self.root.join("TextAsset").join("DropInfoData.txt");
        if !path.exists() {
            eprintln!(
                "[catalog] DropInfoData.txt not found at {}",
                path.display()
            );
            return;
        }
        for row in read_csv(&path) {
            if row.len() >= 6 && row[0].chars().all(|c| c.is_ascii_digit()) {
                let table = row[0].parse::<i64>().unwrap_or(0);
                let typ = row[2].clone();
                let target = safe_int(&row[3]);
                let weight = safe_int(&row[5]).unwrap_or(0);
                if target.is_some() && weight > 0 {
                    self.drops
                        .entry(table)
                        .or_default()
                        .push((typ, target, weight));
                }
            }
        }
    }

    fn load_stages(&mut self) {
        let path = self.root.join("TextAsset").join("StageInfoData.txt");
        if !path.exists() {
            eprintln!(
                "[catalog] StageInfoData.txt not found at {}",
                path.display()
            );
            return;
        }
        let box_re = Regex::new(r"^9[12]\d{4}$").unwrap();
        for row in read_csv(&path) {
            if row.is_empty() || !row[0].chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let mut boxes = Vec::new();
            for (i, v) in row.iter().enumerate() {
                if box_re.is_match(v) {
                    if let Some(bid) = safe_int(v) {
                        let count = row
                            .get(i + 1)
                            .and_then(|s| safe_int(s))
                            .unwrap_or(0) as i32;
                        boxes.push((bid, count));
                    }
                }
            }
            if !boxes.is_empty() {
                self.stages.push(StageInfo {
                    id: row[0].parse::<i64>().unwrap_or(0),
                    name: row.get(1).cloned().unwrap_or_default(),
                    difficulty: row.get(3).cloned().unwrap_or_default(),
                    level: row.get(6).and_then(|s| safe_int(s)).unwrap_or(-1) as i32,
                    boxes,
                });
            }
        }
    }

    pub fn pretty_name(&self, iid: i64) -> String {
        if let Some(name) = self.display_names.get(&iid) {
            return name.clone();
        }
        if let Some(info) = self.items.get(&iid) {
            let name_key = &info.name;
            if let Some(id_str) = name_key.strip_prefix("ItemName_") {
                if let Ok(id) = id_str.parse::<i64>() {
                    if let Some(name) = self.display_names.get(&id) {
                        return name.clone();
                    }
                }
            }
            let subtype = if !info.subtype.is_empty() {
                &info.subtype
            } else if !info.slot.is_empty() {
                &info.slot
            } else if !info.kind.is_empty() {
                &info.kind
            } else {
                name_key
            };
            let title = subtype.replace('_', " ");
            let mut chars = title.chars();
            match chars.next() {
                None => return String::new(),
                Some(f) => {
                    let upper: String = f.to_uppercase().collect();
                    return format!("{}{}", upper, chars.as_str());
                }
            }
        }
        iid.to_string()
    }

    pub fn item_parts(&self, iid: Option<i64>) -> (String, String) {
        let iid = match iid {
            Some(id) => id,
            None => return ("UNKNOWN".to_string(), "?".to_string()),
        };
        match self.items.get(&iid) {
            Some(info) => {
                let rarity = if info.rarity.is_empty() {
                    "UNKNOWN"
                } else {
                    &info.rarity
                };
                (rarity.to_string(), self.pretty_name(iid))
            }
            None => ("UNKNOWN".to_string(), iid.to_string()),
        }
    }

    fn expand(&self, typ: &str, target: Option<i64>) -> Vec<i64> {
        match typ {
            "ITEM" => target.into_iter().collect(),
            "ITEMGROUP" => self.groups.get(&target.unwrap_or(0)).cloned().unwrap_or_default(),
            _ => vec![],
        }
    }

    pub fn table_chance(
        &self,
        table_id: i64,
        rarity: Option<&str>,
        kind: Option<&str>,
        item_id: Option<i64>,
    ) -> f64 {
        let entries = match self.drops.get(&table_id) {
            Some(e) => e,
            None => return 0.0,
        };
        let total: i64 = entries.iter().map(|(_, _, w)| w).sum();
        if total <= 0 {
            return 0.0;
        }
        let mut hit = 0.0f64;
        for (typ, target, weight) in entries {
            let items = self.expand(typ, *target);
            if items.is_empty() {
                continue;
            }
            let mut hits = 0i64;
            for &iid in &items {
                let inf = self.items.get(&iid);
                let mut ok = true;
                if let Some(iid_filter) = item_id {
                    ok = iid == iid_filter;
                } else {
                    if let Some(r) = rarity {
                        ok = ok && inf.map_or(false, |i| i.rarity == r);
                    }
                    if let Some(k) = kind {
                        ok = ok && inf.map_or(false, |i| i.kind == k);
                    }
                }
                if ok {
                    hits += 1;
                }
            }
            hit += (*weight as f64) * (hits as f64 / items.len() as f64);
        }
        hit / total as f64
    }

    pub fn box_chance(
        &self,
        box_id: i64,
        rarity: Option<&str>,
        kind: Option<&str>,
        item_id: Option<i64>,
    ) -> f64 {
        let table = self.items.get(&box_id).and_then(|i| i.drop_table);
        match table {
            Some(t) => self.table_chance(t, rarity, kind, item_id),
            None => 0.0,
        }
    }

    pub fn stage_expected(
        &self,
        stage: &StageInfo,
        rarity: Option<&str>,
        kind: Option<&str>,
        item_id: Option<i64>,
    ) -> f64 {
        stage
            .boxes
            .iter()
            .map(|(bid, cnt)| self.box_chance(*bid, rarity, kind, item_id) * *cnt as f64)
            .sum()
    }

    pub fn rank_stages(
        &self,
        rarity: Option<&str>,
        kind: Option<&str>,
        item_id: Option<i64>,
        min_level: Option<i32>,
        max_level: Option<i32>,
        clear_time: Option<f64>,
        limit: usize,
    ) -> Vec<(f64, f64, Option<f64>, StageInfo)> {
        let mut rows: Vec<(f64, f64, Option<f64>, StageInfo)> = self
            .stages
            .iter()
            .filter(|s| {
                if let Some(min) = min_level {
                    if s.level < min {
                        return false;
                    }
                }
                if let Some(max) = max_level {
                    if s.level > max {
                        return false;
                    }
                }
                true
            })
            .map(|s| {
                let exp = self.stage_expected(s, rarity, kind, item_id);
                let per_hour = clear_time.map(|ct| exp * (3600.0 / ct));
                let sort_key = per_hour.unwrap_or(exp);
                (sort_key, exp, per_hour, s.clone())
            })
            .collect();
        rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        rows.truncate(limit);
        rows
    }

    pub fn items_count(&self) -> usize {
        self.items.len()
    }

    pub fn stages_count(&self) -> usize {
        self.stages.len()
    }

    pub fn display_names_count(&self) -> usize {
        self.display_names.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_catalog_with_items() -> StaticCatalog {
        let mut cat = StaticCatalog {
            root: PathBuf::new(),
            items: HashMap::new(),
            display_names: HashMap::new(),
            groups: HashMap::new(),
            drops: HashMap::new(),
            stages: Vec::new(),
            valid: true,
        };

        cat.items.insert(
            1001,
            StaticItem {
                id: Some(1001),
                kind: "weapon".into(),
                rarity: "RARE".into(),
                slot: "sword".into(),
                subtype: "longsword".into(),
                name: "ItemName_1001".into(),
                drop_table: Some(5001),
            },
        );
        cat.items.insert(
            1002,
            StaticItem {
                id: Some(1002),
                kind: "armor".into(),
                rarity: "LEGENDARY".into(),
                slot: "chest".into(),
                subtype: "plate".into(),
                name: "ItemName_1002".into(),
                drop_table: Some(5001),
            },
        );
        cat.items.insert(
            2001,
            StaticItem {
                id: Some(2001),
                kind: "material".into(),
                rarity: "COMMON".into(),
                slot: "".into(),
                subtype: "ore".into(),
                name: "ItemName_2001".into(),
                drop_table: None,
            },
        );

        cat.display_names.insert(1001, "Flame Sword".into());
        cat.display_names.insert(1002, "Dragon Plate".into());

        cat.groups.insert(3001, vec![1001, 1002]);

        cat.drops
            .entry(5001)
            .or_default()
            .push(("ITEM".into(), Some(1001), 10));
        cat.drops
            .entry(5001)
            .or_default()
            .push(("ITEM".into(), Some(1002), 5));
        cat.drops
            .entry(5001)
            .or_default()
            .push(("ITEMGROUP".into(), Some(3001), 15));

        cat.drops
            .entry(5002)
            .or_default()
            .push(("ITEM".into(), Some(2001), 100));

        cat.stages.push(StageInfo {
            id: 70001,
            name: "stage_70001".into(),
            difficulty: "normal".into(),
            level: 10,
            boxes: vec![(910651, 2)],
        });
        cat.stages.push(StageInfo {
            id: 70002,
            name: "stage_70002".into(),
            difficulty: "hard".into(),
            level: 20,
            boxes: vec![(920651, 3)],
        });

        cat.items.insert(
            910651,
            StaticItem {
                id: Some(910651),
                kind: "box".into(),
                rarity: "COMMON".into(),
                drop_table: Some(5001),
                ..Default::default()
            },
        );
        cat.items.insert(
            920651,
            StaticItem {
                id: Some(920651),
                kind: "box".into(),
                rarity: "COMMON".into(),
                drop_table: Some(5002),
                ..Default::default()
            },
        );

        cat
    }

    #[test]
    fn item_parts_known_id() {
        let cat = make_catalog_with_items();
        let (rarity, name) = cat.item_parts(Some(1001));
        assert_eq!(rarity, "RARE");
        assert_eq!(name, "Flame Sword");
    }

    #[test]
    fn item_parts_unknown_id() {
        let cat = make_catalog_with_items();
        let (rarity, name) = cat.item_parts(Some(99999));
        assert_eq!(rarity, "UNKNOWN");
        assert_eq!(name, "99999");
    }

    #[test]
    fn item_parts_none() {
        let cat = make_catalog_with_items();
        let (rarity, name) = cat.item_parts(None);
        assert_eq!(rarity, "UNKNOWN");
        assert_eq!(name, "?");
    }

    #[test]
    fn table_chance_nonexistent() {
        let cat = make_catalog_with_items();
        assert_eq!(cat.table_chance(99999, None, None, None), 0.0);
    }

    #[test]
    fn table_chance_all_items() {
        let cat = make_catalog_with_items();
        let chance = cat.table_chance(5001, None, None, None);
        assert!(chance > 0.0);
    }

    #[test]
    fn table_chance_filter_rarity() {
        let cat = make_catalog_with_items();
        let rarer = cat.table_chance(5001, Some("LEGENDARY"), None, None);
        let common = cat.table_chance(5001, Some("RARE"), None, None);
        assert!(rarer > 0.0);
        assert!(common > 0.0);
        assert!(rarer != common);
    }

    #[test]
    fn table_chance_filter_item() {
        let cat = make_catalog_with_items();
        let only_1001 = cat.table_chance(5001, None, None, Some(1001));
        let only_1002 = cat.table_chance(5001, None, None, Some(1002));
        assert!(only_1001 > 0.0);
        assert!(only_1002 > 0.0);
    }

    #[test]
    fn expand_item() {
        let cat = make_catalog_with_items();
        let result = cat.expand("ITEM", Some(12345));
        assert_eq!(result, vec![12345]);
    }

    #[test]
    fn expand_item_none() {
        let cat = make_catalog_with_items();
        let result = cat.expand("ITEM", None);
        assert!(result.is_empty());
    }

    #[test]
    fn expand_itemgroup() {
        let cat = make_catalog_with_items();
        let result = cat.expand("ITEMGROUP", Some(3001));
        assert_eq!(result, vec![1001, 1002]);
    }

    #[test]
    fn expand_itemgroup_unknown() {
        let cat = make_catalog_with_items();
        let result = cat.expand("ITEMGROUP", Some(99999));
        assert!(result.is_empty());
    }

    #[test]
    fn expand_unknown_type() {
        let cat = make_catalog_with_items();
        let result = cat.expand("WEAPON", Some(1001));
        assert!(result.is_empty());
    }

    #[test]
    fn box_chance_no_drop_table() {
        let cat = make_catalog_with_items();
        assert_eq!(cat.box_chance(2001, None, None, None), 0.0);
    }

    #[test]
    fn box_chance_with_drop_table() {
        let cat = make_catalog_with_items();
        let chance = cat.box_chance(910651, None, None, None);
        assert!(chance > 0.0);
    }

    #[test]
    fn box_chance_unknown_box() {
        let cat = make_catalog_with_items();
        assert_eq!(cat.box_chance(99999, None, None, None), 0.0);
    }

    #[test]
    fn pretty_name_from_display_names() {
        let cat = make_catalog_with_items();
        assert_eq!(cat.pretty_name(1001), "Flame Sword");
    }

    #[test]
    fn pretty_name_from_item_name_prefix() {
        let mut cat = make_catalog_with_items();
        cat.items.insert(
            5555,
            StaticItem {
                id: Some(5555),
                name: "ItemName_1001".into(),
                ..Default::default()
            },
        );
        assert_eq!(cat.pretty_name(5555), "Flame Sword");
    }

    #[test]
    fn pretty_name_fallback_to_subtype() {
        let cat = make_catalog_with_items();
        assert_eq!(cat.pretty_name(2001), "Ore");
    }

    #[test]
    fn pretty_name_unknown_item_returns_id() {
        let cat = make_catalog_with_items();
        assert_eq!(cat.pretty_name(99999), "99999");
    }

    #[test]
    fn stage_expected_basic() {
        let cat = make_catalog_with_items();
        let stage = &cat.stages[0];
        let exp = cat.stage_expected(stage, None, None, None);
        assert!(exp > 0.0);
    }

    #[test]
    fn rank_stages_basic() {
        let cat = make_catalog_with_items();
        let ranked = cat.rank_stages(None, None, None, None, None, None, 10);
        assert!(!ranked.is_empty());
        assert!(ranked.len() <= 10);
    }

    #[test]
    fn rank_stages_with_level_filter() {
        let cat = make_catalog_with_items();
        let ranked = cat.rank_stages(None, None, None, Some(15), None, None, 10);
        for (_, _, _, stage) in &ranked {
            assert!(stage.level >= 15);
        }
    }

    #[test]
    fn rank_stages_sorted_descending() {
        let cat = make_catalog_with_items();
        let ranked = cat.rank_stages(None, None, None, None, None, None, 10);
        for i in 1..ranked.len() {
            assert!(ranked[i - 1].0 >= ranked[i].0);
        }
    }

    #[test]
    fn rank_stages_with_clear_time() {
        let cat = make_catalog_with_items();
        let ranked = cat.rank_stages(None, None, None, None, None, Some(60.0), 10);
        assert!(!ranked.is_empty());
        for (_, _, per_hour, _) in &ranked {
            assert!(per_hour.is_some());
        }
    }

    #[test]
    fn id_regex_matches_valid_ids() {
        let re = Regex::new(r"_Inv_\s*(\d+)").unwrap();
        assert!(re.is_match("_Inv_12345 Sword.asset"));
        assert!(re.is_match("_Inv_12345.asset"));
        assert!(re.captures("_Inv_ 12345 Sword.asset").is_some());
    }

    #[test]
    fn id_regex_rejects_invalid() {
        let re = Regex::new(r"_Inv_\s*(\d+)").unwrap();
        assert!(!re.is_match("Other_12345.asset"));
    }

    #[test]
    fn box_regex_matches() {
        let re = Regex::new(r"^9[12]\d{4}$").unwrap();
        assert!(re.is_match("910651"));
        assert!(re.is_match("920651"));
        assert!(re.is_match("910000"));
        assert!(re.is_match("929999"));
    }

    #[test]
    fn box_regex_rejects() {
        let re = Regex::new(r"^9[12]\d{4}$").unwrap();
        assert!(!re.is_match("900651"));
        assert!(!re.is_match("930651"));
        assert!(!re.is_match("91065"));
        assert!(!re.is_match("9106510"));
    }

    #[test]
    fn value_regex_extracts_name() {
        let re = Regex::new(r"value:\s*(.+)").unwrap();
        let line = "  value: 'Flame Sword'";
        let caps = re.captures(line).unwrap();
        let val = caps.get(1).unwrap().as_str().trim().trim_matches(|c| c == '\'' || c == '"');
        assert_eq!(val, "Flame Sword");
    }

    #[test]
    fn fallback_regex_extracts_name() {
        let re = Regex::new(r"_Inv_\s*\d+\s+(.+?)\.asset$").unwrap();
        let name = "_Inv_12345 Flame Sword.asset";
        let caps = re.captures(name).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "Flame Sword");
    }
}
